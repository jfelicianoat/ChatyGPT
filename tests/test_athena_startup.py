from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class AthenaStartupTests(unittest.TestCase):
    def test_launcher_starts_athena_before_opening_chatygpt(self) -> None:
        launcher = (ROOT / "Arrancar ChatyGPT.bat").read_text(encoding="utf-8")
        script = (ROOT / "scripts" / "Start-AthenaForChatyGPT.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn("Start-AthenaForChatyGPT.ps1", launcher)
        self.assertIn("/v1/health", script)
        self.assertIn("pythonw.exe", script)
        self.assertIn("athena_service", script)
        self.assertIn("ProtectedData]::Protect", script)
        self.assertIn("CHATYGPT_MANAGED_ATHENA_PID", script)
        self.assertIn("Stop-AthenaForChatyGPT.ps1", launcher)
        self.assertIn("athena-managed.json", script)
        self.assertIn("startedAt", script)
        self.assertNotIn("dsfdsjk", script)

    def test_existing_external_service_is_never_terminated(self) -> None:
        script = (ROOT / "scripts" / "Start-AthenaForChatyGPT.ps1").read_text(
            encoding="utf-8"
        )
        attach_branch = script.split("if (Test-AthenaHealth)", maxsplit=1)[1].split(
            "if (-not $AthenaRoot)", maxsplit=1
        )[0]

        self.assertNotIn("Stop-Process", attach_branch)
        self.assertIn("Athena ya está disponible", attach_branch)
        self.assertNotIn("throw", attach_branch.lower())
        self.assertIn("Podrás guardarla en la sección Athena", attach_branch)

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
