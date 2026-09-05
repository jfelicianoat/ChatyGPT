/**
 * Estado y acciones de los GPT personales.
 *
 * Es el bloque mas grande que quedaba dentro de `App.tsx`: el formulario, las
 * acciones de API, el conocimiento asociado y el historial de versiones. Vive
 * aparte para que la tarjeta reciba `ReturnType<typeof useGpts>` en vez de un
 * centenar de props.
 */
import { useState } from "react";

import {
  brokerAttachmentExtensions,
  type ApiCredentialStatus,
  type AttachmentView,
  type BrokerDiagnostic,
  type ConversationExecutionPreferences,
  type ConversationView,
  type CustomGptApiActionPreview,
  type CustomGptApiActionTestResult,
  type CustomGptIcon,
  type CustomGptPreview,
  type CustomGptVersionView,
  type CustomGptView,
  type Loadable,
  type MemoryItemView
} from "../domain";
import { describeError } from "../errors";
import {
  shouldDescribeImages,
  type ImageDescriptionPreference
} from "../ingestionPreferences";
import { platform } from "../platform";

type Opciones = {
  /** Diagnostico del broker: dice que extensiones admite el adjunto. */
  broker: Loadable<BrokerDiagnostic> | null;
  /** Conversacion abierta, para refrescar el chat si usa el GPT tocado. */
  conversation: Loadable<ConversationView> | null;
  /** Preferencia global de descripcion de imagenes al subir ficheros. */
  imageDescriptionPreference: ImageDescriptionPreference;
  /** Relee la auditoria despues de una accion que deja rastro. */
  refreshAuditEvents: () => Promise<void>;
};

export function useGpts({
  broker,
  conversation,
  imageDescriptionPreference,
  refreshAuditEvents
}: Opciones) {
  const [customGpts, setCustomGpts] =
    useState<Loadable<CustomGptView[]>>({ state: "loading" });
  const [customGptEditingId, setCustomGptEditingId] = useState<string | null>(null);
  const [customGptName, setCustomGptName] = useState("");
  const [customGptDescription, setCustomGptDescription] = useState("");
  const [customGptIcon, setCustomGptIcon] = useState<CustomGptIcon>("spark");
  const [customGptInstructions, setCustomGptInstructions] = useState("");
  const [customGptStartersText, setCustomGptStartersText] = useState("");
  const [customGptRunCodePermission, setCustomGptRunCodePermission] = useState(false);
  const [customGptRenamePermission, setCustomGptRenamePermission] = useState(false);
  const [customGptFolderReadPermission, setCustomGptFolderReadPermission] = useState(false);
  const [customGptFileModifyPermission, setCustomGptFileModifyPermission] = useState(false);
  const [customGptSchedulePermission, setCustomGptSchedulePermission] = useState(false);
  const [customGptExternalApiPermission, setCustomGptExternalApiPermission] = useState(false);
  const [customGptApiActions, setCustomGptApiActions] = useState<CustomGptView["apiActions"]>([]);
  const [customGptApiSamples, setCustomGptApiSamples] = useState<Record<number, Record<string, string>>>({});
  const [customGptApiPreviews, setCustomGptApiPreviews] = useState<Record<number, { value?: CustomGptApiActionPreview; error?: string }>>({});
  const [customGptApiTests, setCustomGptApiTests] = useState<Record<number, { loading?: boolean; value?: CustomGptApiActionTestResult; error?: string }>>({});
  const [apiCredentials, setApiCredentials] = useState<Loadable<ApiCredentialStatus[]>>({ state: "loading" });
  const [apiCredentialDrafts, setApiCredentialDrafts] = useState<Record<number, string>>({});
  const [apiCredentialBusy, setApiCredentialBusy] = useState<number | null>(null);
  const [customGptPreferredModel, setCustomGptPreferredModel] = useState("");
  const [customGptDefaultProject, setCustomGptDefaultProject] = useState("");
  const [customGptOwnExecution, setCustomGptOwnExecution] = useState(false);
  const [customGptContextProfile, setCustomGptContextProfile] =
    useState<CustomGptView["contextProfile"]>("balanced");
  const [customGptDataClassification, setCustomGptDataClassification] =
    useState<ConversationExecutionPreferences["dataClassification"]>("internal");
  const [customGptStrategy, setCustomGptStrategy] =
    useState<ConversationExecutionPreferences["strategy"]>("single");
  const [customGptPreset, setCustomGptPreset] =
    useState<ConversationExecutionPreferences["preset"]>("fast");
  const [customGptMaxCost, setCustomGptMaxCost] = useState("0.10");
  const [customGptLongContext, setCustomGptLongContext] =
    useState<ConversationExecutionPreferences["longContext"]>("fail");
  const [customGptPriority, setCustomGptPriority] = useState("100");
  const [customGptHistoryId, setCustomGptHistoryId] = useState<string | null>(null);
  const [customGptPreview, setCustomGptPreview] =
    useState<Loadable<CustomGptPreview> | null>(null);
  const [customGptVersions, setCustomGptVersions] =
    useState<Loadable<CustomGptVersionView[]>>({ state: "loading" });
  const [customGptBusy, setCustomGptBusy] = useState(false);
  const [customGptError, setCustomGptError] = useState<string | null>(null);
  const [customGptNotice, setCustomGptNotice] = useState<string | null>(null);
  const [customGptKnowledge, setCustomGptKnowledge] = useState<{
    customGptId: string;
    data: Loadable<MemoryItemView[]>;
  } | null>(null);
  const [customGptFiles, setCustomGptFiles] = useState<{
    customGptId: string;
    data: Loadable<AttachmentView[]>;
  } | null>(null);
  const [customGptKnowledgeDraft, setCustomGptKnowledgeDraft] = useState("");
  const [customGptKnowledgeCategory, setCustomGptKnowledgeCategory] =
    useState<MemoryItemView["category"]>("fact");
  const [customGptKnowledgeSensitive, setCustomGptKnowledgeSensitive] = useState(false);
  const [customGptKnowledgeBusy, setCustomGptKnowledgeBusy] = useState(false);
  const [customGptKnowledgeNotice, setCustomGptKnowledgeNotice] =
    useState<string | null>(null);
  const [activeCustomGptKnowledge, setActiveCustomGptKnowledge] =
    useState<Loadable<MemoryItemView[]> | null>(null);
  const [activeCustomGptFiles, setActiveCustomGptFiles] =
    useState<Loadable<AttachmentView[]> | null>(null);

  const resetCustomGptForm = () => {
    setCustomGptEditingId(null);
    setCustomGptName("");
    setCustomGptDescription("");
    setCustomGptIcon("spark");
    setCustomGptInstructions("");
    setCustomGptStartersText("");
    setCustomGptRunCodePermission(false);
    setCustomGptRenamePermission(false);
    setCustomGptFolderReadPermission(false);
    setCustomGptFileModifyPermission(false);
    setCustomGptSchedulePermission(false);
    setCustomGptExternalApiPermission(false);
    setCustomGptApiActions([]);
    setCustomGptApiSamples({});
    setCustomGptApiPreviews({});
    setCustomGptApiTests({});
    setApiCredentialDrafts({});
    setCustomGptPreferredModel("");
    setCustomGptDefaultProject("");
    setCustomGptOwnExecution(false);
    setCustomGptContextProfile("balanced");
    setCustomGptDataClassification("internal");
    setCustomGptStrategy("single");
    setCustomGptPreset("fast");
    setCustomGptMaxCost("0.10");
    setCustomGptLongContext("fail");
    setCustomGptPriority("100");
    setCustomGptError(null);
  };

  const loadCustomGptVersions = async (customGptId: string) => {
    if (customGptHistoryId === customGptId) {
      setCustomGptHistoryId(null);
      return;
    }
    setCustomGptHistoryId(customGptId);
    setCustomGptVersions({ state: "loading" });
    try {
      setCustomGptVersions({
        state: "ready",
        value: await platform.listCustomGptVersions(customGptId)
      });
    } catch (error) {
      setCustomGptVersions({ state: "error", message: describeError(error) });
    }
  };

  const restoreCustomGptVersion = async (customGptId: string, versionId: string) => {
    // Restaurar reemplaza la configuración vigente del GPT. Las versiones
    // anteriores se conservan, pero la decisión sigue siendo de la persona.
    if (
      !window.confirm(
        "¿Restaurar esta versión? Reemplazará la configuración actual del GPT; las versiones anteriores se conservan."
      )
    ) {
      return;
    }
    setCustomGptBusy(true);
    setCustomGptError(null);
    try {
      const restored = await platform.restoreCustomGptVersion(customGptId, versionId);
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptVersions({
        state: "ready",
        value: await platform.listCustomGptVersions(customGptId)
      });
      setCustomGptNotice(
        `${restored.name}: restaurado como versión ${restored.versionNo}. Las anteriores se conservan.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const openCustomGptPreview = async (customGptId: string) => {
    // La vista previa no envía nada al Broker ni genera coste: solo compone
    // localmente lo que recibiría el modelo si se usara este GPT.
    setCustomGptPreview({ state: "loading" });
    try {
      setCustomGptPreview({
        state: "ready",
        value: await platform.previewCustomGpt(customGptId)
      });
    } catch (error) {
      setCustomGptPreview({ state: "error", message: describeError(error) });
    }
  };

  const duplicateCustomGpt = async (customGptId: string) => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    try {
      const copy = await platform.duplicateCustomGpt(customGptId);
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        `${copy.name}: copia creada sin permisos ni conocimiento del original.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const beginCustomGptEdit = (item: CustomGptView) => {
    setCustomGptEditingId(item.id);
    setCustomGptName(item.name);
    setCustomGptDescription(item.description ?? "");
    setCustomGptIcon(item.iconRef ?? "spark");
    setCustomGptInstructions(item.instructions);
    setCustomGptStartersText(item.conversationStarters.join("\n"));
    setCustomGptRunCodePermission(item.toolPermissions.runCode === "confirm");
    setCustomGptRenamePermission(item.toolPermissions.renameConversation === "confirm");
    setCustomGptFolderReadPermission(item.toolPermissions.readAuthorizedFolders === "confirm");
    setCustomGptFileModifyPermission(item.toolPermissions.modifyAuthorizedFiles === "confirm");
    setCustomGptSchedulePermission(item.toolPermissions.createScheduledTasks === "confirm");
    setCustomGptExternalApiPermission(item.toolPermissions.callExternalApis === "confirm");
    setCustomGptApiActions((item.apiActions ?? []).map((action) => ({
      ...action,
      authMode: action.authMode ?? "none",
      parameters: action.parameters.length > 0
        ? action.parameters
        : (action.queryParameters ?? []).map((name) => ({ name, type: "string" as const, required: true, location: "query" as const }))
    })));
    setCustomGptApiSamples({});
    setCustomGptApiPreviews({});
    setCustomGptApiTests({});
    setApiCredentialDrafts({});
    setCustomGptPreferredModel(item.preferredModel ?? "");
    setCustomGptDefaultProject(item.defaultProjectId ?? "");
    setCustomGptOwnExecution(item.executionProfile !== null);
    setCustomGptContextProfile(item.contextProfile ?? "balanced");
    setCustomGptDataClassification(item.executionProfile?.dataClassification ?? "internal");
    setCustomGptStrategy(item.executionProfile?.strategy ?? "single");
    setCustomGptPreset(item.executionProfile?.preset ?? "fast");
    setCustomGptMaxCost(String(item.executionProfile?.maxCostUsd ?? 0.1));
    setCustomGptLongContext(item.executionProfile?.longContext ?? "fail");
    setCustomGptPriority(String(item.executionProfile?.priority ?? 100));
    setCustomGptError(null);
    setCustomGptNotice(null);
  };

  const saveCustomGpt = async () => {
    if (!customGptName.trim() || !customGptInstructions.trim()) return;
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const conversationStarters = customGptStartersText
        .split(/\r?\n/)
        .map((starter) => starter.trim())
        .filter(Boolean);
      const parsedCost = Number(customGptMaxCost);
      const parsedPriority = Number(customGptPriority);
      if (customGptOwnExecution && (!Number.isFinite(parsedCost) || parsedCost < 0 || parsedCost > 10)) {
        throw new Error("El límite de coste debe estar entre 0 y 10 USD.");
      }
      if (customGptOwnExecution && (!Number.isInteger(parsedPriority) || parsedPriority < 0 || parsedPriority > 1000)) {
        throw new Error("La prioridad debe ser un número entero entre 0 y 1000.");
      }
      const executionProfile: CustomGptView["executionProfile"] = customGptOwnExecution
        ? {
            dataClassification: customGptDataClassification,
            strategy: customGptStrategy,
            preset: customGptStrategy === "mixture_of_agents" ? customGptPreset : "fast",
            maxCostUsd: parsedCost,
            longContext: customGptStrategy === "mixture_of_agents" ? "fail" : customGptLongContext,
            priority: parsedPriority
          }
        : null;
      const saved = customGptEditingId
        ? await platform.updateCustomGpt(
            customGptEditingId,
            customGptName,
            customGptDescription,
            customGptIcon,
            customGptInstructions,
            conversationStarters,
            {
              runCode: customGptRunCodePermission ? "confirm" : "deny",
              renameConversation: customGptRenamePermission ? "confirm" : "deny",
              readAuthorizedFolders: customGptFolderReadPermission ? "confirm" : "deny",
              modifyAuthorizedFiles: customGptFileModifyPermission ? "confirm" : "deny",
              createScheduledTasks: customGptSchedulePermission ? "confirm" : "deny",
              callExternalApis: customGptExternalApiPermission ? "confirm" : "deny"
            },
            customGptPreferredModel.trim() || null,
            customGptDefaultProject || null,
            executionProfile,
            customGptContextProfile
            ,customGptApiActions
          )
        : await platform.createCustomGpt(
            customGptName,
            customGptDescription,
            customGptIcon,
            customGptInstructions,
            conversationStarters,
            {
              runCode: customGptRunCodePermission ? "confirm" : "deny",
              renameConversation: customGptRenamePermission ? "confirm" : "deny",
              readAuthorizedFolders: customGptFolderReadPermission ? "confirm" : "deny",
              modifyAuthorizedFiles: customGptFileModifyPermission ? "confirm" : "deny",
              createScheduledTasks: customGptSchedulePermission ? "confirm" : "deny",
              callExternalApis: customGptExternalApiPermission ? "confirm" : "deny"
            },
            customGptPreferredModel.trim() || null,
            customGptDefaultProject || null,
            executionProfile,
            customGptContextProfile
            ,customGptApiActions
          );
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        customGptEditingId
          ? `${saved.name}: versión ${saved.versionNo} guardada.`
          : `${saved.name}: GPT creado con su versión 1.`
      );
      resetCustomGptForm();
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const importCustomGpt = async () => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const sourcePath = await platform.pickCustomGptImportPath();
      if (!sourcePath) return;
      const report = await platform.importCustomGpt(sourcePath);
      const imported = report.customGpt;
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        report.knowledgeRequiresReview
          ? `${imported.name}: GPT importado con ${report.importedKnowledge} elemento(s) de conocimiento pendientes de revisión. Abre Conocimiento y pulsa Usar en los que quieras activar.`
          : `${imported.name}: GPT importado como versión 1. Sus permisos están denegados por seguridad.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const exportCustomGpt = async (item: CustomGptView, includeKnowledge = false) => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const destinationPath = await platform.pickCustomGptExportPath(
        includeKnowledge ? `${item.name}-con-conocimiento` : item.name
      );
      if (!destinationPath) return;
      const report = await platform.exportCustomGpt(
        item.id,
        destinationPath,
        includeKnowledge
      );
      setCustomGptNotice(
        includeKnowledge
          ? `${item.name}: paquete exportado con ${report.includedKnowledge} elemento(s) de conocimiento. Se excluyeron ${report.excludedSensitive} sensibles, ${report.excludedDisabled} desactivados y ${report.excludedFiles} archivos.`
          : `${item.name}: configuración exportada sin conocimiento ni archivos.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const openCustomGptKnowledge = async (customGptId: string) => {
    if (customGptKnowledge?.customGptId === customGptId) {
      setCustomGptKnowledge(null);
      setCustomGptFiles(null);
      setCustomGptKnowledgeNotice(null);
      return;
    }
    setCustomGptKnowledge({ customGptId, data: { state: "loading" } });
    setCustomGptFiles({ customGptId, data: { state: "loading" } });
    setCustomGptKnowledgeNotice(null);
    try {
      const [items, files] = await Promise.all([
        platform.getCustomGptKnowledge(customGptId),
        platform.listCustomGptFiles(customGptId)
      ]);
      setCustomGptKnowledge({
        customGptId,
        data: { state: "ready", value: items }
      });
      setCustomGptFiles({
        customGptId,
        data: { state: "ready", value: files }
      });
    } catch (error) {
      const message = describeError(error);
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message }
      });
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message }
      });
    }
  };

  const importCustomGptFiles = async () => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    setCustomGptKnowledgeNotice(null);
    try {
      const paths = await platform.pickAttachmentPaths(
        broker?.state === "ready" ? brokerAttachmentExtensions(broker.value) : []
      );
      if (paths.length === 0) return;
      const currentCount =
        customGptFiles?.customGptId === customGptId &&
        customGptFiles.data.state === "ready"
          ? customGptFiles.data.value.length
          : 0;
      if (currentCount + paths.length > 20) {
        throw new Error(
          `Este GPT admite hasta 20 archivos. Ya tiene ${currentCount} y has elegido ${paths.length}.`
        );
      }
      for (const sourcePath of paths) {
        await platform.importCustomGptFile(
          customGptId,
          sourcePath,
          shouldDescribeImages(imageDescriptionPreference)
        );
      }
      const files = await platform.listCustomGptFiles(customGptId);
      setCustomGptFiles({ customGptId, data: { state: "ready", value: files } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptFiles({ state: "ready", value: files });
      }
      setCustomGptKnowledgeNotice(
        `${paths.length} archivo(s) añadido(s). Se usarán cuando su estado sea Preparado.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const removeCustomGptFile = async (attachmentId: string) => {
    if (!customGptKnowledge) return;
    if (!window.confirm("¿Retirar este archivo del conocimiento de este GPT?")) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const files = await platform.removeCustomGptFile(customGptId, attachmentId);
      setCustomGptFiles({ customGptId, data: { state: "ready", value: files } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptFiles({ state: "ready", value: files });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const createCustomGptKnowledge = async () => {
    if (
      !customGptKnowledge ||
      customGptKnowledge.data.state !== "ready" ||
      !customGptKnowledgeDraft.trim()
    ) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    setCustomGptKnowledgeNotice(null);
    try {
      const items = await platform.createCustomGptKnowledgeItem(
        customGptId,
        customGptKnowledgeDraft,
        customGptKnowledgeCategory,
        customGptKnowledgeSensitive ? "sensitive" : "normal"
      );
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      setCustomGptKnowledgeDraft("");
      setCustomGptKnowledgeSensitive(false);
      setCustomGptKnowledgeNotice(
        "Conocimiento guardado. Ya se aplicará cuando selecciones este GPT en un chat."
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const toggleCustomGptKnowledgeItem = async (
    memoryId: string,
    enabled: boolean
  ) => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.setCustomGptKnowledgeItemEnabled(
        customGptId,
        memoryId,
        enabled
      );
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const removeCustomGptKnowledgeItem = async (memoryId: string) => {
    if (!customGptKnowledge) return;
    if (!window.confirm("¿Eliminar este conocimiento solo de este GPT personal?")) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.deleteCustomGptKnowledgeItem(customGptId, memoryId);
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const reindexCustomGptKnowledgeItem = async (memoryId: string) => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.reindexCustomGptKnowledgeItem(customGptId, memoryId);
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const apiActionSampleValues = (actionIndex: number) => {
    const action = customGptApiActions[actionIndex];
    const raw = customGptApiSamples[actionIndex] ?? {};
    const sampleValues: Record<string, string | number | boolean> = {};
    for (const parameter of action.parameters) {
      const value = raw[parameter.name];
      if (!parameter.required && !value?.trim()) continue;
      if (parameter.type === "number") {
        const parsed = Number(value);
        if (!Number.isFinite(parsed)) {
          setCustomGptApiPreviews((current) => ({ ...current, [actionIndex]: { error: `${parameter.name} debe ser un número.` } }));
          return null;
        }
        sampleValues[parameter.name] = parsed;
      } else if (parameter.type === "boolean") {
        sampleValues[parameter.name] = value === "true";
      } else {
        sampleValues[parameter.name] = value ?? "";
      }
    }
    return sampleValues;
  };

  const previewApiAction = async (actionIndex: number) => {
    const action = customGptApiActions[actionIndex];
    const sampleValues = apiActionSampleValues(actionIndex);
    if (!sampleValues) return;
    try {
      const value = await platform.previewCustomGptApiAction(action, sampleValues);
      setCustomGptApiPreviews((current) => ({ ...current, [actionIndex]: { value } }));
    } catch (error) {
      setCustomGptApiPreviews((current) => ({ ...current, [actionIndex]: { error: describeError(error) } }));
    }
  };

  const testApiAction = async (actionIndex: number) => {
    const action = customGptApiActions[actionIndex];
    const sampleValues = apiActionSampleValues(actionIndex);
    if (!sampleValues) return;
    let preview: CustomGptApiActionPreview;
    try {
      preview = await platform.previewCustomGptApiAction(action, sampleValues);
      setCustomGptApiPreviews((current) => ({ ...current, [actionIndex]: { value: preview } }));
    } catch (error) {
      setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { error: describeError(error) } }));
      return;
    }
    const dataSummary = preview.dataSent.length === 0
      ? "No se enviarán parámetros."
      : `Se enviarán ${preview.dataSent.length} parámetro(s).`;
    const credentialSummary = (action.authMode ?? "none") === "none"
      ? "No se usará ninguna credencial."
      : `Se usará la credencial protegida «${action.credentialRef}». Su valor no se mostrará ni se enviará al modelo.`;
    if (!window.confirm(
      `¿Conectar ahora con ${preview.destination}?\n\n${dataSummary}\n${credentialSummary}\nMétodo: ${preview.method}\n\nEsta prueba abre la API una sola vez y no contacta con Broker AI.`
    )) return;
    setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { loading: true } }));
    try {
      const value = await platform.testCustomGptApiAction(action, sampleValues);
      setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { value } }));
    } catch (error) {
      setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { error: describeError(error) } }));
    }
  };

  const saveApiCredential = async (actionIndex: number) => {
    const action = customGptApiActions[actionIndex];
    const name = action.credentialRef?.trim() ?? "";
    const secret = apiCredentialDrafts[actionIndex]?.trim() ?? "";
    if (!name || !secret) {
      setCustomGptApiTests((current) => ({
        ...current,
        [actionIndex]: { error: "Indica un alias y pega la clave antes de guardarla." }
      }));
      return;
    }
    setApiCredentialBusy(actionIndex);
    try {
      setApiCredentials({ state: "ready", value: await platform.setApiCredential(name, secret) });
      setApiCredentialDrafts((current) => ({ ...current, [actionIndex]: "" }));
      setCustomGptApiTests((current) => ({
        ...current,
        [actionIndex]: { error: undefined }
      }));
    } catch (error) {
      setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { error: describeError(error) } }));
    } finally {
      setApiCredentialBusy(null);
    }
  };

  const removeApiCredential = async (actionIndex: number) => {
    const name = customGptApiActions[actionIndex].credentialRef?.trim();
    if (!name || !window.confirm(`¿Retirar la credencial protegida «${name}» de este equipo? Las acciones que la usen dejarán de funcionar hasta que vuelvas a guardarla.`)) return;
    setApiCredentialBusy(actionIndex);
    try {
      setApiCredentials({ state: "ready", value: await platform.clearApiCredential(name) });
    } catch (error) {
      setCustomGptApiTests((current) => ({ ...current, [actionIndex]: { error: describeError(error) } }));
    } finally {
      setApiCredentialBusy(null);
    }
  };

  return {
    customGpts,
    setCustomGpts,
    customGptEditingId,
    setCustomGptEditingId,
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
    setCustomGptApiPreviews,
    customGptApiTests,
    setCustomGptApiTests,
    apiCredentials,
    setApiCredentials,
    apiCredentialDrafts,
    setApiCredentialDrafts,
    apiCredentialBusy,
    setApiCredentialBusy,
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
    setCustomGptHistoryId,
    customGptPreview,
    setCustomGptPreview,
    customGptVersions,
    setCustomGptVersions,
    customGptBusy,
    setCustomGptBusy,
    customGptError,
    setCustomGptError,
    customGptNotice,
    setCustomGptNotice,
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
    setCustomGptKnowledgeBusy,
    customGptKnowledgeNotice,
    setCustomGptKnowledgeNotice,
    activeCustomGptKnowledge,
    setActiveCustomGptKnowledge,
    activeCustomGptFiles,
    setActiveCustomGptFiles,
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
    apiActionSampleValues,
    previewApiAction,
    testApiAction,
    saveApiCredential,
    removeApiCredential
  };
}
