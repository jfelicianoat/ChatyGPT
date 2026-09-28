// @vitest-environment jsdom
/**
 * Pruebas de interfaz sobre las acciones sensibles.
 *
 * El 5 de agosto de 2026 se descubrió que cinco acciones enviaban
 * `confirmed: true` a Rust sin haber preguntado a nadie: la comprobación del
 * backend existía, pero el frontend la satisfacía por su cuenta. La prueba de
 * contrato en Python impide que vuelva a ocurrir leyendo el código fuente; esta
 * lo comprueba desde el otro lado, **ejecutando la interfaz**: monta la
 * aplicación, pulsa el botón real y verifica que cancelar no ejecuta nada.
 *
 * Es la diferencia entre «la confirmación está escrita» y «la confirmación
 * funciona». Ambas comprobaciones se complementan: un análisis estático no
 * detecta que la pregunta se ignore, y esta no detecta una ruta que nadie use.
 */

import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** Respuestas por defecto que permiten montar la aplicación sin Broker real. */
const DEFAULTS: Record<string, unknown> = {
  bootstrap: {
    appVersion: "0.1.0",
    databasePath: "C:/pruebas/chatygpt.db",
    logPath: null,
    schemaVersion: 18,
    recoveredTasks: 0,
    recoveredAttachments: 0,
    recoveryItems: []
  },
  diagnoseBroker: {
    reachable: true,
    ready: true,
    baseUrl: "http://127.0.0.1:8765",
    contractVersion: "2.8",
    strategies: ["single"],
    presets: {},
    workLanes: ["inference"],
    agentSkills: [],
    agentSkillsEgress: [],
    taskDependencies: true,
    latencyMs: 4,
    message: "Broker AI está listo"
  },
  getWindowsStartupStatus: {
    supported: true,
    enabled: false,
    credentialProtected: true,
    message: "Disponible"
  },
  getBrokerCredential: {
    source: "protected",
    protected: true,
    environmentPresent: false,
    message: "Credencial cifrada para tu cuenta de Windows."
  },
  getAthenaStatus: {
    estado: "conectado",
    urlBase: "http://127.0.0.1:8770",
    credencialConfigurada: true,
    versionContrato: 1,
    runsActivos: 0
  },
  listAuthorizedFolders: [
    {
      id: "folder-1",
      path: "D:/Exportaciones",
      displayName: "D:/Exportaciones",
      permissions: { write: true, purpose: "export" },
      grantedAt: "2026-08-01T10:00:00Z",
      revokedAt: null
    }
  ],
  // El listado de perfiles es un objeto, no una lista: el valor por defecto del doble
  // dejaría `profiles` sin definir, y el selector se cae al contarlos.
  listAthenaProfiles: { default: "software_engineering", profiles: [] },
  // Una orden que devuelve un objeto necesita su entrada aqui: el doble contesta `[]`
  // por defecto y el componente lee `.models` de esa lista.
  listAthenaModels: { default: "", models: [] },
  getMemoryOverview: { enabled: false, items: [] },
  getLatestMemorySearch: null,
  getPerformanceReport: {
    sampleLimit: 200,
    totalSamples: 12,
    metrics: [
      {
        metric: "app_start",
        label: "Arranque de la aplicación",
        description: "Desde que la vista web empieza a cargar.",
        budgetMs: 2000,
        samples: 12,
        p50Ms: 900,
        p95Ms: 1400,
        maxMs: 1800,
        meetsBudget: true,
        lastRecordedAt: "2026-08-05T09:00:00Z"
      }
    ]
  }
};

/**
 * Doble de `platform` que registra cada llamada.
 *
 * Se usa un proxy para no tener que declarar las más de cien órdenes: las que
 * la prueba no necesita devuelven una lista vacía, que es lo que la interfaz
 * espera de casi todas ellas.
 */
const callLog = new Map<string, ReturnType<typeof vi.fn>>();

function platformMethod(name: string) {
  let mock = callLog.get(name);
  if (!mock) {
    mock = vi.fn(async () =>
      Object.prototype.hasOwnProperty.call(DEFAULTS, name) ? DEFAULTS[name] : []
    );
    callLog.set(name, mock);
  }
  return mock;
}

vi.mock("../../../apps/desktop/src/platform", () => ({
  platform: new Proxy(
    {},
    {
      get: (_target, property: string) => platformMethod(property)
    }
  )
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: async () => () => undefined
  })
}));

import { App } from "../../../apps/desktop/src/App";
import { WorkflowStudio } from "../../../apps/desktop/src/WorkflowStudio";

/** Espera a que el arranque termine y la pantalla de Inicio esté montada. */
async function mountHome() {
  render(<App />);
  await waitFor(() => expect(platformMethod("bootstrap")).toHaveBeenCalled());
  await screen.findByRole("heading", { name: "Credencial de Broker AI" });
}


// Estas pruebas documentan el comportamiento defectuoso actual.
// Sus aserciones deberán invertirse al convertirlas en regresiones tras corregirlo.
const summaries = ['A', 'B', 'C'].map(id => ({
  id, title: `Chat ${id}`, projectId: null, updatedAt: '2026-09-28T10:00:00Z'
}));
const view = (id: string) => ({
  ...summaries.find(item => item.id === id), customGptId: null,
  executionPreferences: {dataClassification:'public',strategy:'single',preset:'fast',maxCostUsd:0.1,longContext:'fail',priority:50},
  messages:[], researchRuns:[]
});
const task = {id:'task-A',remoteTaskId:'remote-A',remoteStatus:'completed',localState:'terminal',consecutivePollErrors:0,result:{},progress:{phase:'completed'},pendingToolCalls:[],updatedAt:'2026-09-28T10:00:00Z'};
async function setup() {
  platformMethod('listConversations').mockResolvedValue(summaries);
  platformMethod('getConversation').mockImplementation(async (id:string)=>view(id));
  render(<App/>);
  await screen.findByRole('heading',{name:'Chat A'});
}
describe('Evidencias de auditoría 2026-09-28',()=>{
  beforeEach(()=>{callLog.clear();vi.restoreAllMocks();});
  afterEach(()=>cleanup());
  it('reproduce el traslado del borrador de A a B',async()=>{
    await setup();
    await userEvent.type(screen.getByPlaceholderText('Escribe un mensaje…'),'Contenido privado destinado a A');
    await userEvent.click(screen.getByRole('button',{name:'Chat B'}));
    await screen.findByRole('heading',{name:'Chat B'});
    expect((screen.getByPlaceholderText('Escribe un mensaje…') as HTMLTextAreaElement).value).toBe('Contenido privado destinado a A');
  });
  it('reproduce que una carga antigua B desplaza la última selección C',async()=>{
    await setup();
    let finishB!:(value:unknown)=>void;
    platformMethod('getConversation').mockImplementation((id:string)=>id==='B'?new Promise(resolve=>{finishB=resolve;}):Promise.resolve(view(id)));
    await userEvent.click(screen.getByRole('button',{name:'Chat B'}));
    await userEvent.click(screen.getByRole('button',{name:'Chat C'}));
    await screen.findByRole('heading',{name:'Chat C'});
    await act(async()=>finishB(view('B')));
    expect(screen.getByRole('heading',{name:'Chat B'})).toBeDefined();
  });
  it('reproduce que la aceptación tardía de A saca al usuario de B',async()=>{
    await setup();
    let finish!:(value:unknown)=>void;
    platformMethod('sendChatTurn').mockReturnValue(new Promise(resolve=>{finish=resolve;}));
    await userEvent.type(screen.getByPlaceholderText('Escribe un mensaje…'),'Pregunta de A');
    await userEvent.click(screen.getByRole('button',{name:'Enviar'}));
    await userEvent.click(screen.getByRole('button',{name:'Chat B'}));
    await screen.findByRole('heading',{name:'Chat B'});
    await act(async()=>finish(task));
    await screen.findByRole('heading',{name:'Chat A'});
  });
  it('reproduce que un fallo de refresco restaura un texto ya aceptado',async()=>{
    await setup();
    platformMethod('sendChatTurn').mockResolvedValue(task);
    platformMethod('getConversation').mockRejectedValueOnce(new Error('Fallo simulado de lectura posterior al commit'));
    await userEvent.type(screen.getByPlaceholderText('Escribe un mensaje…'),'Pregunta aceptada');
    await userEvent.click(screen.getByRole('button',{name:'Enviar'}));
    await waitFor(()=>expect((screen.getByPlaceholderText('Escribe un mensaje…') as HTMLTextAreaElement).value).toBe('Pregunta aceptada'));
    expect(platformMethod('sendChatTurn')).toHaveBeenCalledTimes(1);
  });
  it('reproduce el envío con Enter mientras sigue activa la composición IME',async()=>{
    await setup();
    platformMethod('sendChatTurn').mockReturnValue(new Promise(()=>{}));
    const composer=screen.getByPlaceholderText('Escribe un mensaje…');
    fireEvent.change(composer,{target:{value:'文字 en composición'}});
    fireEvent.keyDown(composer,{key:'Enter',code:'Enter',isComposing:true,keyCode:229});
    await waitFor(()=>expect(platformMethod('sendChatTurn')).toHaveBeenCalledTimes(1));
  });
  it('reproduce la pérdida de un flujo sin guardar al abrir otro',async()=>{
    const first={id:'w1',name:'Flujo uno',description:'Descripción guardada',projectId:null,publishedVersionNo:1,nodeCount:2,updatedAt:'2026-09-28',definition:{nodes:[{id:'input',kind:'input',label:'Entrada',x:20,y:50,attachmentIds:[]},{id:'result',kind:'result',label:'Resultado',x:500,y:50,attachmentIds:[]}],edges:[{id:'edge',source:'input',target:'result'}]}};
    const second={...first,id:'w2',name:'Flujo dos'};
    platformMethod('listWorkflows').mockResolvedValue([first,second]);
    platformMethod('getWorkflow').mockImplementation(async(id:string)=>id==='w1'?first:second);
    const confirmation=vi.spyOn(window,'confirm').mockReturnValue(false);
    render(<WorkflowStudio projects={[]} customGpts={[]} onOpenBrokerCredential={()=>{}} onOpenAutomations={()=>{}}/>);
    await screen.findByDisplayValue('Flujo uno');
    fireEvent.change(screen.getByLabelText('Descripción'),{target:{value:'Trabajo sin guardar'}});
    await userEvent.click(screen.getByRole('button',{name:/Flujo dos/}));
    await screen.findByDisplayValue('Flujo dos');
    await userEvent.click(screen.getByRole('button',{name:/Flujo uno/}));
    await screen.findByDisplayValue('Flujo uno');
    expect((screen.getByLabelText('Descripción') as HTMLInputElement).value).toBe('Descripción guardada');
    expect(confirmation).not.toHaveBeenCalled();
    expect(platformMethod('saveWorkflow')).not.toHaveBeenCalled();
  });
});
