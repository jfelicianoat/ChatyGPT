import { describe, expect, it } from "vitest";

import type { AthenaEstadoArea, AthenaPermiso, AthenaRun } from "./domain";
import {
  debeSeguirSondeando,
  esFaseTerminal,
  etiquetasPermiso,
  mensajeServicio,
  motivoBloqueoPermiso,
  nombreEstadoTarea,
  nombreFase,
  nombreRiesgo,
  nombreVerificacion,
  permisoActivo,
  puedeCancelarse,
  puedeLanzarse,
  puedeReanudarse,
  puedeResponderPermiso,
  resumenActividad,
  textoArgumento,
  tiempoRestante
} from "./athenaView";

function run(parcial: Partial<AthenaRun> = {}): AthenaRun {
  return {
    runId: "run-1",
    objetivo: "Arreglar calc.add",
    fase: "running",
    carpeta: "D:/repo",
    degradado: false,
    reanudable: false,
    conectado: true,
    suscriptor: "sus-1",
    controla: true,
    tareas: [],
    herramientas: [],
    permisos: [],
    comprobaciones: [],
    ficherosModificados: [],
    artefactos: [],
    errores: [],
    actividad: [],
    evidencia: [],
    ciclosReparacion: 0,
    ...parcial
  };
}

function permiso(parcial: Partial<AthenaPermiso> = {}): AthenaPermiso {
  return {
    requestId: "req-1",
    herramienta: "edit_file",
    operacion: "write",
    accion: "replace 1 occurrence(s) in calc.py",
    riesgo: "medium",
    nivel: "r1_workspace_write",
    motivo: "quiere escribir",
    efectos: ["Modifica calc.py"],
    recursos: ["calc.py"],
    workspace: "D:/repo",
    argumentos: [],
    soloLectura: false,
    destructivo: false,
    confirmado: false,
    segundosRestantes: 300,
    caducado: false,
    ...parcial
  };
}

function estado(parcial: Partial<AthenaEstadoArea> = {}): AthenaEstadoArea {
  return {
    estado: "conectado",
    urlBase: "http://127.0.0.1:8770",
    credencialConfigurada: true,
    runsActivos: 0,
    ...parcial
  };
}

describe("fases", () => {
  it("distingue las fases terminales de las que aún pueden cambiar", () => {
    expect(esFaseTerminal("completed")).toBe(true);
    expect(esFaseTerminal("failed")).toBe(true);
    expect(esFaseTerminal("cancelled")).toBe(true);
    expect(esFaseTerminal("running")).toBe(false);
    expect(esFaseTerminal("waiting_permission")).toBe(false);
    expect(esFaseTerminal("verifying")).toBe(false);
    expect(esFaseTerminal(undefined)).toBe(false);
  });

  it("un run interrumpido no cuenta como terminado", () => {
    // Es lo que impide presentarlo como trabajo hecho.
    expect(esFaseTerminal("recovery_pending")).toBe(false);
    expect(nombreFase("recovery_pending")).toContain("necesita decisión");
  });

  it("nombra las ocho fases sin inventarse ninguna", () => {
    expect(nombreFase("starting")).toBe("Arrancando");
    expect(nombreFase("running")).toBe("Trabajando");
    expect(nombreFase("waiting_permission")).toBe("Esperando tu autorización");
    expect(nombreFase("verifying")).toBe("Verificando");
    expect(nombreFase("completed")).toBe("Terminado");
    expect(nombreFase("failed")).toBe("Fallido");
    expect(nombreFase("cancelled")).toBe("Cancelado");
    expect(nombreFase(undefined)).toBe("Sin estado");
  });

  it("nombra los siete estados de tarea", () => {
    expect(nombreEstadoTarea("pending")).toBe("Pendiente");
    expect(nombreEstadoTarea("running")).toBe("En marcha");
    expect(nombreEstadoTarea("completed")).toBe("Terminada");
    expect(nombreEstadoTarea("failed")).toBe("Fallida");
    expect(nombreEstadoTarea("cancelled")).toBe("Cancelada");
    expect(nombreEstadoTarea("killed")).toBe("Detenida");
    expect(nombreEstadoTarea("recovery_pending")).toBe("Por recuperar");
  });
});

describe("sondeo", () => {
  it("sigue mientras el run pueda cambiar solo", () => {
    expect(debeSeguirSondeando(run({ fase: "running" }))).toBe(true);
    expect(debeSeguirSondeando(run({ fase: "waiting_permission" }))).toBe(true);
    expect(debeSeguirSondeando(run({ fase: "verifying" }))).toBe(true);
  });

  it("para cuando el run ha terminado", () => {
    expect(debeSeguirSondeando(run({ fase: "completed" }))).toBe(false);
    expect(debeSeguirSondeando(run({ fase: "failed" }))).toBe(false);
    expect(debeSeguirSondeando(null)).toBe(false);
  });
});

describe("acciones disponibles", () => {
  it("cancelar sigue disponible mientras se espera una autorización", () => {
    // Quien mira la petición puede preferir parar del todo.
    expect(puedeCancelarse(run({ fase: "waiting_permission" }))).toBe(true);
    expect(puedeCancelarse(run({ fase: "completed" }))).toBe(false);
  });

  it("solo se reanuda lo que Athena marcó como reanudable", () => {
    expect(puedeReanudarse(run({ fase: "recovery_pending", reanudable: true }))).toBe(true);
    expect(puedeReanudarse(run({ fase: "recovery_pending", reanudable: false }))).toBe(false);
    expect(puedeReanudarse(run({ fase: "failed", reanudable: true }))).toBe(false);
  });

  it("no se puede responder a un permiso sin controlar el run", () => {
    // Sin la identidad que entrega el flujo, el servicio rechazaría la respuesta.
    expect(puedeResponderPermiso(run())).toBe(true);
    expect(puedeResponderPermiso(run({ controla: false }))).toBe(false);
    expect(puedeResponderPermiso(run({ suscriptor: undefined }))).toBe(false);
  });

  it("no se lanza un run sin servicio, objetivo o carpeta", () => {
    expect(puedeLanzarse(estado(), "Arreglar", "D:/repo")).toBe(true);
    expect(puedeLanzarse(estado({ estado: "no_disponible" }), "Arreglar", "D:/repo")).toBe(false);
    expect(puedeLanzarse(estado(), "   ", "D:/repo")).toBe(false);
    expect(puedeLanzarse(estado(), "Arreglar", "")).toBe(false);
    expect(puedeLanzarse(null, "Arreglar", "D:/repo")).toBe(false);
  });
});

describe("permisos", () => {
  it("señala el permiso al que hay que atender", () => {
    expect(permisoActivo(run())).toBeNull();
    const conPermiso = run({ permisos: [permiso(), permiso({ requestId: "req-2" })] });
    expect(permisoActivo(conPermiso)?.requestId).toBe("req-1");
  });

  it("muestra el tiempo que queda de forma legible", () => {
    expect(tiempoRestante(permiso({ segundosRestantes: 300 }))).toBe("5 min");
    expect(tiempoRestante(permiso({ segundosRestantes: 95 }))).toBe("1 min 35 s");
    expect(tiempoRestante(permiso({ segundosRestantes: 28 }))).toBe("28 s");
    expect(tiempoRestante(permiso({ segundosRestantes: 0 }))).toBe("sin tiempo");
  });
});

describe("verificación", () => {
  it("dice el veredicto sin suavizarlo", () => {
    expect(nombreVerificacion("passed")).toBe("La verificación pasó");
    expect(nombreVerificacion("failed")).toBe("La verificación falló");
    expect(nombreVerificacion("inconclusive")).toBe("No se pudo verificar");
  });

  it("no dice nada cuando todavía no hay veredicto", () => {
    expect(nombreVerificacion(undefined)).toBeNull();
    expect(nombreVerificacion("algo_nuevo")).toBeNull();
  });
});

describe("estado del servicio", () => {
  it("distingue no disponible de incompatible", () => {
    expect(mensajeServicio(estado({ estado: "no_disponible" }))).toContain(
      "chat normal sigue funcionando"
    );
    expect(mensajeServicio(estado({ estado: "incompatible" }))).toContain("versión");
  });

  it("no promete nada mientras no se ha comprobado", () => {
    expect(mensajeServicio(null)).toContain("Comprobando");
  });
});

describe("resumen", () => {
  it("cuenta lo que el run lleva hecho", () => {
    const vista = run({
      ficherosModificados: ["calc.py", "test_calc.py"],
      ciclosReparacion: 1,
      errores: [{ codigo: "permission_denied", mensaje: "denegado" }]
    });

    expect(resumenActividad(vista)).toBe(
      "2 ficheros modificados · 1 ciclo de reparación · 1 error"
    );
  });

  it("no dice nada cuando no hay nada que contar", () => {
    expect(resumenActividad(run())).toBe("");
    expect(resumenActividad(null)).toBe("");
  });
});

describe("decisión sobre un permiso", () => {
  it("deja responder cuando esta ventana controla el run", () => {
    const vista = run({ controla: true, suscriptor: "sub-1", fase: "waiting_permission" });

    expect(motivoBloqueoPermiso(vista, permiso())).toBeNull();
  });

  it("no deja responder una petición caducada", () => {
    // El plazo lo lleva Athena; cuando se agota ya dio la petición por
    // denegada, así que un botón activo solo produciría un error confuso.
    const vista = run({ controla: true, suscriptor: "sub-1", fase: "waiting_permission" });

    const motivo = motivoBloqueoPermiso(vista, permiso({ caducado: true, segundosRestantes: 0 }));

    expect(motivo).toContain("plazo");
  });

  it("no deja responder si el run ya terminó", () => {
    const vista = run({ controla: true, suscriptor: "sub-1", fase: "cancelled" });

    expect(motivoBloqueoPermiso(vista, permiso())).toContain("terminó");
  });

  it("explica que manda otra ventana en vez de callarse", () => {
    const vista = run({ controla: false, suscriptor: undefined });

    expect(motivoBloqueoPermiso(vista, permiso())).toContain("Otra ventana");
  });

  it("mantiene puedeResponderPermiso como la comprobación de control", () => {
    expect(puedeResponderPermiso(run({ controla: true, suscriptor: "sub-1" }))).toBe(true);
    expect(puedeResponderPermiso(run({ controla: false, suscriptor: "sub-1" }))).toBe(false);
  });
});

describe("cómo se presenta la petición", () => {
  it("traduce el riesgo sin suavizarlo", () => {
    expect(nombreRiesgo("critical")).toBe("Riesgo crítico");
    expect(nombreRiesgo("low")).toBe("Riesgo bajo");
  });

  it("deja pasar un riesgo que no conoce en vez de inventarse uno", () => {
    expect(nombreRiesgo("raro")).toBe("raro");
  });

  it("marca lo destructivo y lo caducado", () => {
    const etiquetas = etiquetasPermiso(
      permiso({ riesgo: "high", destructivo: true, caducado: true })
    );

    expect(etiquetas).toEqual(["Riesgo alto", "Destructiva", "Caducada"]);
  });

  it("nombra un argumento oculto en vez de pintarlo", () => {
    const texto = textoArgumento({
      nombre: "token",
      valor: "[REDACTED]",
      redactado: true,
      resumido: false
    });

    expect(texto).toBe("(oculto por seguridad)");
    expect(texto).not.toContain("REDACTED");
  });

  it("dice cuánto se quedó fuera de un valor resumido", () => {
    // Sin el tamaño, «fn main() {…» parecería el fichero entero.
    const texto = textoArgumento({
      nombre: "content",
      valor: "fn main() {",
      caracteres: 4096,
      redactado: false,
      resumido: true
    });

    expect(texto).toContain("4096 caracteres");
  });

  it("enseña tal cual un valor corto", () => {
    const texto = textoArgumento({
      nombre: "path",
      valor: "src/main.rs",
      redactado: false,
      resumido: false
    });

    expect(texto).toBe("src/main.rs");
  });

  it("dice «sin tiempo» cuando el plazo se agotó", () => {
    expect(tiempoRestante(permiso({ segundosRestantes: 0, caducado: true }))).toBe("sin tiempo");
  });
});
