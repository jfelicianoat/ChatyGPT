/**
 * Tarjeta de los GPT personales.
 *
 * Recibe el hook completo (`useGpts`) en vez de un centenar de props sueltas:
 * el estado del formulario y las acciones que lo mueven ya viajan juntos, y
 * asi la tarjeta no necesita una interfaz paralela que mantener a mano.
 */
import {
  attachmentContextSummary,
  attachmentImagePolicyLabel,
  attachmentStatusLabel,
  customGptIconGlyph,
  customGptIconOptions,
  customGptVersionSummary,
  type ConversationExecutionPreferences,
  type CustomGptApiAction,
  type CustomGptView,
  type MemoryItemView,
  type ProjectSummary
} from "../domain";
import type { DialogState } from "../dialogs";
import type { useGpts } from "./useGpts";

type Props = {
  gpts: ReturnType<typeof useGpts>;
  projects: ProjectSummary[];
  openDialog: (dialogo: DialogState) => void;
};

export function TarjetaGpts({ gpts, projects, openDialog }: Props) {
  const {
    customGpts,
    customGptEditingId,
    customGptName,
    setCustomGptName,
    customGptDescription,
    setCustomGptDescription,
    customGptIcon,
    setCustomGptIcon,
    customGptInstructions,
    setCustomGptInstructions,
    customGptStartersText,
    setCustomGptStartersText,
    customGptRunCodePermission,
    setCustomGptRunCodePermission,
    customGptRenamePermission,
    setCustomGptRenamePermission,
    customGptFolderReadPermission,
    setCustomGptFolderReadPermission,
    customGptFileModifyPermission,
    setCustomGptFileModifyPermission,
    customGptSchedulePermission,
    setCustomGptSchedulePermission,
    customGptExternalApiPermission,
    setCustomGptExternalApiPermission,
    customGptApiActions,
    setCustomGptApiActions,
    customGptApiSamples,
    setCustomGptApiSamples,
    customGptApiPreviews,
    customGptApiTests,
    apiCredentials,
    apiCredentialDrafts,
    setApiCredentialDrafts,
    apiCredentialBusy,
    customGptPreferredModel,
    setCustomGptPreferredModel,
    customGptDefaultProject,
    setCustomGptDefaultProject,
    customGptOwnExecution,
    setCustomGptOwnExecution,
    customGptContextProfile,
    setCustomGptContextProfile,
    customGptDataClassification,
    setCustomGptDataClassification,
    customGptStrategy,
    setCustomGptStrategy,
    customGptPreset,
    setCustomGptPreset,
    customGptMaxCost,
    setCustomGptMaxCost,
    customGptLongContext,
    setCustomGptLongContext,
    customGptPriority,
    setCustomGptPriority,
    customGptHistoryId,
    customGptVersions,
    customGptBusy,
    customGptError,
    customGptNotice,
    customGptKnowledge,
    setCustomGptKnowledge,
    customGptFiles,
    setCustomGptFiles,
    customGptKnowledgeDraft,
    setCustomGptKnowledgeDraft,
    customGptKnowledgeCategory,
    setCustomGptKnowledgeCategory,
    customGptKnowledgeSensitive,
    setCustomGptKnowledgeSensitive,
    customGptKnowledgeBusy,
    customGptKnowledgeNotice,
    resetCustomGptForm,
    loadCustomGptVersions,
    restoreCustomGptVersion,
    openCustomGptPreview,
    duplicateCustomGpt,
    beginCustomGptEdit,
    saveCustomGpt,
    importCustomGpt,
    exportCustomGpt,
    openCustomGptKnowledge,
    importCustomGptFiles,
    removeCustomGptFile,
    createCustomGptKnowledge,
    toggleCustomGptKnowledgeItem,
    removeCustomGptKnowledgeItem,
    reindexCustomGptKnowledgeItem,
    previewApiAction,
    testApiAction,
    saveApiCredential,
    removeApiCredential
  } = gpts;

  return (
  <section className="custom-gpt-card">
    <div className="panel-heading">
      <div>
        <span className="kicker">Fase 3 · Asistentes personales</span>
        <h3>Mis GPTs</h3>
      </div>
      <div className="custom-gpt-panel-actions">
        {customGpts.state === "ready" && (
          <span className="badge">
            {customGpts.value.length} guardado(s)
          </span>
        )}
        <button
          className="secondary"
          onClick={() => void importCustomGpt()}
          disabled={customGptBusy}
        >
          Importar GPT
        </button>
      </div>
    </div>
    <p className="muted">
      Define asistentes reutilizables con propuestas para empezar. Cada cambio crea
      una versión nueva y conserva localmente las anteriores.
    </p>
    <div className="custom-gpt-safety">
      Elegir un GPT aplica sus instrucciones a los mensajes de ese chat. No concede
      herramientas ni ejecuta acciones por sí mismo.
    </div>
    <div className="custom-gpt-form">
      <div className="custom-gpt-form-heading">
        <strong>
          {customGptEditingId ? "Editar GPT personal" : "Crear GPT personal"}
        </strong>
        {customGptEditingId && <span>Se guardará como una versión nueva</span>}
      </div>
      <label>
        <span>Nombre</span>
        <input
          value={customGptName}
          onChange={(event) => setCustomGptName(event.target.value)}
          placeholder="Ejemplo: Tutor de arquitectura"
          maxLength={80}
          disabled={customGptBusy}
        />
      </label>
      <fieldset className="custom-gpt-icon-picker">
        <legend>Icono</legend>
        <div role="group" aria-label="Icono del GPT personal">
          {customGptIconOptions.map((option) => (
            <button
              key={option.id}
              type="button"
              className={customGptIcon === option.id ? "selected" : ""}
              aria-pressed={customGptIcon === option.id}
              aria-label={`Icono ${option.label}`}
              onClick={() => setCustomGptIcon(option.id)}
              disabled={customGptBusy}
            >
              <span aria-hidden="true">{option.glyph}</span>
              <small>{option.label}</small>
            </button>
          ))}
        </div>
        <small>Se guarda con cada versión y ayuda a reconocerlo en chats y flujos.</small>
      </fieldset>
      <label>
        <span>Descripción breve (opcional)</span>
        <textarea
          value={customGptDescription}
          onChange={(event) => setCustomGptDescription(event.target.value)}
          placeholder="Explica para qué sirve este GPT."
          rows={2}
          maxLength={500}
          disabled={customGptBusy}
        />
      </label>
      <label>
        <span>Instrucciones</span>
        <textarea
          value={customGptInstructions}
          onChange={(event) => setCustomGptInstructions(event.target.value)}
          placeholder="Describe cómo debe responder, qué debe priorizar y qué límites debe respetar."
          rows={5}
          maxLength={12000}
          disabled={customGptBusy}
        />
      </label>
      <label>
        <span>Iniciadores de conversación (opcional)</span>
        <textarea
          value={customGptStartersText}
          onChange={(event) => setCustomGptStartersText(event.target.value)}
          placeholder={"Una propuesta por línea, hasta 6.\nEjemplo: Explícame este tema paso a paso"}
          rows={4}
          maxLength={1805}
          disabled={customGptBusy}
        />
        <small>
          Aparecerán como botones al abrir un chat vacío que use este GPT.
        </small>
      </label>
      <fieldset className="custom-gpt-permissions">
        <legend>Permisos de herramientas</legend>
        <p>
          Todo está denegado por defecto. Activar una capacidad solo permite
          solicitarla: ChatyGPT seguirá pidiendo tu confirmación.
        </p>
        <label>
          <input
            type="checkbox"
            checked={customGptRunCodePermission}
            onChange={(event) =>
              setCustomGptRunCodePermission(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Código aislado
            <small>Puede solicitar Python para un turno; nunca se activa solo.</small>
          </span>
        </label>
        <label>
          <input
            type="checkbox"
            checked={customGptRenamePermission}
            onChange={(event) =>
              setCustomGptRenamePermission(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Renombrar conversación
            <small>Puede proponer un título; tendrás que aprobarlo.</small>
          </span>
        </label>
        <label>
          <input
            type="checkbox"
            checked={customGptFolderReadPermission}
            onChange={(event) =>
              setCustomGptFolderReadPermission(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Leer carpetas autorizadas
            <small>Puede listar y leer texto; confirmarás cada operación.</small>
          </span>
        </label>
        <label>
          <input
            type="checkbox"
            checked={customGptFileModifyPermission}
            onChange={(event) => {
              setCustomGptFileModifyPermission(event.target.checked);
              if (event.target.checked) setCustomGptFolderReadPermission(true);
            }}
            disabled={customGptBusy}
          />
          <span>
            Modificar archivos autorizados
            <small>Solo reemplaza texto existente y confirmarás cada cambio.</small>
          </span>
        </label>
        <label>
          <input
            type="checkbox"
            checked={customGptSchedulePermission}
            onChange={(event) =>
              setCustomGptSchedulePermission(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Crear tareas programadas
            <small>Puede proponer una ejecución futura; confirmarás antes de activarla.</small>
          </span>
        </label>
        <label>
          <input
            type="checkbox"
            checked={customGptExternalApiPermission}
            onChange={(event) =>
              setCustomGptExternalApiPermission(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Consultar APIs externas
            <small>Solo HTTPS GET público; puede usar una credencial protegida y confirmarás cada URL.</small>
          </span>
        </label>
        {customGptExternalApiPermission && (
          <div className="custom-gpt-api-actions">
            <strong>Acciones API configuradas</strong>
            <small>
              Define destinos reutilizables. Los parámetros se enviarán como consulta y verás una confirmación cada vez.
            </small>
            {customGptApiActions.map((action, index) => (
              <div className="custom-gpt-api-action" key={`${action.name}-${index}`}>
                <label>
                  <span>Nombre interno</span>
                  <input value={action.name} placeholder="consultar_tiempo" maxLength={40}
                    onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, name: event.target.value } : item))} />
                </label>
                <label>
                  <span>Descripción</span>
                  <input value={action.description} placeholder="Consulta la previsión pública de una ciudad" maxLength={300}
                    onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, description: event.target.value } : item))} />
                </label>
                <label>
                  <span>URL HTTPS fija</span>
                  <input value={action.url} placeholder="https://api.example.com/weather" spellCheck={false}
                    onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, url: event.target.value } : item))} />
                </label>
                <section className="custom-gpt-api-credential" aria-label={`Autenticación de ${action.name || `la acción ${index + 1}`}`}>
                  <div className="custom-gpt-api-credential-heading">
                    <strong>Autenticación</strong>
                    <small>La clave se cifra en este equipo y nunca se entrega al modelo.</small>
                  </div>
                  <label>
                    <span>Tipo</span>
                    <select
                      aria-label={`Tipo de autenticación de la acción ${index + 1}`}
                      value={action.authMode ?? "none"}
                      onChange={(event) => {
                        const authMode = event.currentTarget.value as CustomGptApiAction["authMode"];
                        setCustomGptApiActions((current) => current.map((item, itemIndex) =>
                          itemIndex === index
                            ? { ...item, authMode, credentialRef: authMode === "none" ? undefined : item.credentialRef }
                            : item
                        ));
                      }}
                    >
                      <option value="none">Sin credencial</option>
                      <option value="bearer">Token Bearer</option>
                      <option value="api_key">Clave API (X-API-Key)</option>
                    </select>
                  </label>
                  {(action.authMode ?? "none") !== "none" && (
                    <>
                      <label>
                        <span>Alias de la credencial</span>
                        <input
                          value={action.credentialRef ?? ""}
                          list={`api-credential-aliases-${index}`}
                          placeholder="mi_servicio"
                          maxLength={40}
                          spellCheck={false}
                          autoComplete="off"
                          onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) =>
                            itemIndex === index ? { ...item, credentialRef: event.target.value.toLowerCase() } : item
                          ))}
                        />
                        <datalist id={`api-credential-aliases-${index}`}>
                          {apiCredentials.state === "ready" && apiCredentials.value.map((credential) => (
                            <option value={credential.name} key={credential.name} />
                          ))}
                        </datalist>
                        <small>Entre 3 y 40 caracteres: empieza por letra y usa minúsculas, números o guion bajo.</small>
                      </label>
                      <label>
                        <span>Clave o token</span>
                        <input
                          type="password"
                          value={apiCredentialDrafts[index] ?? ""}
                          placeholder="Pega aquí la clave para guardarla"
                          autoComplete="new-password"
                          onChange={(event) => setApiCredentialDrafts((current) => ({ ...current, [index]: event.target.value }))}
                        />
                        <small>Por seguridad, ChatyGPT no vuelve a mostrar una clave guardada.</small>
                      </label>
                      <div className="custom-gpt-api-credential-actions">
                        <button
                          type="button"
                          className="ghost"
                          disabled={apiCredentialBusy === index || !(action.credentialRef?.trim()) || !(apiCredentialDrafts[index]?.trim())}
                          onClick={() => void saveApiCredential(index)}
                        >
                          {apiCredentialBusy === index ? "Guardando…" : "Guardar credencial"}
                        </button>
                        {apiCredentials.state === "ready" && apiCredentials.value.some((credential) => credential.name === action.credentialRef?.trim()) && (
                          <>
                            <span className="custom-gpt-api-credential-saved" role="status">Guardada y cifrada</span>
                            <button
                              type="button"
                              className="ghost danger"
                              disabled={apiCredentialBusy === index}
                              onClick={() => void removeApiCredential(index)}
                            >
                              Retirar del equipo
                            </button>
                          </>
                        )}
                      </div>
                    </>
                  )}
                </section>
                <div className="custom-gpt-api-parameters">
                  <strong>Parámetros</strong>
                  {action.parameters.map((parameter, parameterIndex) => (
                    <div className="custom-gpt-api-parameter" key={`${parameter.name}-${parameterIndex}`}>
                      <input aria-label={`Nombre del parámetro ${parameterIndex + 1}`} value={parameter.name} placeholder="city" maxLength={40}
                        onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.map((value, valueIndex) => valueIndex === parameterIndex ? { ...value, name: event.target.value } : value) } : item))} />
                      <select aria-label={`Tipo del parámetro ${parameterIndex + 1}`} value={parameter.type}
                        onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.map((value, valueIndex) => valueIndex === parameterIndex ? { ...value, type: event.target.value as typeof value.type } : value) } : item))}>
                        <option value="string">Texto</option><option value="number">Número</option><option value="boolean">Sí / no</option>
                      </select>
                      <select aria-label={`Ubicación del parámetro ${parameterIndex + 1}`} value={parameter.location}
                        onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.map((value, valueIndex) => valueIndex === parameterIndex ? { ...value, location: event.target.value as typeof value.location, required: event.target.value === "path" ? true : value.required } : value) } : item))}>
                        <option value="query">Consulta</option><option value="path">Ruta</option>
                      </select>
                      <label><input type="checkbox" checked={parameter.required}
                        disabled={parameter.location === "path"}
                        onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.map((value, valueIndex) => valueIndex === parameterIndex ? { ...value, required: event.target.checked } : value) } : item))} /> Obligatorio</label>
                      <input aria-label={`Explicación del parámetro ${parameterIndex + 1}`} value={parameter.description ?? ""} placeholder="Ciudad que se quiere consultar" maxLength={160}
                        onChange={(event) => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.map((value, valueIndex) => valueIndex === parameterIndex ? { ...value, description: event.target.value } : value) } : item))} />
                      <button type="button" className="ghost danger" onClick={() => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: item.parameters.filter((_, valueIndex) => valueIndex !== parameterIndex) } : item))}>Quitar</button>
                    </div>
                  ))}
                  <button type="button" className="ghost" disabled={action.parameters.length >= 8}
                    onClick={() => setCustomGptApiActions((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, parameters: [...item.parameters, { name: "", type: "string", required: true, location: "query", description: "" }] } : item))}>+ Añadir parámetro</button>
                </div>
                <div className="custom-gpt-api-tester">
                  <strong>Previsualizar llamada</strong>
                  <small>No conecta con la API ni genera coste.</small>
                  <div className="custom-gpt-api-samples">
                    {action.parameters.map((parameter) => (
                      <label key={parameter.name || "parameter"}>
                        <span>{parameter.name || "Parámetro sin nombre"}{parameter.required ? " *" : ""}</span>
                        {parameter.type === "boolean" ? (
                          <select value={customGptApiSamples[index]?.[parameter.name] ?? "false"}
                            onChange={(event) => setCustomGptApiSamples((current) => ({ ...current, [index]: { ...current[index], [parameter.name]: event.target.value } }))}>
                            <option value="false">No</option><option value="true">Sí</option>
                          </select>
                        ) : (
                          <input type={parameter.type === "number" ? "number" : "text"}
                            value={customGptApiSamples[index]?.[parameter.name] ?? ""}
                            onChange={(event) => setCustomGptApiSamples((current) => ({ ...current, [index]: { ...current[index], [parameter.name]: event.target.value } }))} />
                        )}
                      </label>
                    ))}
                  </div>
                  <div className="custom-gpt-api-test-actions">
                    <button type="button" className="ghost" onClick={() => void previewApiAction(index)}>Previsualizar</button>
                    <button type="button" className="ghost" disabled={customGptApiTests[index]?.loading}
                      onClick={() => void testApiAction(index)}>
                      {customGptApiTests[index]?.loading ? "Probando…" : "Probar conexión"}
                    </button>
                  </div>
                  {customGptApiPreviews[index]?.error && <p className="error" role="alert">{customGptApiPreviews[index].error}</p>}
                  {customGptApiPreviews[index]?.value && (
                    <div className="custom-gpt-api-preview" role="status">
                      <span>{customGptApiPreviews[index].value?.method} · {customGptApiPreviews[index].value?.destination}</span>
                      <code>{customGptApiPreviews[index].value?.finalUrl}</code>
                      <small>{customGptApiPreviews[index].value?.dataSent.length ?? 0} dato(s) aparecerán en la confirmación.</small>
                    </div>
                  )}
                  {customGptApiTests[index]?.error && <p className="error" role="alert">{customGptApiTests[index].error}</p>}
                  {customGptApiTests[index]?.value && (
                    <div className="custom-gpt-api-test-result" role="status">
                      <strong>Respuesta HTTP {customGptApiTests[index].value?.status}</strong>
                      <span>{customGptApiTests[index].value?.destination} · {customGptApiTests[index].value?.durationMs} ms</span>
                      {customGptApiTests[index].value?.contentType && <small>{customGptApiTests[index].value?.contentType}</small>}
                      <pre>{customGptApiTests[index].value?.body || "La API respondió sin contenido."}</pre>
                      {customGptApiTests[index].value?.truncated && <small>Vista recortada por seguridad.</small>}
                    </div>
                  )}
                </div>
                <button type="button" className="ghost danger" onClick={() => setCustomGptApiActions((current) => current.filter((_, itemIndex) => itemIndex !== index))}>Quitar acción</button>
              </div>
            ))}
            <button type="button" className="ghost" disabled={customGptApiActions.length >= 10}
              onClick={() => setCustomGptApiActions((current) => [...current, { name: "", description: "", url: "https://", parameters: [], authMode: "none" }])}>
              + Añadir acción API
            </button>
          </div>
        )}
      </fieldset>
      <fieldset className="custom-gpt-preferences">
        <legend>Preferencias de ejecución</legend>
        <p>
          Son preferencias, no imposiciones: si el modelo no está
          disponible, el Broker elegirá otro y la respuesta seguirá llegando.
        </p>
        <label htmlFor="custom-gpt-model">Modelo preferido</label>
        <input
          id="custom-gpt-model"
          value={customGptPreferredModel}
          onChange={(event) => setCustomGptPreferredModel(event.target.value)}
          placeholder="Por ejemplo, qwen2.5:14b (vacío = decide el Broker)"
          spellCheck={false}
          disabled={customGptBusy}
        />
        <label htmlFor="custom-gpt-project">Proyecto predeterminado</label>
        <select
          id="custom-gpt-project"
          value={customGptDefaultProject}
          onChange={(event) => setCustomGptDefaultProject(event.target.value)}
          disabled={customGptBusy}
        >
          <option value="">Sin proyecto predeterminado</option>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
        <small>
          Solo se aplica a los chats que aún no pertenecen a ningún
          proyecto; nunca mueve una conversación ya clasificada.
        </small>
        <label htmlFor="custom-gpt-context-profile">Cantidad de contexto</label>
        <select
          id="custom-gpt-context-profile"
          value={customGptContextProfile}
          onChange={(event) =>
            setCustomGptContextProfile(
              event.target.value as CustomGptView["contextProfile"]
            )}
          disabled={customGptBusy}
        >
          <option value="focused">Enfocado · menos contexto y respuestas más directas</option>
          <option value="balanced">Equilibrado · recomendado</option>
          <option value="broad">Amplio · más historial y documentos</option>
        </select>
        <small>
          Controla cuánto historial, conocimiento y fragmentos documentales puede
          recibir este GPT. No cambia el tamaño de los archivos originales.
        </small>
        <label className="custom-gpt-own-execution">
          <input
            type="checkbox"
            checked={customGptOwnExecution}
            onChange={(event) => setCustomGptOwnExecution(event.target.checked)}
            disabled={customGptBusy}
          />
          <span>
            Usar un perfil propio
            <small>Si está desactivado, este GPT respeta las opciones elegidas en cada chat.</small>
          </span>
        </label>
        {customGptOwnExecution && (
          <div className="custom-gpt-execution-grid">
            <label>
              <span>Privacidad de los datos</span>
              <select
                value={customGptDataClassification}
                onChange={(event) => setCustomGptDataClassification(event.target.value as ConversationExecutionPreferences["dataClassification"])}
                disabled={customGptBusy}
              >
                <option value="public">Públicos · local o nube</option>
                <option value="internal">Uso personal · local o nube</option>
                <option value="confidential">Confidenciales · solo local</option>
                <option value="local_only">Siempre en este equipo</option>
              </select>
            </label>
            <label>
              <span>Forma de responder</span>
              <select
                value={customGptStrategy}
                onChange={(event) => setCustomGptStrategy(event.target.value as ConversationExecutionPreferences["strategy"])}
                disabled={customGptBusy}
              >
                <option value="single">Una respuesta</option>
                <option value="auto">Que decida el Broker</option>
                <option value="mixture_of_agents">Varios modelos y un revisor</option>
              </select>
            </label>
            {customGptStrategy === "mixture_of_agents" && (
              <label>
                <span>Profundidad</span>
                <select
                  value={customGptPreset}
                  onChange={(event) => setCustomGptPreset(event.target.value as ConversationExecutionPreferences["preset"])}
                  disabled={customGptBusy}
                >
                  <option value="fast">Rápida · secuencial</option>
                  <option value="slow">Profunda · paralela si es posible</option>
                </select>
              </label>
            )}
            {customGptStrategy !== "mixture_of_agents" && (
              <label>
                <span>Documentos demasiado largos</span>
                <select
                  value={customGptLongContext}
                  onChange={(event) => setCustomGptLongContext(event.target.value as ConversationExecutionPreferences["longContext"])}
                  disabled={customGptBusy}
                >
                  <option value="fail">Parar y avisar</option>
                  <option value="map_reduce">Dividir y combinar resultados</option>
                </select>
              </label>
            )}
            <label>
              <span>Límite por respuesta (USD)</span>
              <input
                type="number"
                min="0"
                max="10"
                step="0.01"
                value={customGptMaxCost}
                onChange={(event) => setCustomGptMaxCost(event.target.value)}
                disabled={customGptBusy}
              />
            </label>
            <label>
              <span>Prioridad en la cola</span>
              <select
                value={customGptPriority}
                onChange={(event) => setCustomGptPriority(event.target.value)}
                disabled={customGptBusy}
              >
                <option value="50">Alta</option>
                <option value="100">Normal</option>
                <option value="200">Baja</option>
              </select>
            </label>
          </div>
        )}
      </fieldset>
      {customGptError && (
        <p className="error" role="alert">{customGptError}</p>
      )}
      <div className="custom-gpt-form-actions">
        {customGptEditingId && (
          <button
            className="secondary"
            onClick={resetCustomGptForm}
            disabled={customGptBusy}
          >
            Cancelar edición
          </button>
        )}
        <button
          className="primary"
          onClick={() => void saveCustomGpt()}
          disabled={
            customGptBusy ||
            !customGptName.trim() ||
            !customGptInstructions.trim()
          }
        >
          {customGptBusy
            ? "Guardando…"
            : customGptEditingId
              ? "Guardar versión nueva"
              : "Crear GPT"}
        </button>
      </div>
    </div>
    {customGptNotice && (
      <p className="custom-gpt-notice" role="status" aria-live="polite">
        {customGptNotice}
      </p>
    )}
    {customGpts.state === "loading" && (
      <p className="muted">Cargando GPTs personales…</p>
    )}
    {customGpts.state === "error" && (
      <p className="error">{customGpts.message}</p>
    )}
    {customGpts.state === "ready" && (
      customGpts.value.length === 0 ? (
        <p className="activity-empty">Todavía no has creado ningún GPT personal.</p>
      ) : (
        <div className="custom-gpt-list">
          {customGpts.value.map((item) => (
            <article key={item.id} className="custom-gpt-item">
              <div>
                <div className="custom-gpt-item-heading">
                  <span className="custom-gpt-avatar" aria-hidden="true">
                    {customGptIconGlyph(item.iconRef)}
                  </span>
                  <strong>{item.name}</strong>
                  <span>Versión {item.versionNo}</span>
                </div>
                {item.description && <p>{item.description}</p>}
                <small>{item.instructions}</small>
                {item.conversationStarters.length > 0 && (
                  <div className="custom-gpt-starter-summary">
                    {item.conversationStarters.length} iniciador(es)
                  </div>
                )}
                <div className="custom-gpt-permission-summary">
                  <span>
                    Código: {item.toolPermissions.runCode === "confirm"
                      ? "confirmar"
                      : "denegado"}
                  </span>
                  <span>
                    Renombrar: {item.toolPermissions.renameConversation === "confirm"
                      ? "confirmar"
                      : "denegado"}
                  </span>
                  <span>
                    Carpetas: {item.toolPermissions.readAuthorizedFolders === "confirm"
                      ? "confirmar cada lectura"
                      : "denegado"}
                  </span>
                  <span>
                    Modificar archivos: {item.toolPermissions.modifyAuthorizedFiles === "confirm"
                      ? "confirmar cada cambio"
                      : "denegado"}
                  </span>
                  <span>
                    Programar: {item.toolPermissions.createScheduledTasks === "confirm"
                      ? "confirmar cada tarea"
                      : "denegado"}
                  </span>
                  <span>
                    APIs externas: {item.toolPermissions.callExternalApis === "confirm"
                      ? "confirmar cada consulta"
                      : "denegado"}
                  </span>
                  {item.apiActions.length > 0 && (
                    <span>Acciones API: {item.apiActions.length}</span>
                  )}
                  <span>
                    Ejecución: {item.executionProfile
                      ? item.executionProfile.strategy === "mixture_of_agents"
                        ? `varios modelos · ${item.executionProfile.preset === "slow" ? "profunda" : "rápida"}`
                        : item.executionProfile.strategy === "auto"
                          ? "decide el Broker"
                          : "una respuesta"
                      : "ajustes del chat"}
                  </span>
                  <span>
                    Contexto: {item.contextProfile === "focused"
                      ? "enfocado"
                      : item.contextProfile === "broad"
                        ? "amplio"
                        : "equilibrado"}
                  </span>
                </div>
              </div>
              <div className="custom-gpt-item-actions">
                <button
                  className="primary"
                  onClick={() => openDialog({ kind: "custom-gpt-test", customGpt: item })}
                  disabled={customGptBusy}
                  title="Crea un chat normal, envía una pregunta y conserva el resultado."
                >
                  Probar
                </button>
                <button
                  className={
                    customGptKnowledge?.customGptId === item.id
                      ? "primary"
                      : "secondary"
                  }
                  onClick={() => void openCustomGptKnowledge(item.id)}
                  disabled={customGptBusy || customGptKnowledgeBusy}
                  aria-expanded={customGptKnowledge?.customGptId === item.id}
                >
                  Conocimiento
                </button>
                <button
                  className="secondary"
                  onClick={() => void exportCustomGpt(item)}
                  disabled={customGptBusy}
                  title="Exporta solo la configuración. No incluye conocimiento, archivos ni permisos."
                >
                  Exportar
                </button>
                <button
                  className="secondary"
                  onClick={() => void exportCustomGpt(item, true)}
                  disabled={customGptBusy}
                  title="Incluye únicamente conocimiento textual activo y no sensible. Los archivos y permisos nunca se exportan."
                >
                  Exportar con conocimiento
                </button>
                <button
                  className="secondary"
                  onClick={() => void openCustomGptPreview(item.id)}
                  disabled={customGptBusy}
                  title="Muestra lo que recibiría el modelo. No envía nada ni genera coste."
                >
                  Vista previa
                </button>
                <button
                  className="secondary"
                  onClick={() => void duplicateCustomGpt(item.id)}
                  disabled={customGptBusy}
                  title="Crea una copia con la misma configuración, sin permisos ni conocimiento."
                >
                  Duplicar
                </button>
                <button
                  className={customGptHistoryId === item.id ? "primary" : "secondary"}
                  onClick={() => void loadCustomGptVersions(item.id)}
                  disabled={customGptBusy}
                  aria-expanded={customGptHistoryId === item.id}
                >
                  Historial
                </button>
                <button
                  className="secondary"
                  onClick={() => beginCustomGptEdit(item)}
                  disabled={customGptBusy}
                >
                  Editar
                </button>
              </div>
              {customGptHistoryId === item.id && (
                <div className="custom-gpt-history" aria-live="polite">
                  {customGptVersions.state === "loading" && (
                    <small>Cargando historial…</small>
                  )}
                  {customGptVersions.state === "error" && (
                    <small role="alert">{customGptVersions.message}</small>
                  )}
                  {customGptVersions.state === "ready" && (
                    <ul>
                      {customGptVersions.value.map((version) => (
                        <li key={version.id}>
                          <div>
                            <strong>
                              <span className="custom-gpt-history-icon" aria-hidden="true">
                                {customGptIconGlyph(version.iconRef)}
                              </span>{" "}
                              Versión {version.versionNo}
                            </strong>
                            <span>{customGptVersionSummary(version)}</span>
                            <span>
                              Ejecución: {version.executionProfile
                                ? version.executionProfile.strategy === "mixture_of_agents"
                                  ? `varios modelos · ${version.executionProfile.preset === "slow" ? "profunda" : "rápida"}`
                                  : version.executionProfile.strategy === "auto"
                                    ? "decide el Broker"
                                    : "una respuesta"
                                : "ajustes del chat"}
                            </span>
                            <span>
                              Contexto: {version.contextProfile === "focused"
                                ? "enfocado"
                                : version.contextProfile === "broad"
                                  ? "amplio"
                                  : "equilibrado"}
                            </span>
                            <small>{version.instructions}</small>
                          </div>
                          {!version.active && (
                            <button
                              className="secondary"
                              onClick={() =>
                                void restoreCustomGptVersion(item.id, version.id)}
                              disabled={customGptBusy}
                              title="Crea una versión nueva con este contenido. No borra ninguna revisión."
                            >
                              Restaurar
                            </button>
                          )}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
            </article>
          ))}
        </div>
      )
    )}
    {customGptKnowledge && (
      <section className="custom-gpt-knowledge" aria-live="polite">
        <div className="custom-gpt-knowledge-heading">
          <div>
            <span className="kicker">Conocimiento privado</span>
            <h4>
              {customGpts.state === "ready"
                ? customGpts.value.find(
                    (item) => item.id === customGptKnowledge.customGptId
                  )?.name ?? "GPT personal"
                : "GPT personal"}
            </h4>
          </div>
          <button
            className="secondary"
            onClick={() => {
              setCustomGptKnowledge(null);
              setCustomGptFiles(null);
            }}
            disabled={customGptKnowledgeBusy}
          >
            Cerrar
          </button>
        </div>
        <p className="muted">
          Estos datos y archivos solo se añaden a los chats que usan este GPT. No
          aparecen en la memoria general ni en otros GPTs.
        </p>
        <div className="custom-gpt-files">
          <div className="custom-gpt-files-heading">
            <div>
              <strong>Archivos de conocimiento</strong>
              <span>Hasta 20; se aplican automáticamente al seleccionar el GPT.</span>
            </div>
            <button
              className="secondary"
              onClick={() => void importCustomGptFiles()}
              disabled={customGptKnowledgeBusy}
            >
              Añadir archivos
            </button>
          </div>
          {customGptFiles?.data.state === "loading" && (
            <p className="muted">Cargando archivos…</p>
          )}
          {customGptFiles?.data.state === "error" && (
            <p className="error" role="alert">
              {customGptFiles.data.message}
            </p>
          )}
          {customGptFiles?.data.state === "ready" &&
            (customGptFiles.data.value.length === 0 ? (
              <p className="activity-empty">
                Este GPT todavía no tiene archivos propios.
              </p>
            ) : (
              <div className="custom-gpt-files-list">
                {customGptFiles.data.value.map((file) => (
                  <article key={file.id} className="custom-gpt-file-item">
                    <div>
                      <strong>{file.displayName}</strong>
                      <span>
                        {attachmentStatusLabel(file.ingestionStatus)}
                        {attachmentImagePolicyLabel(file) &&
                          ` · ${attachmentImagePolicyLabel(file)}`}
                        {file.ingestionStatus === "ready" &&
                          ` · ${
                            attachmentContextSummary(file)?.label ??
                            "Contexto pendiente"
                          }`}
                      </span>
                    </div>
                    <button
                      className="danger-link"
                      onClick={() => void removeCustomGptFile(file.id)}
                      disabled={customGptKnowledgeBusy}
                    >
                      Retirar
                    </button>
                  </article>
                ))}
              </div>
            ))}
        </div>
        {customGptKnowledge.data.state === "loading" && (
          <p className="muted">Cargando conocimiento…</p>
        )}
        {customGptKnowledge.data.state === "error" && (
          <p className="error" role="alert">
            {customGptKnowledge.data.message}
          </p>
        )}
        {customGptKnowledge.data.state === "ready" && (
          <>
            <div className="custom-gpt-knowledge-form">
              <textarea
                value={customGptKnowledgeDraft}
                onChange={(event) =>
                  setCustomGptKnowledgeDraft(event.target.value)}
                placeholder="Ejemplo: El producto usa contratos versionados y prioriza compatibilidad hacia atrás."
                rows={3}
                maxLength={2000}
                disabled={customGptKnowledgeBusy}
              />
              <div className="custom-gpt-knowledge-controls">
                <select
                  value={customGptKnowledgeCategory}
                  onChange={(event) =>
                    setCustomGptKnowledgeCategory(
                      event.target.value as MemoryItemView["category"]
                    )}
                  disabled={customGptKnowledgeBusy}
                  aria-label="Tipo de conocimiento"
                >
                  <option value="fact">Dato</option>
                  <option value="instruction">Instrucción</option>
                  <option value="preference">Preferencia</option>
                </select>
                <label>
                  <input
                    type="checkbox"
                    checked={customGptKnowledgeSensitive}
                    onChange={(event) =>
                      setCustomGptKnowledgeSensitive(event.target.checked)}
                    disabled={customGptKnowledgeBusy}
                  />
                  Sensible: mantener en modelos locales
                </label>
                <button
                  className="primary"
                  onClick={() => void createCustomGptKnowledge()}
                  disabled={
                    customGptKnowledgeBusy ||
                    !customGptKnowledgeDraft.trim()
                  }
                >
                  {customGptKnowledgeBusy ? "Guardando…" : "Añadir conocimiento"}
                </button>
              </div>
            </div>
            {customGptKnowledgeNotice && (
              <p className="custom-gpt-notice" role="status">
                {customGptKnowledgeNotice}
              </p>
            )}
            {customGptKnowledge.data.value.length === 0 ? (
              <p className="activity-empty">
                Este GPT todavía no tiene conocimiento propio.
              </p>
            ) : (
              <div className="custom-gpt-knowledge-list">
                {customGptKnowledge.data.value.map((item) => (
                  <article
                    key={item.id}
                    className={`custom-gpt-knowledge-item ${
                      item.enabled ? "" : "disabled"
                    }`}
                  >
                    <div>
                      <div className="memory-badges">
                        <span>
                          {item.category === "preference"
                            ? "Preferencia"
                            : item.category === "instruction"
                              ? "Instrucción"
                              : "Dato"}
                        </span>
                        {item.sensitivity === "sensitive" && (
                          <span className="sensitive">Sensible</span>
                        )}
                        <span className={`embedding ${item.embeddingStatus}`}>
                          {item.embeddingStatus === "ready"
                            ? "Índice preparado"
                            : item.embeddingStatus === "indexing"
                              ? "Indexando…"
                              : item.embeddingStatus === "failed"
                                ? "Error de índice"
                                : "Sin índice"}
                        </span>
                      </div>
                      <p>{item.content}</p>
                      {item.embeddingError && (
                        <small className="error">{item.embeddingError}</small>
                      )}
                    </div>
                    <div className="custom-gpt-knowledge-item-actions">
                      <button
                        className="secondary"
                        onClick={() =>
                          void toggleCustomGptKnowledgeItem(
                            item.id,
                            !item.enabled
                          )}
                        disabled={customGptKnowledgeBusy}
                      >
                        {item.enabled ? "No usar" : "Usar"}
                      </button>
                      {item.embeddingStatus !== "indexing" && (
                        <button
                          className="secondary"
                          onClick={() =>
                            void reindexCustomGptKnowledgeItem(item.id)}
                          disabled={customGptKnowledgeBusy}
                        >
                          Preparar índice
                        </button>
                      )}
                      <button
                        className="danger-link"
                        onClick={() =>
                          void removeCustomGptKnowledgeItem(item.id)}
                        disabled={customGptKnowledgeBusy}
                      >
                        Eliminar
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </>
        )}
      </section>
    )}
  </section>
  );
}
