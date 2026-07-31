export type CapturedScreenFrame = {
  blob: Blob;
  width: number;
  height: number;
  displayName: string;
};

export type CropSelection = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export function normalizeCropSelection(
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  minimumSize = 0.02
): CropSelection | null {
  const clamp = (value: number) => Math.min(1, Math.max(0, value));
  const x = clamp(Math.min(startX, endX));
  const y = clamp(Math.min(startY, endY));
  const width = clamp(Math.max(startX, endX)) - x;
  const height = clamp(Math.max(startY, endY)) - y;
  if (width < minimumSize || height < minimumSize) return null;
  return { x, y, width, height };
}

export function constrainedCaptureSize(
  width: number,
  height: number,
  maxEdge = 2_560,
  maxPixels = 4_000_000
): { width: number; height: number } {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    throw new Error("La pantalla seleccionada no tiene unas dimensiones válidas.");
  }
  const edgeScale = Math.min(1, maxEdge / Math.max(width, height));
  const pixelScale = Math.min(1, Math.sqrt(maxPixels / (width * height)));
  const scale = Math.min(edgeScale, pixelScale);
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale))
  };
}

export function captureDisplayName(now = new Date(), prefix = "captura"): string {
  const part = (value: number) => String(value).padStart(2, "0");
  return [
    prefix,
    now.getFullYear(),
    part(now.getMonth() + 1),
    part(now.getDate()),
    `${part(now.getHours())}${part(now.getMinutes())}${part(now.getSeconds())}`
  ].join("-") + ".jpg";
}

function waitForVideoFrame(video: HTMLVideoElement): Promise<void> {
  if (video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA && video.videoWidth > 0) {
    return Promise.resolve();
  }
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      cleanup();
      reject(new Error("La pantalla seleccionada no entregó ninguna imagen."));
    }, 10_000);
    const cleanup = () => {
      window.clearTimeout(timeout);
      video.removeEventListener("loadeddata", loaded);
      video.removeEventListener("error", failed);
    };
    const loaded = () => {
      cleanup();
      resolve();
    };
    const failed = () => {
      cleanup();
      reject(new Error("No se pudo leer la pantalla seleccionada."));
    };
    video.addEventListener("loadeddata", loaded, { once: true });
    video.addEventListener("error", failed, { once: true });
  });
}

function canvasBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (blob) resolve(blob);
        else reject(new Error("No se pudo preparar la captura."));
      },
      "image/jpeg",
      0.9
    );
  });
}

function loadImage(blob: Blob): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    const url = URL.createObjectURL(blob);
    const cleanup = () => URL.revokeObjectURL(url);
    image.onload = () => {
      cleanup();
      resolve(image);
    };
    image.onerror = () => {
      cleanup();
      reject(new Error("No se pudo leer la imagen para recortarla."));
    };
    image.src = url;
  });
}

export async function cropCapturedFrame(
  frame: CapturedScreenFrame,
  selection: CropSelection
): Promise<CapturedScreenFrame> {
  const image = await loadImage(frame.blob);
  const sourceX = Math.round(selection.x * image.naturalWidth);
  const sourceY = Math.round(selection.y * image.naturalHeight);
  const sourceWidth = Math.max(1, Math.round(selection.width * image.naturalWidth));
  const sourceHeight = Math.max(1, Math.round(selection.height * image.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = sourceWidth;
  canvas.height = sourceHeight;
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) {
    throw new Error("No se pudo preparar el recorte.");
  }
  context.drawImage(
    image,
    sourceX,
    sourceY,
    sourceWidth,
    sourceHeight,
    0,
    0,
    sourceWidth,
    sourceHeight
  );
  return {
    blob: await canvasBlob(canvas),
    width: sourceWidth,
    height: sourceHeight,
    displayName: frame.displayName
  };
}

export async function captureVideoFrame(
  video: HTMLVideoElement,
  displayName: string
): Promise<CapturedScreenFrame> {
  await waitForVideoFrame(video);
  const size = constrainedCaptureSize(video.videoWidth, video.videoHeight);
  const canvas = document.createElement("canvas");
  canvas.width = size.width;
  canvas.height = size.height;
  const context = canvas.getContext("2d", { alpha: false });
  if (!context) {
    throw new Error("No se pudo preparar el lienzo de la captura.");
  }
  context.drawImage(video, 0, 0, size.width, size.height);
  return {
    blob: await canvasBlob(canvas),
    width: size.width,
    height: size.height,
    displayName
  };
}

export async function captureScreenFrame(): Promise<CapturedScreenFrame> {
  if (!navigator.mediaDevices?.getDisplayMedia) {
    throw new Error(
      "La captura de pantalla no está disponible en esta versión de WebView2."
    );
  }

  let stream: MediaStream | null = null;
  const video = document.createElement("video");
  try {
    stream = await navigator.mediaDevices.getDisplayMedia({
      audio: false,
      video: { frameRate: { ideal: 1, max: 2 } }
    });
    video.muted = true;
    video.playsInline = true;
    video.srcObject = stream;
    await video.play();
    return await captureVideoFrame(video, captureDisplayName());
  } finally {
    video.pause();
    video.srcObject = null;
    stream?.getTracks().forEach((track) => track.stop());
  }
}
