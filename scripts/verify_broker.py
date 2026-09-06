"""Verificación reproducible y sin persistencia de secretos para AI Broker.

Comprueba un **mínimo** de contrato, no una versión exacta. El contrato del
Broker crece de forma aditiva: un 2.10 sirve todo lo que servía un 2.8, así
que exigir igualdad convertía cada versión nueva en un "fallo" que no lo era
—y que además tapaba los fallos de verdad, porque la verificación no pasaba
de la primera comprobación.

Ojo con comparar cadenas: `"2.10" < "2.9"` es cierto y `2.10 < 2.9` no.
"""

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
    "converting",
    "chunking",
    "generating",
    "proposing",
    "evaluating",
    "debating",
    "synthesizing",
    "verifying",
    "waiting_for_memory",
    "waiting_for_dependencies",
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
    "/api/v1/queue": {"get"},
    "/health/ready": {"get"},
}


#: Contrato mínimo que ChatyGPT necesita. No es "el último": es el más antiguo
#: con el que todo lo que hace la aplicación sigue siendo cierto.
MINIMUM_CONTRACT_VERSION = "2.8"


def _contract_at_least(observed: object, required: str) -> bool:
    """Compara versiones de contrato por número, nunca por cadena."""
    if not isinstance(observed, str) or not observed.strip():
        return False
    try:
        left = tuple(int(part) for part in observed.strip().split("."))
        right = tuple(int(part) for part in required.split("."))
    except ValueError:
        return False
    width = max(len(left), len(right))
    return left + (0,) * (width - len(left)) >= right + (0,) * (width - len(right))


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


#: Almacén de credenciales del sistema donde el Broker publica el token de la
#: sesión en marcha (Client_API.md, 3.1). Los dos nombres son parte del
#: contrato: no cambian sin nota en su sección 12.
KEYRING_SERVICE = "ai-broker"
#: El token se regenera EN CADA ARRANQUE del Broker y se publica aquí cuando su
#: configuración lleva `server.publish_session_token: keyring`. No es
#: persistente y no debe cachearse entre reinicios.
KEYRING_SESSION_USER = "session_admin_token"
#: Entrada del OPERADOR, para fijar un token estable. El propio Broker solo la
#: consulta como último recurso, y el contrato pide explícitamente no leerla en
#: su lugar: si hay una sesión publicada, esa es la buena.
KEYRING_OPERATOR_USER = "dashboard_admin_token"


def resolve_token() -> str | None:
    """Resuelve la credencial sin inventarse ninguna fuente.

    Orden: variable de entorno —que es como se fija un token estable desde
    fuera— y después el almacén de credenciales del sistema, primero la sesión
    en marcha y solo después la entrada del operador.

    El orden importa: leer `dashboard_admin_token` antes que
    `session_admin_token` devuelve un token que puede llevar meses caducado
    mientras hay uno válido al lado, y el síntoma es un 403 que parece un fallo
    de integración cuando en realidad es que el Broker se reinició.
    """
    token = os.environ.get("AI_BROKER_ADMIN_TOKEN")
    if token:
        return token
    try:
        import keyring  # type: ignore[import-not-found]
    except Exception:
        return None
    for username in (KEYRING_SESSION_USER, KEYRING_OPERATOR_USER):
        try:
            stored = keyring.get_password(KEYRING_SERVICE, username)
        except Exception:
            return None
        if stored:
            return stored
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
        "output": {"format": "markdown", "language": "es"},
        "generation": {"temperature": 0, "max_output_tokens": 32},
        "model_requirements": {
            "fallback_allowed": True,
            "max_cost_usd": 0,
        },
        "execution": {
            "strategy": "single",
            "preset": "fast",
            "long_context": "fail",
            "timeout_seconds": 120,
        },
        "risk": {"data_classification": "local_only"},
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
    if not _contract_at_least(contract_version, MINIMUM_CONTRACT_VERSION):
        raise VerificationError(
            f"contrato por debajo del mínimo: {contract_version!r} "
            f"(se necesita {MINIMUM_CONTRACT_VERSION} o posterior)"
        )
    ingestion_formats = capabilities.get("ingestion_formats")
    if not isinstance(ingestion_formats, dict) or any(
        not isinstance(group, str)
        or not isinstance(extensions, list)
        or any(not isinstance(extension, str) for extension in extensions)
        for group, extensions in ingestion_formats.items()
    ):
        raise VerificationError(
            "ingestion_formats debe ser un objeto de grupos con listas de extensiones"
        )
    egress = capabilities.get("agent_skills_egress")
    if not isinstance(egress, list) or any(not isinstance(skill, str) for skill in egress):
        raise VerificationError("agent_skills_egress debe ser una lista de nombres")
    if capabilities.get("task_dependencies") is not True:
        raise VerificationError(
            "task_dependencies debe estar anunciado desde el contrato 2.8"
        )
    checks.append(
        Check(
            "capabilities",
            "passed",
            {
                "contract_version": contract_version,
                "derived_data_boundary": capabilities.get("derived_data_boundary"),
                "work_lanes": capabilities.get("work_lanes", []),
                "strategies": capabilities.get("strategies", []),
                "file_ingestion": capabilities.get("file_ingestion"),
                "ingestion_formats": ingestion_formats,
                "sandbox_run_code": capabilities.get("sandbox_run_code"),
                "agent_skills_egress": egress,
                "task_dependencies": capabilities.get("task_dependencies"),
                "long_context_map_reduce": capabilities.get("long_context_map_reduce"),
                # Contrato 2.10: qué puede demostrar este Broker sobre CÓMO
                # ejecutó, y si acepta que el contenido lo vea solo el modelo
                # que responde. Se informan tal cual: no son requisitos, son
                # hechos sobre el Broker que hay enfrente.
                "invocation_contract": capabilities.get("invocation_contract"),
                "prompt_compression_echo": capabilities.get("prompt_compression_echo"),
                "canonical_artifacts": capabilities.get("canonical_artifacts"),
                "auxiliary_invocations": capabilities.get("auxiliary_invocations"),
                "auxiliary_invocations_optout": capabilities.get(
                    "auxiliary_invocations_optout"
                ),
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
