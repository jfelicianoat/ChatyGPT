"""Verificación reproducible y sin persistencia de secretos para AI Broker 2.5."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import uuid
from dataclasses import dataclass, field
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


TERMINAL_STATES = {"completed", "failed", "cancelled"}
REQUIRED_TASK_STATES = {
    "queued",
    "routing",
    "planning",
    "resource_planning",
    "chunking",
    "generating",
    "proposing",
    "evaluating",
    "debating",
    "synthesizing",
    "verifying",
    "waiting_for_tools",
    *TERMINAL_STATES,
}
REQUIRED_PATHS = {
    "/api/v1/tasks": {"post"},
    "/api/v1/tasks/{task_id}": {"get", "delete"},
    "/api/v1/tasks/{task_id}/tool_results": {"post"},
    "/api/v1/files": {"post"},
    "/api/v1/files/{file_id}": {"get"},
    "/api/v1/capabilities": {"get"},
    "/health/ready": {"get"},
}


class VerificationError(RuntimeError):
    pass


@dataclass
class Check:
    name: str
    status: str
    evidence: dict[str, Any] = field(default_factory=dict)


class BrokerProbe:
    def __init__(self, base_url: str, token: str | None, timeout: float = 15.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.timeout = timeout

    def request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
    ) -> tuple[int, dict[str, Any]]:
        headers = {"Accept": "application/json", "User-Agent": "ChatyGPT-Phase0-Probe/1"}
        if self.token:
            headers["x-admin-token"] = self.token
        body = None
        if payload is not None:
            headers["Content-Type"] = "application/json"
            body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        request = Request(
            f"{self.base_url}{path}",
            data=body,
            headers=headers,
            method=method,
        )
        try:
            with urlopen(request, timeout=self.timeout) as response:
                raw = response.read()
                return response.status, json.loads(raw) if raw else {}
        except HTTPError as error:
            raw = error.read()
            detail = raw.decode("utf-8", errors="replace")[:1_000]
            raise VerificationError(f"{method} {path}: HTTP {error.code}: {detail}") from error
        except (URLError, TimeoutError) as error:
            raise VerificationError(f"{method} {path}: {error}") from error


def resolve_token() -> str | None:
    token = os.environ.get("AI_BROKER_ADMIN_TOKEN")
    if token:
        return token
    try:
        import keyring  # type: ignore[import-not-found]

        return keyring.get_password("ai-broker", "dashboard_admin_token") or None
    except Exception:
        return None


def smoke_payload(idempotency_key: str) -> dict[str, Any]:
    return {
        "idempotency_key": idempotency_key,
        "request_id": f"chatygpt_smoke_{uuid.uuid4().hex}",
        "inference_kind": "chat",
        "content": {
            "prompt": "Responde únicamente: conexión correcta",
            "attachments": [],
            "metadata": {"origin": "chatygpt_phase_0_smoke"},
        },
        "output": {"format": "markdown", "json_schema": None, "language": "es"},
        "generation": {"temperature": 0, "max_output_tokens": 32},
        "model_requirements": {
            "preferred_model": None,
            "target_model": None,
            "fallback_allowed": True,
            "cloud_allowed": False,
            "allowed_providers": ["ollama"],
            "max_cost_usd": 0,
        },
        "execution": {
            "strategy": "single",
            "preset": "fast",
            "long_context": "fail",
            "scheduling": "adaptive",
            "max_proposers": 1,
            "max_judges": 0,
            "max_rounds": 1,
            "timeout_seconds": 120,
            "early_stop": True,
            "selection": {
                "mode": "auto",
                "diversity_policy": "different_families",
                "arbiter_policy": "strongest_available",
                "preferred_arbiter": None,
                "allow_substitution": True,
                "proposers": [],
                "required_proposers": [],
                "arbiter": None,
                "proposer_count": 1,
            },
            "agent": {
                "skills": ["web_search", "fetch_url", "calculator", "current_datetime"],
                "max_iterations": 6,
                "client_tools": [],
            },
            "proposer_skills": [],
        },
        "risk": {"data_classification": "local_only", "human_review_required": False},
        "priority": 100,
        "prompt_compression": "off",
    }


def verify_read_contract(probe: BrokerProbe) -> list[Check]:
    checks: list[Check] = []

    status, live = probe.request("GET", "/health/live")
    checks.append(Check("health_live", "passed", {"http": status, "status": live.get("status")}))

    status, ready = probe.request("GET", "/health/ready")
    checks.append(Check("health_ready", "passed", {"http": status, "status": ready.get("status")}))

    _, capabilities = probe.request("GET", "/api/v1/capabilities")
    contract_version = capabilities.get("contract_version")
    if contract_version != "2.5":
        raise VerificationError(f"versión contractual inesperada: {contract_version!r}")
    checks.append(
        Check(
            "capabilities",
            "passed",
            {
                "contract_version": contract_version,
                "strategies": capabilities.get("strategies", []),
                "file_ingestion": capabilities.get("file_ingestion"),
                "sandbox_run_code": capabilities.get("sandbox_run_code"),
            },
        )
    )

    _, availability = probe.request("GET", "/api/v1/models/availability?only_dispatchable=true")
    items = availability.get("items", [])
    local_dispatchable = [
        item
        for item in items
        if item.get("dispatchable")
        and str(item.get("deployment", "")).lower() in {"local", "bootstrap"}
    ]
    checks.append(
        Check(
            "models",
            "passed" if local_dispatchable else "warning",
            {
                "dispatchable_count": len(items),
                "local_dispatchable_count": len(local_dispatchable),
                "local_models": [
                    {
                        "provider": item.get("provider"),
                        "deployment": item.get("deployment"),
                        "model": item.get("model") or item.get("name"),
                    }
                    for item in local_dispatchable[:10]
                ],
            },
        )
    )

    _, openapi = probe.request("GET", "/openapi.json")
    paths = openapi.get("paths", {})
    missing = {
        path: sorted(methods - set(paths.get(path, {})))
        for path, methods in REQUIRED_PATHS.items()
        if methods - set(paths.get(path, {}))
    }
    if missing:
        raise VerificationError(f"OpenAPI no contiene operaciones requeridas: {missing}")
    serialized = json.dumps(openapi)
    missing_states = sorted(state for state in REQUIRED_TASK_STATES if f'"{state}"' not in serialized)
    if missing_states:
        raise VerificationError(f"OpenAPI no declara estados requeridos: {missing_states}")
    checks.append(
        Check(
            "openapi",
            "passed",
            {
                "openapi": openapi.get("openapi"),
                "api_version": openapi.get("info", {}).get("version"),
                "path_count": len(paths),
            },
        )
    )
    return checks


def verify_smoke_task(probe: BrokerProbe, timeout_seconds: float) -> list[Check]:
    key = f"chatygpt:phase0:{uuid.uuid4()}"
    payload = smoke_payload(key)
    first_status, first = probe.request("POST", "/api/v1/tasks", payload)
    second_status, second = probe.request("POST", "/api/v1/tasks", payload)
    if first.get("task_id") != second.get("task_id"):
        raise VerificationError("la misma clave idempotente produjo task_id diferentes")
    task_id = str(first["task_id"])
    checks = [
        Check(
            "task_idempotency",
            "passed",
            {
                "first_http": first_status,
                "repeat_http": second_status,
                "same_task_id": True,
                "task_id": task_id,
            },
        )
    ]

    deadline = time.monotonic() + timeout_seconds
    observed: list[str] = []
    delay = 0.75
    last: dict[str, Any] = {}
    while time.monotonic() < deadline:
        _, last = probe.request("GET", f"/api/v1/tasks/{task_id}")
        state = str(last.get("status"))
        if not observed or observed[-1] != state:
            observed.append(state)
            delay = 0.75
        if state in TERMINAL_STATES:
            break
        time.sleep(delay)
        delay = min(10.0, delay * 1.7)
    else:
        try:
            probe.request("DELETE", f"/api/v1/tasks/{task_id}")
        finally:
            raise VerificationError(f"timeout esperando tarea {task_id}; cancelación solicitada")

    checks.append(
        Check(
            "task_polling",
            "passed" if last.get("status") == "completed" else "warning",
            {
                "task_id": task_id,
                "terminal_status": last.get("status"),
                "observed_states": observed,
                "has_result": last.get("result") is not None,
                "error": last.get("error"),
            },
        )
    )
    return checks


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="http://127.0.0.1:8765")
    parser.add_argument("--smoke-task", action="store_true")
    parser.add_argument("--task-timeout", type=float, default=180.0)
    args = parser.parse_args()

    report: dict[str, Any] = {
        "base_url": args.base_url,
        "checks": [],
        "overall": "failed",
    }
    try:
        probe = BrokerProbe(args.base_url, resolve_token())
        checks = verify_read_contract(probe)
        if args.smoke_task:
            checks.extend(verify_smoke_task(probe, args.task_timeout))
        report["checks"] = [check.__dict__ for check in checks]
        report["overall"] = (
            "passed"
            if all(check.status == "passed" for check in checks)
            else "passed_with_warnings"
        )
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    except VerificationError as error:
        report["error"] = str(error)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1


if __name__ == "__main__":
    sys.exit(main())

