"""El estado de tarea que ChatyGPT asume, validado contra el contrato del Broker.

`contracts/broker/2.8/task-state.response.json` conserva el núcleo del esquema que
publica AI Broker (`tests/fixtures/broker_task_state_response.schema.json`). No
se edita aquí: si el Broker cambia, se vuelve a copiar y estas pruebas dicen qué
se rompe.

Lo que se valida son las **respuestas que ChatyGPT da por buenas**: las mismas
formas que aparecen en las pruebas de Rust y en el Broker simulado. Si una de
ellas dejara de cumplir el contrato, significaría que las pruebas se están
apoyando en algo que el Broker no promete, y pasarían en verde mientras la
aplicación falla contra el Broker real.

El esquema recuerda que `progress` y `result` admiten claves adicionales que
**no son contrato**: por eso ChatyGPT solo construye lógica sobre las declaradas.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "contracts" / "broker" / "2.8" / "task-state.response.json"


def load_validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema)


def inference_state(**overrides) -> dict:
    """Estado de inferencia como el que devuelve el Broker."""
    state = {
        "task_id": "remote-1",
        "kind": "inference",
        "status": "generating",
        "request_id": "request-remote-1",
        "created_at": "2026-08-06T10:00:00Z",
        "updated_at": "2026-08-06T10:00:05Z",
        "execution_strategy": "single",
        "execution_preset": "fast",
        "selection_mode": "auto",
        "progress": {
            "phase": "generating",
            "invocations_completed": 1,
            "invocations_total": 3,
        },
        "result": None,
        "error": None,
    }
    state.update(overrides)
    return state


class TaskStateContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.validator = load_validator()

    def assert_valid(self, state: dict) -> None:
        errors = sorted(self.validator.iter_errors(state), key=lambda error: error.path)
        self.assertEqual(
            [],
            [f"{list(error.path)}: {error.message}" for error in errors],
        )

    def assert_invalid(self, state: dict) -> None:
        self.assertFalse(
            self.validator.is_valid(state),
            "el esquema debería rechazar este estado",
        )

    def test_schema_is_the_untouched_copy_of_the_broker_contract(self) -> None:
        """El identificador delata una copia editada por nuestra parte."""
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        self.assertEqual(
            "https://ai-broker.local/contracts/broker/2.8/task-state.response.json",
            schema["$id"],
        )

    def test_inference_states_that_chatygpt_parses_are_valid(self) -> None:
        self.assert_valid(inference_state())
        self.assert_valid(inference_state(status="queued"))
        self.assert_valid(
            inference_state(
                status="completed",
                result={"result_markdown": "La normativa exige contrato previo."},
            )
        )

    def test_dependency_wait_is_non_terminal_contract_state(self) -> None:
        self.assert_valid(
            inference_state(
                status="waiting_for_dependencies",
                progress={
                    "phase": "waiting_for_dependencies",
                    "invocations_completed": 0,
                    "invocations_total": 1,
                },
            )
        )

    def test_an_ingestion_state_may_omit_execution_and_counters(self) -> None:
        """Es lo que afirma la prueba de contrato de Rust y el esquema confirma:
        una conversión de fichero no tiene estrategia, preset ni modo."""
        self.assert_valid(
            {
                "task_id": "file-1",
                "kind": "ingestion",
                "status": "converting",
                "request_id": None,
                "created_at": "2026-08-06T10:00:00Z",
                "updated_at": "2026-08-06T10:00:01Z",
                "execution_strategy": None,
                "execution_preset": None,
                "selection_mode": None,
                "progress": {"phase": "converting"},
                "result": None,
                "error": None,
            }
        )

    def test_inference_always_carries_its_invocation_counters(self) -> None:
        """La regla que separa los dos carriles: en inferencia los contadores
        son contrato, así que ChatyGPT puede mostrarlos sin comprobar nada."""
        state = inference_state()
        del state["progress"]["invocations_completed"]
        self.assert_invalid(state)

    def test_a_paused_agent_must_publish_its_pending_calls(self) -> None:
        """Es la base de las subtareas reales: sin `pending_tool_calls` no hay
        nada que ejecutar ni que registrar como paso."""
        paused = inference_state(
            status="waiting_for_tools",
            execution_strategy="agent",
            progress={
                "phase": "generating",
                "invocations_completed": 2,
                "invocations_total": 2,
                "agent_iteration": 3,
                "agent_max_iterations": 12,
            },
            result={
                "status": "waiting_for_tools",
                "pending_tool_calls": [
                    {
                        "id": "call_1",
                        "name": "fetch_url",
                        "arguments": {"url": "https://example.org/informe"},
                    }
                ],
            },
        )
        self.assert_valid(paused)

        # Sin la lista de llamadas, el estado no cumple el contrato.
        sin_llamadas = json.loads(json.dumps(paused))
        sin_llamadas["result"] = {"status": "waiting_for_tools"}
        self.assert_invalid(sin_llamadas)

        # Cada llamada necesita identificador, nombre y argumentos: los tres
        # son lo que ChatyGPT convierte en una subtarea visible.
        for campo in ("id", "name", "arguments"):
            incompleta = json.loads(json.dumps(paused))
            del incompleta["result"]["pending_tool_calls"][0][campo]
            self.assert_invalid(incompleta)

    def test_a_paused_agent_reports_the_loop_iteration(self) -> None:
        """`agent_iteration` es lo que permite dibujar avance real en vez de
        una barra inventada."""
        paused = inference_state(
            status="waiting_for_tools",
            execution_strategy="agent",
            progress={
                "phase": "generating",
                "invocations_completed": 1,
                "invocations_total": 1,
            },
            result={
                "status": "waiting_for_tools",
                "pending_tool_calls": [
                    {"id": "call_1", "name": "fetch_url", "arguments": {}}
                ],
            },
        )
        self.assert_invalid(paused)

    def test_a_failure_always_says_whether_retrying_makes_sense(self) -> None:
        """`taskFailureSummary` se apoya en `retryable`; el esquema garantiza
        que está, de modo que la interfaz nunca tiene que suponerlo."""
        failed = inference_state(
            status="failed",
            error={
                "code": "RECOVERY_AMBIGUOUS_REMOTE_CALL",
                "message": "La llamada remota quedó en estado desconocido",
                "retryable": False,
            },
        )
        self.assert_valid(failed)

        for campo in ("code", "message", "retryable"):
            incompleto = json.loads(json.dumps(failed))
            del incompleto["error"][campo]
            self.assert_invalid(incompleto)

    def test_unknown_extra_keys_are_tolerated_but_are_not_contract(self) -> None:
        """El esquema admite claves adicionales a propósito. ChatyGPT las
        ignora: construir lógica sobre ellas sería apoyarse en algo que el
        Broker puede retirar sin cambiar de versión."""
        self.assert_valid(
            inference_state(
                progress={
                    "phase": "generating",
                    "invocations_completed": 1,
                    "invocations_total": 3,
                    "detalle_interno": {"cualquier": "cosa"},
                }
            )
        )


if __name__ == "__main__":
    unittest.main()
