"""Lo que hace el lanzador de Athena, ejecutándolo.

Las pruebas que había comprobaban que el script *contuviera* ciertas palabras. Eso detecta
un borrado accidental y poco más: un cambio que rompa el arranque conservando el
vocabulario pasa igual, y un renombrado inofensivo falla.

Aquí se ejecuta el script contra una Athena de mentira que responde lo que haga falta, y
se mira lo que el script hace: si mata procesos ajenos, si toca credenciales que no son
suyas, y qué le deja dicho a la aplicación que arranca después.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "Start-AthenaForChatyGPT.ps1"


class AthenaDeMentira:
    """Un servidor que contesta como Athena, con la credencial que se le diga.

    Deliberadamente no es un doble del cliente sino un socket de verdad: lo que se está
    probando es un script de PowerShell hablando HTTP, y un doble en Python no ejercitaría
    ni `Invoke-RestMethod` ni el manejo de un 401 que hace.
    """

    def __init__(self, credencial_valida: str | None) -> None:
        self.credencial_valida = credencial_valida
        self.rutas_pedidas: list[str] = []
        servidor = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:  # noqa: N802 - lo impone BaseHTTPRequestHandler
                servidor.rutas_pedidas.append(self.path)
                if self.path == "/v1/health":
                    return self._responder(
                        200, {"status": "ok", "wire_version": 1, "runs": 0}
                    )
                if self.path == "/v1/auth/check":
                    presentada = self.headers.get("Authorization", "").removeprefix("Bearer ")
                    if servidor.credencial_valida and presentada == servidor.credencial_valida:
                        return self._responder(200, {"authenticated": True, "wire_version": 1})
                    return self._responder(
                        401, {"error": {"code": "unauthorized", "message": "no"}}
                    )
                return self._responder(404, {"error": {"code": "not_found", "message": "no"}})

            def _responder(self, estado: int, cuerpo: dict[str, object]) -> None:
                carga = json.dumps(cuerpo).encode()
                self.send_response(estado)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(carga)))
                self.end_headers()
                self.wfile.write(carga)

            def log_message(self, *_: object) -> None:
                """Sin ruido en la salida de las pruebas."""

        self._httpd = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.puerto = self._httpd.server_address[1]
        self._hilo = threading.Thread(target=self._httpd.serve_forever, daemon=True)

    def __enter__(self) -> AthenaDeMentira:
        self._hilo.start()
        return self

    def __exit__(self, *_: object) -> None:
        self._httpd.shutdown()
        self._httpd.server_close()

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.puerto}"


def _guardar_credencial(destino: Path, token: str) -> None:
    """Deja una credencial cifrada donde el script la busca."""
    destino.parent.mkdir(parents=True, exist_ok=True)
    guion = (
        "Add-Type -AssemblyName System.Security; "
        f"$plain = [Text.Encoding]::UTF8.GetBytes('{token}'); "
        "$protegido = [Security.Cryptography.ProtectedData]::Protect("
        "$plain, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser); "
        f"[IO.File]::WriteAllBytes('{destino}', $protegido)"
    )
    subprocess.run(
        ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", guion],
        check=True,
        capture_output=True,
    )


def _ejecutar(url: str, local_app_data: Path, athena_root: Path | None = None):
    entorno = dict(os.environ)
    entorno["LOCALAPPDATA"] = str(local_app_data)
    argumentos = [
        "powershell.exe",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(SCRIPT),
        "-BrokerBaseUrl",
        "http://127.0.0.1:9",
        "-BrokerToken",
        "broker-de-prueba",
        "-AthenaBaseUrl",
        url,
    ]
    if athena_root is not None:
        argumentos += ["-AthenaRoot", str(athena_root)]
    # `env` de verdad, no construido y olvidado: sin pasarlo, el script lee el
    # LOCALAPPDATA real de quien ejecuta las pruebas y las hace mirar —y potencialmente
    # tocar— su credencial de verdad. Costó dos fallos entender que el escenario que
    # fallaba no era el que estaba escrito.
    return subprocess.run(
        argumentos,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=90,
        env=entorno,
    )


class LauncherBehaviourTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporal = tempfile.TemporaryDirectory()
        self.local = Path(self._temporal.name)
        self.credencial = (
            self.local / "es.jfeliciano.chatygpt" / "credentials" / "athena-token.dpapi"
        )
        self.estado_gestionado = self.local / "es.jfeliciano.chatygpt" / "athena-managed.json"
        self.addCleanup(self._temporal.cleanup)

    def test_un_servicio_ya_levantado_con_credencial_valida_se_reutiliza(self) -> None:
        with AthenaDeMentira("la-buena") as athena:
            _guardar_credencial(self.credencial, "la-buena")
            antes = self.credencial.read_bytes()

            resultado = _ejecutar(athena.url, self.local)

            self.assertEqual(resultado.returncode, 0, resultado.stderr)
            self.assertIn("/v1/auth/check", athena.rutas_pedidas)
            # Se busca un trozo sin acentos a propósito: PowerShell escribe en la
            # página de códigos de la consola, no en UTF-8, y afirmar sobre letras
            # acentuadas haría que la prueba dependiera de la configuración regional
            # de quien la ejecuta en vez de la conducta del script.
            self.assertIn("disponible en", resultado.stdout)
            # No se toca una credencial que funciona.
            self.assertEqual(self.credencial.read_bytes(), antes)

    def test_una_credencial_que_no_vale_se_avisa_y_no_se_reacuna(self) -> None:
        # El caso que se veía como «conectado» y fallaba en cada operación: Athena viva,
        # credencial guardada de otra sesión suya.
        with AthenaDeMentira("la-que-usa-athena") as athena:
            _guardar_credencial(self.credencial, "la-que-tenemos-guardada")
            antes = self.credencial.read_bytes()

            resultado = _ejecutar(athena.url, self.local)

            salida = resultado.stdout + resultado.stderr
            self.assertIn("rechaza la credencial", salida)
            self.assertIn("vincularla", salida)
            # Y sobre todo: no se genera otra. La credencial de un proceso está fijada
            # mientras vive, así que reacuñarla no arreglaría nada y pisaría la sesión de
            # quien sí la tenga bien.
            self.assertEqual(self.credencial.read_bytes(), antes)

    def test_un_servicio_ajeno_nunca_se_mata(self) -> None:
        # Que ChatyGPT encuentre Athena arrancada no le da derecho a pararla: puede
        # estarla usando otra cosa, o haberla arrancado una persona a mano.
        with AthenaDeMentira("la-buena") as athena:
            _guardar_credencial(self.credencial, "la-buena")

            _ejecutar(athena.url, self.local)
            antes = len(athena.rutas_pedidas)

            # La prueba es que sigue en pie: se le vuelve a hablar y contesta. Comprobar
            # que el guion no contiene cierta palabra diría lo mismo mientras nadie
            # renombre nada, y dejaría de decirlo el día que se reescriba igual de bien.
            _ejecutar(athena.url, self.local)

            self.assertGreater(len(athena.rutas_pedidas), antes)

    def test_no_se_declara_gestionada_una_instancia_que_no_arranco_el_script(self) -> None:
        with AthenaDeMentira("la-buena") as athena:
            _guardar_credencial(self.credencial, "la-buena")

            _ejecutar(athena.url, self.local)

            self.assertFalse(
                self.estado_gestionado.exists(),
                "se apuntó como propia una Athena que el script no arrancó",
            )

    def test_sin_credencial_guardada_se_pide_sin_dar_por_rota_la_conexion(self) -> None:
        with AthenaDeMentira("la-buena") as athena:
            resultado = _ejecutar(athena.url, self.local)

            salida = resultado.stdout + resultado.stderr
            self.assertEqual(resultado.returncode, 0, resultado.stderr)
            self.assertIn("no tiene su credencial", salida)
            # Sin credencial no hay nada que comprobar, y comprobarla igualmente sería
            # preguntar por una que no existe.
            self.assertNotIn("/v1/auth/check", athena.rutas_pedidas)

    def test_una_instalacion_de_athena_incompleta_se_explica(self) -> None:
        # Servicio ausente: el script intenta arrancarlo y no puede. Lo que importa es
        # que diga qué falta en vez de morir con un error de rutas.
        vacio = Path(self._temporal.name) / "athena-sin-entorno"
        vacio.mkdir()
        # Un puerto que se reserva y se suelta: nadie escucha ahí, que es el escenario.
        # Dejarlo reservado y sin servir filtraría un socket por cada ejecución.
        with AthenaDeMentira(None) as libre:
            url_muerta = libre.url
        resultado = _ejecutar(url_muerta, self.local, athena_root=vacio)

        self.assertNotEqual(resultado.returncode, 0)
        self.assertIn("entorno Python de Athena", resultado.stderr)


if __name__ == "__main__":
    unittest.main()
