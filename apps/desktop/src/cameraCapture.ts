export function cameraFailureMessage(error: unknown): string {
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError" || error.name === "SecurityError") {
      return "Windows ha bloqueado el acceso a la cámara. Revisa Configuración > Privacidad y seguridad > Cámara y vuelve a intentarlo.";
    }
    if (error.name === "NotFoundError" || error.name === "DevicesNotFoundError") {
      return "No se ha encontrado ninguna cámara disponible.";
    }
    if (error.name === "NotReadableError" || error.name === "TrackStartError") {
      return "La cámara está siendo utilizada por otra aplicación o no puede iniciarse.";
    }
    if (error.name === "OverconstrainedError") {
      return "La cámara no admite la configuración de vídeo solicitada.";
    }
  }
  return error instanceof Error ? error.message : String(error);
}

export async function openCameraStream(): Promise<MediaStream> {
  if (!navigator.mediaDevices?.getUserMedia) {
    throw new Error("La cámara no está disponible en esta versión de WebView2.");
  }
  return navigator.mediaDevices.getUserMedia({
    audio: false,
    video: {
      facingMode: { ideal: "user" },
      width: { ideal: 1_920 },
      height: { ideal: 1_080 }
    }
  });
}
