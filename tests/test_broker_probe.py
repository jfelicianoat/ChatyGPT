from __future__ import annotations

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from scripts.verify_broker import BrokerProbe, smoke_payload, verify_read_contract, verify_smoke_task


class ContractHandler(BaseHTTPRequestHandler):
    task_id = "task-contract-1"
    create_calls = 0

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def send_json(self, status: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/health/live":
            return self.send_json(200, {"status": "live"})
        if self.path == "/health/ready":
            return self.send_json(200, {"status": "ready"})
        if self.path == "/api/v1/capabilities":
            return self.send_json(
                200,
                {
                    "contract_version": "2.5",
                    "strategies": ["single", "agent"],
                    "file_ingestion": True,
                    "sandbox_run_code": False,
                },
            )
        if self.path.startswith("/api/v1/models/availability"):
            return self.send_json(
                200,
                {
                    "items": [
                        {
                            "provider": "ollama",
                            "deployment": "local",
                            "model": "test",
                            "dispatchable": True,
                        }
                    ]
                },
            )
        if self.path == "/openapi.json":
            paths = {
                path: {method: {} for method in methods}
                for path, methods in {
                    "/api/v1/tasks": {"post"},
                    "/api/v1/tasks/{task_id}": {"get", "delete"},
                    "/api/v1/tasks/{task_id}/tool_results": {"post"},
                    "/api/v1/files": {"post"},
                    "/api/v1/files/{file_id}": {"get"},
                    "/api/v1/capabilities": {"get"},
                    "/health/ready": {"get"},
                }.items()
            }
            return self.send_json(
                200,
                {
                    "openapi": "3.1.0",
                    "info": {"version": "0.1.0"},
                    "paths": paths,
                    "components": {
                        "schemas": {
                            "TaskStatus": {
                                "enum": [
                                    "queued", "routing", "planning", "resource_planning",
                                    "chunking", "generating", "proposing", "evaluating",
                                    "debating", "synthesizing", "verifying", "waiting_for_tools",
                                    "completed", "failed", "cancelled",
                                ]
                            }
                        }
                    },
                },
            )
        if self.path == f"/api/v1/tasks/{self.task_id}":
            return self.send_json(
                200,
                {
                    "task_id": self.task_id,
                    "status": "completed",
                    "result": {"result_markdown": "conexión correcta"},
                    "error": None,
                },
            )
        self.send_json(404, {"detail": "not found"})

    def do_POST(self) -> None:
        if self.path == "/api/v1/tasks":
            type(self).create_calls += 1
            length = int(self.headers.get("Content-Length", 0))
            json.loads(self.rfile.read(length))
            return self.send_json(
                202 if self.create_calls == 1 else 200,
                {"task_id": self.task_id, "status": "queued"},
            )
        self.send_json(404, {"detail": "not found"})


class BrokerProbeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), ContractHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.probe = BrokerProbe(f"http://127.0.0.1:{cls.server.server_port}", "secret")

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def test_read_contract(self) -> None:
        checks = verify_read_contract(self.probe)
        self.assertTrue(all(check.status == "passed" for check in checks))

    def test_idempotent_smoke_task(self) -> None:
        ContractHandler.create_calls = 0
        checks = verify_smoke_task(self.probe, timeout_seconds=2)
        self.assertEqual("passed", checks[0].status)
        self.assertTrue(checks[0].evidence["same_task_id"])
        self.assertEqual("completed", checks[1].evidence["terminal_status"])

    def test_payload_is_local_and_cost_bounded(self) -> None:
        payload = smoke_payload("stable")
        self.assertEqual("local_only", payload["risk"]["data_classification"])
        self.assertFalse(payload["model_requirements"]["cloud_allowed"])
        self.assertEqual(0, payload["model_requirements"]["max_cost_usd"])


if __name__ == "__main__":
    unittest.main()

