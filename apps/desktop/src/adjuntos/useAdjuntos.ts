/**
 * Estado y acciones de los adjuntos de la conversación.
 *
 * La ingesta de ficheros, las capturas de pantalla, la cámara y los ficheros
 * de proyecto vivían sueltos dentro de `App.tsx`. Aquí viajan juntos, y la
 * tarjeta recibe `ReturnType<typeof useAdjuntos>` en lugar de cuarenta props.
 */
import { useEffect, useRef, useState, type RefObject } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import {
  brokerAttachmentExtensions,
  projectFilesAvailableToConversation,
  type AttachmentView,
  type BrokerDiagnostic,
  type ConversationView,
  type Loadable
} from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";
import {
  captureDisplayName,
  captureScreenFrame,
  captureVideoFrame,
  cropCapturedFrame,
  normalizeCropSelection,
  type CapturedScreenFrame,
  type CropSelection
} from "../screenCapture";
import { cameraFailureMessage, openCameraStream } from "../cameraCapture";
import {
  shouldDescribeImages,
  type ImageDescriptionPreference
} from "../ingestionPreferences";

export type ScreenCapturePreview = CapturedScreenFrame & {
  conversationId: string;
  previewUrl: string;
  source: "screen" | "camera";
};

type Opciones = {
  /** Conversación abierta: los manejadores la exigen en estado «ready». */
  conversation: Loadable<ConversationView> | null;
  /** Diagnóstico del Broker: da las extensiones aceptadas al elegir ficheros. */
  broker: Loadable<BrokerDiagnostic> | null;
  /** Preferencia de descripción de imágenes al importar documentos. */
  imageDescriptionPreference: ImageDescriptionPreference;
  /** Conversación seleccionada según App: descarta respuestas llegadas tarde. */
  selectedConversationRef: RefObject<string | null>;
};

export function useAdjuntos({
  conversation,
  broker,
  imageDescriptionPreference,
  selectedConversationRef
}: Opciones) {
  const [attachments, setAttachments] = useState<AttachmentView[]>([]);
  const [projectFiles, setProjectFiles] = useState<AttachmentView[]>([]);
  const [draftAttachmentIds, setDraftAttachmentIds] = useState<string[]>([]);
  const [attachmentBusy, setAttachmentBusy] = useState(false);
  const [screenCaptureBusy, setScreenCaptureBusy] = useState(false);
  const [screenCapturePreview, setScreenCapturePreview] =
    useState<ScreenCapturePreview | null>(null);
  const screenCaptureUrlRef = useRef<string | null>(null);
  const [cropMode, setCropMode] = useState(false);
  const [cropSelection, setCropSelection] = useState<CropSelection | null>(null);
  const cropStartRef = useRef<{ x: number; y: number } | null>(null);
  const [cameraOpen, setCameraOpen] = useState(false);
  const [cameraReady, setCameraReady] = useState(false);
  const [cameraBusy, setCameraBusy] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);
  const [cameraConversationId, setCameraConversationId] = useState<string | null>(null);
  const cameraVideoRef = useRef<HTMLVideoElement | null>(null);
  const cameraStreamRef = useRef<MediaStream | null>(null);
  const [projectFileBusyId, setProjectFileBusyId] = useState<string | null>(null);
  const [attachmentContextRetryId, setAttachmentContextRetryId] = useState<string | null>(null);
  const [attachmentSemanticRetryId, setAttachmentSemanticRetryId] = useState<string | null>(null);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);

  const selectedAttachments = attachments.filter((item) =>
    draftAttachmentIds.includes(item.id)
  );
  const availableProjectFiles = projectFilesAvailableToConversation(attachments, projectFiles);

  async function importAttachmentPaths(conversationId: string, paths: string[]) {
    setAttachmentBusy(true);
    setAttachmentError(null);
    try {
      const importedIds: string[] = [];
      for (const path of paths) {
        const attachment = await platform.importAttachment(
          conversationId,
          path,
          shouldDescribeImages(imageDescriptionPreference)
        );
        importedIds.push(attachment.id);
      }
      setAttachments(await platform.listAttachments(conversationId));
      setDraftAttachmentIds((current) => [...new Set([...current, ...importedIds])]);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentBusy(false);
    }
  }

  const chooseAttachments = async () => {
    if (conversation?.state !== "ready") return;
    try {
      const paths = await platform.pickAttachmentPaths(
        broker?.state === "ready" ? brokerAttachmentExtensions(broker.value) : []
      );
      if (paths.length > 0) await importAttachmentPaths(conversation.value.id, paths);
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  function discardScreenCapture() {
    if (screenCaptureUrlRef.current) {
      URL.revokeObjectURL(screenCaptureUrlRef.current);
      screenCaptureUrlRef.current = null;
    }
    cropStartRef.current = null;
    setCropMode(false);
    setCropSelection(null);
    setScreenCapturePreview(null);
  }

  function cropPointerPosition(event: React.PointerEvent<HTMLDivElement>) {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: (event.clientX - bounds.left) / bounds.width,
      y: (event.clientY - bounds.top) / bounds.height
    };
  }

  const beginCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode) return;
    const point = cropPointerPosition(event);
    cropStartRef.current = point;
    setCropSelection(null);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const updateCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode || !cropStartRef.current) return;
    const point = cropPointerPosition(event);
    setCropSelection(
      normalizeCropSelection(
        cropStartRef.current.x,
        cropStartRef.current.y,
        point.x,
        point.y,
        0
      )
    );
  };

  const finishCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode || !cropStartRef.current) return;
    const point = cropPointerPosition(event);
    setCropSelection(
      normalizeCropSelection(
        cropStartRef.current.x,
        cropStartRef.current.y,
        point.x,
        point.y
      )
    );
    cropStartRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const applyScreenCaptureCrop = async () => {
    if (!screenCapturePreview || !cropSelection) return;
    setScreenCaptureBusy(true);
    setAttachmentError(null);
    try {
      const frame = await cropCapturedFrame(screenCapturePreview, cropSelection);
      if (screenCaptureUrlRef.current) URL.revokeObjectURL(screenCaptureUrlRef.current);
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: screenCapturePreview.conversationId,
        previewUrl,
        source: screenCapturePreview.source
      });
      setCropMode(false);
      setCropSelection(null);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setScreenCaptureBusy(false);
    }
  };

  const takeScreenCapture = async () => {
    if (conversation?.state !== "ready") return;
    setScreenCaptureBusy(true);
    setAttachmentError(null);
    try {
      const frame = await captureScreenFrame();
      discardScreenCapture();
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: conversation.value.id,
        previewUrl,
        source: "screen"
      });
    } catch (error) {
      if (error instanceof DOMException && error.name === "NotAllowedError") {
        return;
      }
      setAttachmentError(describeError(error));
    } finally {
      setScreenCaptureBusy(false);
    }
  };

  function stopCamera() {
    cameraStreamRef.current?.getTracks().forEach((track) => track.stop());
    cameraStreamRef.current = null;
    if (cameraVideoRef.current) {
      cameraVideoRef.current.srcObject = null;
    }
    setCameraOpen(false);
    setCameraReady(false);
    setCameraConversationId(null);
  }

  const openCamera = async () => {
    if (conversation?.state !== "ready") return;
    setCameraBusy(true);
    setCameraError(null);
    stopCamera();
    try {
      const stream = await openCameraStream();
      discardScreenCapture();
      cameraStreamRef.current = stream;
      setCameraConversationId(conversation.value.id);
      setCameraOpen(true);
    } catch (error) {
      setCameraError(cameraFailureMessage(error));
    } finally {
      setCameraBusy(false);
    }
  };

  const takeCameraPhoto = async () => {
    if (
      conversation?.state !== "ready" ||
      !cameraVideoRef.current ||
      cameraConversationId !== conversation.value.id
    ) {
      stopCamera();
      return;
    }
    setCameraBusy(true);
    setCameraError(null);
    try {
      const frame = await captureVideoFrame(
        cameraVideoRef.current,
        captureDisplayName(new Date(), "foto")
      );
      stopCamera();
      discardScreenCapture();
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: conversation.value.id,
        previewUrl,
        source: "camera"
      });
    } catch (error) {
      setCameraError(cameraFailureMessage(error));
    } finally {
      setCameraBusy(false);
    }
  };

  const attachScreenCapture = async () => {
    if (
      conversation?.state !== "ready" ||
      !screenCapturePreview ||
      screenCapturePreview.conversationId !== conversation.value.id
    ) {
      discardScreenCapture();
      return;
    }
    setAttachmentBusy(true);
    setAttachmentError(null);
    try {
      const bytes = Array.from(
        new Uint8Array(await screenCapturePreview.blob.arrayBuffer())
      );
      const attachment = await platform.importCapturedImage(
        conversation.value.id,
        screenCapturePreview.displayName,
        bytes
      );
      setAttachments(await platform.listAttachments(conversation.value.id));
      setDraftAttachmentIds((current) => [...new Set([...current, attachment.id])]);
      discardScreenCapture();
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentBusy(false);
    }
  };

  const removeAttachment = async (attachmentId: string) => {
    if (conversation?.state !== "ready") return;
    try {
      await platform.removeAttachment(conversation.value.id, attachmentId);
      setAttachments((items) => items.filter((item) => item.id !== attachmentId));
      setDraftAttachmentIds((ids) => ids.filter((id) => id !== attachmentId));
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  const setAttachmentProjectSharing = async (attachmentId: string, enabled: boolean) => {
    if (conversation?.state !== "ready") return;
    setProjectFileBusyId(attachmentId);
    setAttachmentError(null);
    try {
      setProjectFiles(await platform.setProjectFile(
        conversation.value.id,
        attachmentId,
        enabled
      ));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setProjectFileBusyId(null);
    }
  };

  const addProjectFileToConversation = async (attachmentId: string) => {
    if (conversation?.state !== "ready") return;
    setProjectFileBusyId(attachmentId);
    setAttachmentError(null);
    try {
      const nextAttachments = await platform.useProjectFile(
        conversation.value.id,
        attachmentId
      );
      setAttachments(nextAttachments);
      setDraftAttachmentIds((ids) => [...new Set([...ids, attachmentId])]);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setProjectFileBusyId(null);
    }
  };

  const retryAttachment = async (attachmentId: string) => {
    try {
      const updated = await platform.retryAttachment(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  const retryAttachmentContext = async (attachmentId: string) => {
    setAttachmentContextRetryId(attachmentId);
    setAttachmentError(null);
    try {
      const updated = await platform.retryAttachmentContext(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentContextRetryId(null);
    }
  };

  const retryAttachmentSemanticIndex = async (attachmentId: string) => {
    setAttachmentSemanticRetryId(attachmentId);
    setAttachmentError(null);
    try {
      const updated = await platform.retryAttachmentSemanticIndex(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentSemanticRetryId(null);
    }
  };

  useEffect(
    () => () => {
      if (screenCaptureUrlRef.current) {
        URL.revokeObjectURL(screenCaptureUrlRef.current);
      }
      cameraStreamRef.current?.getTracks().forEach((track) => track.stop());
    },
    []
  );

  useEffect(() => {
    if (!cameraOpen) return;
    const video = cameraVideoRef.current;
    const stream = cameraStreamRef.current;
    if (!video || !stream) return;
    video.srcObject = stream;
    void video.play().catch((error) => {
      setCameraError(cameraFailureMessage(error));
      stopCamera();
    });
  }, [cameraOpen]);

  useEffect(() => {
    const currentConversationId =
      conversation?.state === "ready" ? conversation.value.id : null;
    if (cameraConversationId && cameraConversationId !== currentConversationId) {
      stopCamera();
    }
  }, [conversation, cameraConversationId]);

  useEffect(() => {
    if (conversation?.state !== "ready") return;
    const conversationId = conversation.value.id;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          void importAttachmentPaths(conversationId, event.payload.paths);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => setAttachmentError(describeError(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [conversation?.state === "ready" ? conversation.value.id : null]);

  useEffect(() => {
    if (conversation?.state !== "ready") return;
    if (!attachments.some((item) =>
      !["ready", "failed"].includes(item.ingestionStatus)
      || ["pending", "preparing"].includes(item.contextStatus)
      || ["pending", "indexing"].includes(item.semanticIndexStatus)
    )) {
      return;
    }
    const conversationId = conversation.value.id;
    const interval = window.setInterval(() => {
      Promise.all([
        platform.listAttachments(conversationId),
        platform.listProjectFiles(conversationId)
      ])
        .then(([nextAttachments, nextProjectFiles]) => {
          // Una respuesta de la conversación anterior no se pinta en la actual.
          if (selectedConversationRef.current !== conversationId) return;
          setAttachments(nextAttachments);
          setProjectFiles(nextProjectFiles);
        })
        .catch((error) => {
          if (selectedConversationRef.current === conversationId) {
            setAttachmentError(describeError(error));
          }
        });
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    conversation?.state === "ready" ? conversation.value.id : null,
    attachments
      .map((item) =>
        `${item.id}:${item.ingestionStatus}:${item.contextStatus}:${item.chunkCount}`
      )
      .join("|")
  ]);

  return {
    attachments,
    setAttachments,
    projectFiles,
    setProjectFiles,
    draftAttachmentIds,
    setDraftAttachmentIds,
    attachmentBusy,
    attachmentError,
    setAttachmentError,
    projectFileBusyId,
    attachmentContextRetryId,
    attachmentSemanticRetryId,
    selectedAttachments,
    availableProjectFiles,
    screenCaptureBusy,
    screenCapturePreview,
    cropMode,
    setCropMode,
    cropSelection,
    setCropSelection,
    cropStartRef,
    cameraOpen,
    cameraReady,
    setCameraReady,
    cameraBusy,
    cameraError,
    setCameraError,
    cameraConversationId,
    cameraVideoRef,
    chooseAttachments,
    discardScreenCapture,
    beginCropSelection,
    updateCropSelection,
    finishCropSelection,
    applyScreenCaptureCrop,
    takeScreenCapture,
    attachScreenCapture,
    openCamera,
    takeCameraPhoto,
    stopCamera,
    removeAttachment,
    setAttachmentProjectSharing,
    addProjectFileToConversation,
    retryAttachment,
    retryAttachmentContext,
    retryAttachmentSemanticIndex
  };
}
