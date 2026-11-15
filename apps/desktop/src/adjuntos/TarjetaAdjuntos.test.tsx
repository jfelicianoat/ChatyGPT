// @vitest-environment jsdom
/**
 * CH-03: la zona de adjuntos extraída de App.tsx se prueba suelta, con un
 * objeto con la misma forma que devuelve `useAdjuntos`.
 */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { TarjetaAdjuntos } from "./TarjetaAdjuntos";
import type { useAdjuntos } from "./useAdjuntos";
import type { AttachmentView, ConversationView } from "../domain";

afterEach(cleanup);

const conversation: ConversationView = {
  id: "conv-1",
  projectId: null
} as unknown as ConversationView;

type AdjuntosShape = ReturnType<typeof useAdjuntos>;

const baseAdjuntos = (partial: Partial<AdjuntosShape>): AdjuntosShape =>
  ({
    attachments: [],
    setAttachments: vi.fn(),
    projectFiles: [],
    setProjectFiles: vi.fn(),
    draftAttachmentIds: [],
    setDraftAttachmentIds: vi.fn(),
    attachmentBusy: false,
    attachmentError: null,
    setAttachmentError: vi.fn(),
    projectFileBusyId: null,
    attachmentContextRetryId: null,
    attachmentSemanticRetryId: null,
    selectedAttachments: [],
    availableProjectFiles: [],
    screenCaptureBusy: false,
    screenCapturePreview: null,
    cropMode: false,
    setCropMode: vi.fn(),
    cropSelection: null,
    setCropSelection: vi.fn(),
    cropStartRef: { current: null },
    cameraOpen: false,
    cameraReady: false,
    setCameraReady: vi.fn(),
    cameraBusy: false,
    cameraError: null,
    setCameraError: vi.fn(),
    cameraConversationId: null,
    cameraVideoRef: { current: null },
    chooseAttachments: vi.fn(),
    discardScreenCapture: vi.fn(),
    beginCropSelection: vi.fn(),
    updateCropSelection: vi.fn(),
    finishCropSelection: vi.fn(),
    applyScreenCaptureCrop: vi.fn(),
    takeScreenCapture: vi.fn(),
    attachScreenCapture: vi.fn(),
    openCamera: vi.fn(),
    takeCameraPhoto: vi.fn(),
    stopCamera: vi.fn(),
    removeAttachment: vi.fn(),
    setAttachmentProjectSharing: vi.fn(),
    addProjectFileToConversation: vi.fn(),
    retryAttachment: vi.fn(),
    retryAttachmentContext: vi.fn(),
    retryAttachmentSemanticIndex: vi.fn(),
    ...partial
  }) as AdjuntosShape;

const attachment = (patch: Partial<AttachmentView> = {}): AttachmentView => ({
  id: "att-1",
  displayName: "informe.pdf",
  sizeBytes: 2048,
  ingestionStatus: "ready",
  contextStatus: "ready",
  semanticIndexStatus: "ready",
  chunkCount: 3,
  indexedCharacters: 1200,
  semanticIndexedChunks: 3,
  ...patch
} as unknown as AttachmentView);

it("adjunta: sin turno en curso deja usar los tres botones de captura e ingesta", () => {
  const elegir = vi.fn();
  const capturar = vi.fn();
  const camara = vi.fn();
  render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({
        chooseAttachments: elegir,
        takeScreenCapture: capturar,
        openCamera: camara
      })}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  const adjuntar = screen.getByRole("button", { name: "+ Adjuntar archivos" });
  expect((adjuntar as HTMLButtonElement).disabled).toBe(false);
  expect((screen.getByRole("button", { name: "Capturar pantalla" }) as HTMLButtonElement).disabled).toBe(false);
  expect((screen.getByRole("button", { name: "Usar cámara" }) as HTMLButtonElement).disabled).toBe(false);
  void adjuntar.click();
  expect(elegir).toHaveBeenCalledTimes(1);
});

it("adjunta: con un turno en curso bloquea editar los adjuntos", () => {
  render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({})}
      conversation={conversation}
      currentTurnBlocks={true}
    />
  );
  expect((screen.getByRole("button", { name: "+ Adjuntar archivos" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Capturar pantalla" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "Usar cámara" }) as HTMLButtonElement).disabled).toBe(true);
});

it("adjunta: sin vista previa no hay sección de captura; con ella se adjunta", async () => {
  const user = userEvent.setup();
  const adjuntarCaptura = vi.fn();
  const preview = {
    blob: new Blob(["x"], { type: "image/png" }),
    displayName: "captura.png",
    width: 800,
    height: 600,
    conversationId: "conv-1",
    previewUrl: "blob:preview",
    source: "screen"
  } as AdjuntosShape["screenCapturePreview"];
  const { rerender } = render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({})}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  expect(screen.queryByRole("region", { name: "Vista previa de captura" })).toBeNull();

  rerender(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({ screenCapturePreview: preview, attachScreenCapture: adjuntarCaptura })}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  expect(screen.getByRole("region", { name: "Vista previa de captura" }).textContent)
    .toContain("800 × 600");
  await user.click(screen.getByRole("button", { name: "Adjuntar captura" }));
  expect(adjuntarCaptura).toHaveBeenCalledTimes(1);
});

it("adjunta: la vista previa de otra conversación no se pinta", () => {
  const preview = {
    blob: new Blob(["x"], { type: "image/png" }),
    displayName: "captura.png",
    width: 800,
    height: 600,
    conversationId: "conv-otra",
    previewUrl: "blob:preview",
    source: "screen"
  } as AdjuntosShape["screenCapturePreview"];
  render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({ screenCapturePreview: preview })}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  expect(screen.queryByRole("region", { name: "Vista previa de captura" })).toBeNull();
});

it("adjunta: lista los chips, alterna la selección y quita con confirmación visual", async () => {
  const user = userEvent.setup();
  const setSeleccion = vi.fn();
  const quitar = vi.fn();
  render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({
        attachments: [attachment()],
        draftAttachmentIds: ["att-1"],
        setDraftAttachmentIds: setSeleccion,
        removeAttachment: quitar
      })}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  const lista = screen.getByLabelText("Archivos de la conversación");
  expect(lista.textContent).toContain("informe.pdf");
  expect(lista.textContent).toContain("2.0 KB");

  await user.click(screen.getByTitle("Desactivar para los próximos mensajes"));
  expect(setSeleccion).toHaveBeenCalledTimes(1);

  await user.click(screen.getByRole("button", { name: "Quitar informe.pdf" }));
  expect(quitar).toHaveBeenCalledWith("att-1");
});

it("adjunta: sin archivos de proyecto no pinta la biblioteca; con ellos ofrece usarlos", async () => {
  const user = userEvent.setup();
  const usar = vi.fn();
  const { rerender } = render(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({})}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  expect(screen.queryByLabelText("Archivos del proyecto")).toBeNull();

  rerender(
    <TarjetaAdjuntos
      adjuntos={baseAdjuntos({
        availableProjectFiles: [attachment({ id: "att-9", displayName: "guia.md" })],
        addProjectFileToConversation: usar
      })}
      conversation={conversation}
      currentTurnBlocks={false}
    />
  );
  const usarBoton = screen.getByRole("button", { name: "Usar en este chat" });
  await user.click(usarBoton);
  expect(usar).toHaveBeenCalledWith("att-9");
});
