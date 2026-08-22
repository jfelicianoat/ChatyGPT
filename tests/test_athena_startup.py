from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AthenaStartupTests(unittest.TestCase):
    """El cableado del arranque.

    Lo que queda aquí es lo único que sigue siendo una pregunta sobre el texto: si el BAT
    llama a los dos guiones. Comprobarlo ejecutándolo exigiría arrancar la aplicación
    entera, y la conducta de los guiones ya se prueba corriéndolos, en
    `test_athena_launcher_behaviour`.
    """

    def test_launcher_starts_and_stops_athena_around_the_application(self) -> None:
        launcher = (ROOT / "Arrancar ChatyGPT.bat").read_text(encoding="utf-8")
        self.assertIn("Start-ChatyGPT.ps1", launcher)

        # Athena se levanta y se cierra desde el script de arranque, no desde el BAT.
        arranque = (ROOT / "scripts" / "Start-ChatyGPT.ps1").read_text(encoding="utf-8")
        self.assertIn("Start-AthenaForChatyGPT.ps1", arranque)
        self.assertIn("Stop-AthenaForChatyGPT.ps1", arranque)
        # El cierre va en un `finally`: si la aplicacion se va por las malas, el servicio
        # administrado no puede quedarse vivo con el puerto cogido.
        cierre = arranque.index("Stop-AthenaForChatyGPT.ps1")
        self.assertIn("finally", arranque[:cierre])

        # Athena necesita un modelo que devuelva su decision estructurada; sin decir cual,
        # el broker enruta por su cuenta y un run puede morir en el primer turno.
        self.assertIn("-PreferredModel", arranque)

    @unittest.skipUnless(os.name == "nt", "el servicio administrado es específico de Windows")
    def test_shutdown_terminates_the_durable_managed_process(self) -> None:
        stop_script = ROOT / "scripts" / "Stop-AthenaForChatyGPT.ps1"
        self.assertTrue(stop_script.exists(), "falta el cierre durable de Athena")

        process = subprocess.Popen(
            ["powershell.exe", "-NoProfile", "-Command", "Start-Sleep -Seconds 60"],
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        try:
            started_at = subprocess.check_output(
                [
                    "powershell.exe",
                    "-NoProfile",
                    "-Command",
                    (
                        f"(Get-Process -Id {process.pid}).StartTime.ToUniversalTime()"
                        ".ToString('O')"
                    ),
                ],
                text=True,
            ).strip()
            with tempfile.TemporaryDirectory() as data_dir:
                marker = Path(data_dir) / "athena-managed.json"
                marker.write_text(
                    json.dumps({"pid": process.pid, "startedAt": started_at}),
                    encoding="utf-8",
                )
                subprocess.run(
                    [
                        "powershell.exe",
                        "-NoProfile",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-File",
                        str(stop_script),
                        "-DataDir",
                        data_dir,
                    ],
                    check=True,
                    capture_output=True,
                    text=True,
                )
                process.wait(timeout=5)
                self.assertFalse(marker.exists())
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
