import { useEffect, useMemo, useState, type PointerEvent as ReactPointerEvent } from "react";

import type {
  AttachmentView,
  CustomGptView,
  ProjectSummary,
  WorkflowDefinition,
  WorkflowNode,
  WorkflowNodeKind,
  WorkflowRunView,
  WorkflowSummary,
  WorkflowView
} from "./domain";
import { customGptIconGlyph } from "./domain";
import { describeError } from "./errors";
import { MarkdownContent } from "./MarkdownContent";
import { platform } from "./platform";
import { describeWorkflowFailure } from "./workflowFailure";

type WorkflowStudioProps = {
  projects: ProjectSummary[];
  customGpts: CustomGptView[];
  onOpenBrokerCredential: () => void;
  onOpenAutomations: () => void;
};

const nodeWidth = 210;
const nodeHeight = 116;

const id = (prefix: string) =>
  `${prefix}_${globalThis.crypto?.randomUUID?.().replaceAll("-", "") ?? Date.now().toString(36)}`;

const nodeKindLabel: Record<WorkflowNodeKind, string> = {
  input: "Entrada",
  custom_gpt: "GPT personal",
  prompt: "Instrucción rápida",
  approval: "Aprobación",
  result: "Resultado"
};

const runStatusLabel: Record<WorkflowRunView["status"], string> = {
  queued: "Preparando",
  running: "En ejecución",
  waiting_approval: "Esperando aprobación",
  completed: "Completado",
  partial_failed: "Completado con una rama fallida",
  failed: "Fallido",
  cancelled: "Cancelado"
};

const nextHourInputValue = () => {
  const date = new Date(Date.now() + 60 * 60 * 1_000);
  date.setMinutes(0, 0, 0);
  const offset = date.getTimezoneOffset() * 60_000;
  return new Date(date.getTime() - offset).toISOString().slice(0, 16);
};

export function WorkflowStudio({ projects, customGpts, onOpenBrokerCredential, onOpenAutomations }: WorkflowStudioProps) {
  const [workflows, setWorkflows] = useState<WorkflowSummary[]>([]);
  const [selected, setSelected] = useState<WorkflowView | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [connectionSource, setConnectionSource] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newProjectId, setNewProjectId] = useState("");
  const [runInput, setRunInput] = useState("");
  const [currentRun, setCurrentRun] = useState<WorkflowRunView | null>(null);
  const [runs, setRuns] = useState<WorkflowRunView[]>([]);
  const [projectFiles, setProjectFiles] = useState<AttachmentView[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scheduleOpen, setScheduleOpen] = useState(false);
  const [scheduleAt, setScheduleAt] = useState(nextHourInputValue);
  const [scheduleExpression, setScheduleExpression] = useState<"once" | "daily" | "weekly">("once");
  const [scheduledCreated, setScheduledCreated] = useState(false);
  const [selectedGptContext, setSelectedGptContext] = useState<{
    customGptId: string;
    enabledKnowledge: number;
    readyFiles: number;
  } | null>(null);
  const [projectContextSummary, setProjectContextSummary] = useState<{
    projectId: string;
    hasInstructions: boolean;
    enabledMemories: number;
    memoryEnabled: boolean;
  } | null>(null);

  const selectedNode = selected?.definition.nodes.find((node) => node.id === selectedNodeId);
  const runFailure = currentRun ? describeWorkflowFailure(currentRun) : null;

  const refreshList = async (preferredId?: string) => {
    const items = await platform.listWorkflows();
    setWorkflows(items);
    const workflowId = preferredId ?? selected?.id ?? items[0]?.id;
    if (workflowId) {
      const view = await platform.getWorkflow(workflowId);
      setSelected(view);
      setRuns(await platform.listWorkflowRuns(workflowId));
    } else {
      setSelected(null);
      setRuns([]);
    }
  };

  useEffect(() => {
    void refreshList().catch((reason) => setError(describeError(reason)));
  }, []);

  useEffect(() => {
    const projectId = selected?.projectId;
    if (!projectId) {
      setProjectFiles([]);
      setProjectContextSummary(null);
      return;
    }
    let cancelled = false;
    setProjectFiles([]);
    setProjectContextSummary(null);
    void platform
      .getProjectKnowledge(projectId)
      .then((knowledge) => {
        if (cancelled) return;
        setProjectFiles(knowledge.files.filter((file) => file.ingestionStatus === "ready"));
        setProjectContextSummary({
          projectId,
          hasInstructions: Boolean(knowledge.project.instructions?.trim()),
          enabledMemories: knowledge.memoryEnabled
            ? knowledge.memories.filter((memory) => memory.enabled).length
            : 0,
          memoryEnabled: knowledge.memoryEnabled
        });
      })
      .catch((reason) => {
        if (!cancelled) setError(describeError(reason));
      });
    return () => { cancelled = true; };
  }, [selected?.projectId]);

  useEffect(() => {
    const customGptId = selectedNode?.kind === "custom_gpt" ? selectedNode.customGptId : undefined;
    if (!customGptId) {
      setSelectedGptContext(null);
      return;
    }
    let cancelled = false;
    setSelectedGptContext(null);
    void Promise.all([
      platform.getCustomGptKnowledge(customGptId),
      platform.listCustomGptFiles(customGptId)
    ])
      .then(([knowledge, files]) => {
        if (!cancelled) {
          setSelectedGptContext({
            customGptId,
            enabledKnowledge: knowledge.filter((item) => item.enabled).length,
            readyFiles: files.filter((file) => file.ingestionStatus === "ready").length
          });
        }
      })
      .catch((reason) => {
        if (!cancelled) setError(describeError(reason));
      });
    return () => { cancelled = true; };
  }, [selectedNode?.kind, selectedNode?.customGptId]);

  useEffect(() => {
    if (!currentRun || !["queued", "running"].includes(currentRun.status)) return;
    const timer = window.setInterval(() => {
      void platform
        .getWorkflowRun(currentRun.id)
        .then((run) => {
          setCurrentRun(run);
          if (!["queued", "running"].includes(run.status)) {
            void platform.listWorkflowRuns(run.workflowId).then(setRuns);
          }
        })
        .catch((reason) => setError(describeError(reason)));
    }, 1_000);
    return () => window.clearInterval(timer);
  }, [currentRun?.id, currentRun?.status]);

  const mutateDefinition = (change: (definition: WorkflowDefinition) => WorkflowDefinition) => {
    setSelected((current) => current ? { ...current, definition: change(current.definition) } : current);
    setDirty(true);
    setNotice(null);
  };

  const createWorkflow = async () => {
    if (!newName.trim()) return;
    setBusy("create");
    setError(null);
    try {
      const created = await platform.createWorkflow(newName, newProjectId || undefined);
      setNewName("");
      setSelected(created);
      setDirty(true);
      setSelectedNodeId(created.definition.nodes[0]?.id ?? null);
      await refreshList(created.id);
      setNotice("Flujo creado como borrador.");
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const openWorkflow = async (workflowId: string) => {
    setBusy("open");
    setError(null);
    try {
      const view = await platform.getWorkflow(workflowId);
      setSelected(view);
      setDirty(false);
      setSelectedNodeId(null);
      setConnectionSource(null);
      setRuns(await platform.listWorkflowRuns(workflowId));
      setCurrentRun(null);
      setScheduleOpen(false);
      setScheduledCreated(false);
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const addNode = (kind: Exclude<WorkflowNodeKind, "input">) => {
    if (!selected) return;
    const count = selected.definition.nodes.length;
    const node: WorkflowNode = {
      id: id("node"),
      kind,
      label: nodeKindLabel[kind],
      x: 290 + (count % 3) * 235,
      y: 55 + Math.floor(count / 3) * 155,
      customGptId: kind === "custom_gpt" ? customGpts[0]?.id : undefined,
      instruction: kind === "prompt" ? "" : undefined,
      attachmentIds: []
    };
    mutateDefinition((definition) => {
      const input = definition.nodes.find((item) => item.kind === "input");
      const result = definition.nodes.find((item) => item.kind === "result");
      const directEdge = input && result
        ? definition.edges.find((edge) => edge.source === input.id && edge.target === result.id)
        : undefined;

      // Make the first useful node immediately executable: insert it between the
      // starter Entrada and Resultado nodes instead of making the user rewire both.
      if (kind !== "result" && definition.nodes.length === 2 && directEdge && input && result) {
        node.x = 360;
        node.y = 55;
        return {
          nodes: [...definition.nodes, node],
          edges: [
            ...definition.edges.filter((edge) => edge.id !== directEdge.id),
            { id: id("edge"), source: input.id, target: node.id },
            { id: id("edge"), source: node.id, target: result.id }
          ]
        };
      }

      return { ...definition, nodes: [...definition.nodes, node] };
    });
    setSelectedNodeId(node.id);
  };

  const removeNode = (nodeId: string) => {
    mutateDefinition((definition) => ({
      nodes: definition.nodes.filter((node) => node.id !== nodeId),
      edges: definition.edges.filter((edge) => edge.source !== nodeId && edge.target !== nodeId)
    }));
    setSelectedNodeId(null);
    if (connectionSource === nodeId) setConnectionSource(null);
  };

  const connectTo = (targetId: string) => {
    if (!selected || !connectionSource || connectionSource === targetId) return;
    const exists = selected.definition.edges.some(
      (edge) => edge.source === connectionSource && edge.target === targetId
    );
    if (!exists) {
      mutateDefinition((definition) => ({
        ...definition,
        edges: [...definition.edges, { id: id("edge"), source: connectionSource, target: targetId }]
      }));
    }
    setConnectionSource(null);
  };

  const moveNode = (
    event: ReactPointerEvent<HTMLElement>,
    node: WorkflowNode
  ) => {
    if ((event.target as HTMLElement).closest("button, input, select, textarea")) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startY = event.clientY;
    const originX = node.x;
    const originY = node.y;
    const handleMove = (move: PointerEvent) => {
      mutateDefinition((definition) => ({
        ...definition,
        nodes: definition.nodes.map((item) =>
          item.id === node.id
            ? {
                ...item,
                x: Math.max(15, Math.min(970, originX + move.clientX - startX)),
                y: Math.max(15, Math.min(510, originY + move.clientY - startY))
              }
            : item
        )
      }));
    };
    const handleUp = () => {
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
    };
    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
  };

  const save = async (publish = false) => {
    if (!selected) return;
    setBusy(publish ? "publish" : "save");
    setError(null);
    try {
      let saved = await platform.saveWorkflow(
        selected.id,
        selected.name,
        selected.description ?? "",
        selected.projectId ?? undefined,
        selected.definition
      );
      if (publish) saved = await platform.publishWorkflow(saved.id);
      setSelected(saved);
      setDirty(!publish);
      await refreshList(saved.id);
      setNotice(publish ? `Versión ${saved.publishedVersionNo} publicada.` : "Borrador guardado.");
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const run = async () => {
    if (!selected || !runInput.trim()) return;
    setBusy("run");
    setError(null);
    try {
      const started = await platform.runWorkflow(selected.id, runInput);
      setCurrentRun(started);
      setRuns((items) => [started, ...items.filter((item) => item.id !== started.id)]);
      setNotice("Ejecución iniciada. Puedes seguir cada nodo en el lienzo.");
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const retryRun = async (runId: string) => {
    setBusy("retry");
    setError(null);
    try {
      const retried = await platform.retryWorkflowRun(runId);
      setCurrentRun(retried);
      setRuns((items) => [retried, ...items]);
      setNotice("Reintento iniciado conservando los nodos ya completados.");
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const scheduleWorkflow = async () => {
    if (!selected || !runInput.trim() || !scheduleAt) return;
    setBusy("schedule");
    setError(null);
    try {
      const dueAt = new Date(scheduleAt);
      if (!Number.isFinite(dueAt.getTime())) throw new Error("Selecciona una fecha y hora válidas.");
      await platform.createScheduledWorkflow(
        selected.name,
        selected.id,
        runInput,
        dueAt.toISOString(),
        Intl.DateTimeFormat().resolvedOptions().timeZone || "Atlantic/Canary",
        scheduleExpression
      );
      setScheduledCreated(true);
      setScheduleOpen(false);
      setNotice(`“${selected.name}” quedó programado. Puedes administrarlo en Automatizaciones.`);
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const decideApproval = async (nodeId: string, approved: boolean) => {
    if (!currentRun) return;
    setBusy(`approval-${nodeId}`);
    setError(null);
    try {
      const resumed = await platform.decideWorkflowApproval(currentRun.id, nodeId, approved);
      setCurrentRun(resumed);
      setNotice(approved
        ? "Rama aprobada. El flujo continúa desde este punto."
        : "Rama rechazada. Las ramas independientes continuarán.");
    } catch (reason) {
      setError(describeError(reason));
    } finally {
      setBusy(null);
    }
  };

  const nodeRunById = useMemo(
    () => new Map(currentRun?.nodeRuns.map((run) => [run.nodeId, run]) ?? []),
    [currentRun]
  );

  return (
    <section className="workflow-studio" aria-labelledby="workflow-heading">
      <div className="workflow-studio-heading">
        <div>
          <span className="kicker">Orquestación visual</span>
          <h2 id="workflow-heading">Flujos</h2>
          <p>Conecta GPTs e instrucciones para que una salida se convierta en la entrada de los siguientes nodos.</p>
        </div>
        <span className="badge">{workflows.length} flujo(s)</span>
      </div>

      {error && <div className="workflow-message error" role="alert"><strong>No se pudo completar la acción</strong><span>{error}</span></div>}
      {notice && <div className="workflow-message success" role="status">{notice}</div>}

      <div className="workflow-layout">
        <aside className="workflow-library">
          <h3>Mis flujos</h3>
          <label>
            <span>Nombre del flujo</span>
            <input value={newName} onChange={(event) => setNewName(event.target.value)} placeholder="Ejemplo: Investigar y revisar" />
          </label>
          <label>
            <span>Proyecto (opcional)</span>
            <select value={newProjectId} onChange={(event) => setNewProjectId(event.target.value)}>
              <option value="">Global</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
            </select>
          </label>
          <button className="primary" onClick={() => void createWorkflow()} disabled={!newName.trim() || busy !== null}>
            {busy === "create" ? "Creando…" : "Crear flujo"}
          </button>
          <div className="workflow-list">
            {workflows.map((workflow) => (
              <button key={workflow.id} className={selected?.id === workflow.id ? "active" : ""} onClick={() => void openWorkflow(workflow.id)}>
                <strong>{workflow.name}</strong>
                <span>{workflow.nodeCount} nodos · {workflow.publishedVersionNo ? `versión ${workflow.publishedVersionNo}` : "borrador"}</span>
              </button>
            ))}
            {workflows.length === 0 && <p>Crea el primer flujo para abrir el editor.</p>}
          </div>
        </aside>

        {selected ? (
          <div className="workflow-editor-shell">
            <header className="workflow-editor-header">
              <label><span>Nombre</span><input value={selected.name} onChange={(event) => { setSelected({ ...selected, name: event.target.value }); setDirty(true); }} /></label>
              <label><span>Descripción</span><input value={selected.description ?? ""} onChange={(event) => { setSelected({ ...selected, description: event.target.value }); setDirty(true); }} placeholder="Qué resuelve este flujo" /></label>
              <label><span>Proyecto</span><select value={selected.projectId ?? ""} onChange={(event) => { setSelected({ ...selected, projectId: event.target.value || null, definition: { ...selected.definition, projectContext: null, nodes: selected.definition.nodes.map((node) => ({ ...node, attachmentIds: [] })) } }); setDirty(true); }}><option value="">Global</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</select></label>
              <div className="workflow-editor-actions">
                <button className="secondary" onClick={() => void save()} disabled={busy !== null}>{busy === "save" ? "Guardando…" : "Guardar borrador"}</button>
                <button className="secondary" onClick={() => void save(true)} disabled={busy !== null}>{busy === "publish" ? "Publicando…" : "Publicar versión"}</button>
              </div>
            </header>

            <div className="workflow-toolbar" aria-label="Añadir nodos">
              <span>Añadir:</span>
              <button onClick={() => addNode("custom_gpt")} disabled={customGpts.length === 0}>GPT personal</button>
              <button onClick={() => addNode("prompt")}>Instrucción rápida</button>
              <button onClick={() => addNode("approval")}>Aprobación</button>
              <button onClick={() => addNode("result")}>Resultado</button>
              {connectionSource && <strong>Selecciona la entrada del nodo de destino</strong>}
            </div>

            <div className="workflow-canvas" aria-label="Editor gráfico del flujo">
              <svg aria-hidden="true" width="1200" height="650">
                <defs><marker id="workflow-arrow" markerWidth="8" markerHeight="8" refX="7" refY="4" orient="auto"><path d="M0,0 L8,4 L0,8 z" /></marker></defs>
                {selected.definition.edges.map((edge) => {
                  const source = selected.definition.nodes.find((node) => node.id === edge.source);
                  const target = selected.definition.nodes.find((node) => node.id === edge.target);
                  if (!source || !target) return null;
                  const x1 = source.x + nodeWidth;
                  const y1 = source.y + nodeHeight / 2;
                  const x2 = target.x;
                  const y2 = target.y + nodeHeight / 2;
                  const bend = Math.max(45, Math.abs(x2 - x1) / 2);
                  return <path key={edge.id} d={`M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`} markerEnd="url(#workflow-arrow)" />;
                })}
              </svg>
              {selected.definition.nodes.map((node) => {
                const nodeRun = nodeRunById.get(node.id);
                const nodeGpt = node.kind === "custom_gpt"
                  ? customGpts.find((gpt) => gpt.id === node.customGptId)
                  : undefined;
                const nodeGptIcon = node.kind === "custom_gpt"
                  ? customGptIconGlyph(node.customGptIconRef ?? nodeGpt?.iconRef)
                  : null;
                return (
                  <article
                    key={node.id}
                    className={`workflow-node workflow-node-${node.kind} ${selectedNodeId === node.id ? "selected" : ""} ${nodeRun ? `run-${nodeRun.status}` : ""}`}
                    style={{ left: node.x, top: node.y }}
                    onPointerDown={(event) => moveNode(event, node)}
                    onClick={() => setSelectedNodeId(node.id)}
                    onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); setSelectedNodeId(node.id); } }}
                    tabIndex={0}
                  >
                    {node.kind !== "input" && <button className="workflow-port input" aria-label={`Conectar con ${node.label}`} onClick={(event) => { event.stopPropagation(); connectTo(node.id); }}>●</button>}
                    <span>{nodeGptIcon && <b className="workflow-node-gpt-icon" aria-hidden="true">{nodeGptIcon}</b>}{nodeKindLabel[node.kind]}</span>
                    <strong>{node.label}</strong>
                    <small>{nodeRun ? runStatusLabelForNode(nodeRun.status) : node.kind === "custom_gpt" ? nodeGpt?.name ?? node.customGptName ?? "Selecciona un GPT" : node.kind === "prompt" ? node.instruction || "Escribe una instrucción" : nodeKindLabel[node.kind]}</small>
                    {node.kind !== "result" && <button className={`workflow-port output ${connectionSource === node.id ? "active" : ""}`} aria-label={`Conectar salida de ${node.label}`} onClick={(event) => { event.stopPropagation(); setConnectionSource(connectionSource === node.id ? null : node.id); }}>●</button>}
                  </article>
                );
              })}
            </div>

            <div className="workflow-lower-grid">
              <section className="workflow-inspector">
                <h3>Configuración del nodo</h3>
                {selectedNode ? (
                  <>
                    <label><span>Nombre visible</span><input value={selectedNode.label} onChange={(event) => mutateDefinition((definition) => ({ ...definition, nodes: definition.nodes.map((node) => node.id === selectedNode.id ? { ...node, label: event.target.value } : node) }))} /></label>
                    {selectedNode.kind === "custom_gpt" && <label><span>GPT personal</span><select value={selectedNode.customGptId ?? ""} onChange={(event) => mutateDefinition((definition) => ({ ...definition, nodes: definition.nodes.map((node) => node.id === selectedNode.id ? { ...node, customGptId: event.target.value, customGptVersionId: null, customGptName: null, customGptIconRef: null, customGptInstructions: null, customGptMemoryIds: [], customGptAttachmentIds: [] } : node) }))}><option value="">Selecciona un GPT</option>{customGpts.map((gpt) => <option key={gpt.id} value={gpt.id}>{customGptIconGlyph(gpt.iconRef)} {gpt.name}</option>)}</select></label>}
                    {selectedNode.kind === "custom_gpt" && selectedNode.customGptId && (
                      <div className="workflow-gpt-context" aria-live="polite">
                        {selectedGptContext?.customGptId === selectedNode.customGptId ? (
                          <>
                            <strong>Contexto propio al publicar</strong>
                            <span>{selectedGptContext.enabledKnowledge} dato(s) y {selectedGptContext.readyFiles} archivo(s) preparado(s)</span>
                            <small>Los cambios nuevos requieren volver a publicar. Si desactivas o eliminas algo, deja de usarse también en versiones anteriores.</small>
                          </>
                        ) : <span>Comprobando el contexto propio del GPT…</span>}
                      </div>
                    )}
                    {!['input', 'approval', 'result'].includes(selectedNode.kind) && selected.projectId && (
                      <div className="workflow-project-context" aria-live="polite">
                        {projectContextSummary?.projectId === selected.projectId ? (
                          <>
                            <strong>Contexto del proyecto al publicar</strong>
                            <span>{projectContextSummary.hasInstructions ? "Instrucciones incluidas" : "Sin instrucciones"} · {projectContextSummary.enabledMemories} recuerdo(s) activo(s)</span>
                            {!projectContextSummary.memoryEnabled && <small>La memoria general está desactivada; los recuerdos del proyecto no se incluirán.</small>}
                            <small>Los cambios nuevos requieren volver a publicar. Retirar instrucciones o recuerdos revoca su uso en versiones anteriores.</small>
                          </>
                        ) : <span>Comprobando el contexto del proyecto…</span>}
                      </div>
                    )}
                    {selectedNode.kind === "prompt" && <label><span>Instrucción</span><textarea rows={5} value={selectedNode.instruction ?? ""} onChange={(event) => mutateDefinition((definition) => ({ ...definition, nodes: definition.nodes.map((node) => node.id === selectedNode.id ? { ...node, instruction: event.target.value } : node) }))} placeholder="Qué debe hacer este nodo con las salidas recibidas" /></label>}
                    {!["input", "approval", "result"].includes(selectedNode.kind) && (
                      <fieldset className="workflow-file-picker"><legend>Archivos disponibles para este GPT</legend>{selected.projectId ? projectFiles.length > 0 ? projectFiles.map((file) => <label key={file.id}><input type="checkbox" checked={selectedNode.attachmentIds.includes(file.id)} onChange={(event) => mutateDefinition((definition) => ({ ...definition, nodes: definition.nodes.map((node) => node.id === selectedNode.id ? { ...node, attachmentIds: event.target.checked ? [...node.attachmentIds, file.id] : node.attachmentIds.filter((item) => item !== file.id) } : node) }))} /><span>{file.displayName}</span></label>) : <p>No hay archivos preparados en el proyecto.</p> : <p>Asocia el flujo a un proyecto para elegir sus archivos.</p>}</fieldset>
                    )}
                    {selectedNode.kind !== "input" && <button className="danger-link" onClick={() => removeNode(selectedNode.id)}>Eliminar nodo</button>}
                    <div className="workflow-edge-list"><strong>Conexiones de este nodo</strong>{selected.definition.edges.filter((edge) => edge.source === selectedNode.id || edge.target === selectedNode.id).map((edge) => { const otherId = edge.source === selectedNode.id ? edge.target : edge.source; const other = selected.definition.nodes.find((node) => node.id === otherId); return <div key={edge.id}><span>{edge.source === selectedNode.id ? "Hacia" : "Desde"} {other?.label}</span><button aria-label="Eliminar conexión" onClick={() => mutateDefinition((definition) => ({ ...definition, edges: definition.edges.filter((item) => item.id !== edge.id) }))}>×</button></div>; })}</div>
                  </>
                ) : <p>Selecciona un nodo para editarlo.</p>}
              </section>

              <section className="workflow-run-panel">
                <h3>Probar flujo</h3>
                <textarea rows={5} value={runInput} onChange={(event) => setRunInput(event.target.value)} placeholder="Escribe la entrada inicial del flujo" />
                <div className="workflow-run-actions">
                  <button className="primary" onClick={() => void run()} disabled={!runInput.trim() || !selected.publishedVersionNo || dirty || busy !== null}>{busy === "run" ? "Iniciando…" : "Ejecutar flujo"}</button>
                  <button className="secondary" onClick={() => { setScheduleOpen((open) => !open); setScheduledCreated(false); }} disabled={!selected.publishedVersionNo || dirty || busy !== null}>{scheduleOpen ? "Cerrar programación" : "Programar"}</button>
                  {currentRun && ["queued", "running", "waiting_approval"].includes(currentRun.status) && <button className="danger-link" onClick={() => void platform.cancelWorkflowRun(currentRun.id).then(setCurrentRun)}>Cancelar</button>}
                </div>
                {(!selected.publishedVersionNo || dirty) && <small>{dirty ? "Publica los cambios para ejecutar exactamente lo que ves." : "Publica una versión antes de ejecutar."}</small>}
                {scheduleOpen && (
                  <div className="workflow-schedule-panel">
                    <div className="workflow-schedule-heading">
                      <div><strong>Ejecutar automáticamente</strong><p>Usará la versión {selected.publishedVersionNo} y la entrada escrita arriba.</p></div>
                      <span className="badge">Local</span>
                    </div>
                    <div className="workflow-schedule-fields">
                      <label><span>Primera ejecución</span><input type="datetime-local" value={scheduleAt} min={nextHourInputValue()} onChange={(event) => setScheduleAt(event.target.value)} /></label>
                      <label><span>Repetición</span><select value={scheduleExpression} onChange={(event) => setScheduleExpression(event.target.value as typeof scheduleExpression)}><option value="once">Una vez</option><option value="daily">Cada día</option><option value="weekly">Cada semana</option></select></label>
                    </div>
                    <p className="workflow-schedule-disclosure">ChatyGPT debe estar abierto —o configurado para iniciarse con Windows— cuando llegue la hora. Si el flujo alcanza una aprobación, se detendrá hasta que la revises en Flujos.</p>
                    <button className="primary" onClick={() => void scheduleWorkflow()} disabled={!runInput.trim() || !scheduleAt || busy !== null}>{busy === "schedule" ? "Programando…" : "Confirmar programación"}</button>
                  </div>
                )}
                {scheduledCreated && <button className="workflow-automation-link" onClick={onOpenAutomations}>Ver en Automatizaciones →</button>}
                {currentRun && <div className={`workflow-run-summary status-${currentRun.status}`}>
                  <strong>{runStatusLabel[currentRun.status]}</strong>
                  <span>Versión {currentRun.versionNo}</span>
                  {runFailure && ["failed", "partial_failed"].includes(currentRun.status) && (
                    <div className={`workflow-failure-card kind-${runFailure.kind}`} role="alert">
                      <div>
                        <strong>{runFailure.title}</strong>
                        <p>{runFailure.guidance}</p>
                      </div>
                      {runFailure.kind === "credential" && <button className="primary" onClick={onOpenBrokerCredential}>Renovar credencial</button>}
                      <details>
                        <summary>Ver detalle del fallo</summary>
                        {runFailure.failedNodes.length > 0 && (
                          <ul>{runFailure.failedNodes.map((node) => <li key={node.id}><strong>{node.label}:</strong> {node.message}</li>)}</ul>
                        )}
                        {runFailure.failedNodes.every((node) => node.message !== runFailure.technicalMessage) && <code>{runFailure.technicalMessage}</code>}
                      </details>
                    </div>
                  )}
                  {currentRun.nodeRuns.filter((nodeRun) => nodeRun.status === "waiting_approval").map((nodeRun) => (
                    <div className="workflow-approval-decision" key={nodeRun.id}>
                      <div>
                        <strong>{nodeRun.nodeLabel}</strong>
                        <p>Revisa la información recibida por este punto antes de decidir si la rama puede continuar.</p>
                        {nodeRun.inputText && <details><summary>Ver contenido pendiente</summary><MarkdownContent text={nodeRun.inputText} /></details>}
                      </div>
                      <div className="workflow-approval-actions">
                        <button className="primary" onClick={() => void decideApproval(nodeRun.nodeId, true)} disabled={busy !== null}>Aprobar y continuar</button>
                        <button className="danger-link" onClick={() => void decideApproval(nodeRun.nodeId, false)} disabled={busy !== null}>Rechazar rama</button>
                      </div>
                    </div>
                  ))}
                  {["failed", "partial_failed"].includes(currentRun.status) && <button className="secondary" onClick={() => void retryRun(currentRun.id)} disabled={busy !== null}>{busy === "retry" ? "Reintentando…" : runFailure?.kind === "credential" ? "Ya la renové: reintentar" : "Reintentar desde el fallo"}</button>}
                  {Object.entries(currentRun.outputs).map(([label, output]) => <details key={label} open><summary>{label}</summary><MarkdownContent text={output} /></details>)}
                </div>}
                {runs.length > 0 && <details className="workflow-run-history"><summary>Historial reciente ({runs.length})</summary>{runs.map((runItem) => <button key={runItem.id} onClick={() => setCurrentRun(runItem)}><span>{runStatusLabel[runItem.status]}</span><time>{new Date(runItem.updatedAt).toLocaleString("es-ES")}</time></button>)}</details>}
              </section>
            </div>
          </div>
        ) : (
          <div className="workflow-empty"><strong>Diseña tu primer flujo</strong><p>Crea un flujo a la izquierda. Empezará con un nodo Entrada conectado a un nodo Resultado.</p></div>
        )}
      </div>
    </section>
  );
}

function runStatusLabelForNode(status: WorkflowRunView["nodeRuns"][number]["status"]) {
  return {
    pending: "Pendiente",
    running: "Ejecutándose…",
    waiting_approval: "Esperando aprobación",
    completed: "Completado",
    failed: "Fallido",
    skipped: "Omitido por una entrada fallida",
    cancelled: "Cancelado"
  }[status];
}
