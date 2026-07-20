from __future__ import annotations

import json
import sqlite3
import tempfile
import unittest
from contextlib import closing
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MIGRATION = ROOT / "apps" / "desktop" / "src-tauri" / "migrations" / "0001_initial.sql"
CONTRACT = ROOT / "contracts" / "broker" / "2.5" / "single-task.request.json"
RECOVERY_QUERY = (
    ROOT
    / "apps"
    / "desktop"
    / "src-tauri"
    / "queries"
    / "recover_non_terminal_tasks.sql"
)
WINDOWS_ICON = (
    ROOT / "apps" / "desktop" / "src-tauri" / "icons" / "icon.ico"
)
NODE_TSCONFIG = ROOT / "tsconfig.node.json"


class MigrationTests(unittest.TestCase):
    def connect(self, path: Path) -> sqlite3.Connection:
        connection = sqlite3.connect(path)
        connection.execute("PRAGMA foreign_keys = ON")
        return connection

    def test_initial_migration_is_atomic_and_repeatable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            sql = MIGRATION.read_text(encoding="utf-8")
            with closing(self.connect(path)) as connection:
                connection.executescript(sql)
                connection.executescript(sql)
                connection.execute("PRAGMA user_version = 1")
                integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
                foreign_keys = connection.execute("PRAGMA foreign_key_check").fetchall()
                tables = {
                    row[0]
                    for row in connection.execute(
                        "SELECT name FROM sqlite_schema WHERE type='table'"
                    )
                }
            self.assertEqual("ok", integrity)
            self.assertEqual([], foreign_keys)
            self.assertTrue(
                {
                    "conversations",
                    "messages",
                    "message_parts",
                    "broker_tasks",
                    "context_snapshots",
                    "confirmation_requests",
                    "audit_events",
                    "app_settings",
                }.issubset(tables)
            )

    def test_task_idempotency_and_message_order_are_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            with closing(self.connect(path)) as connection:
                connection.executescript(MIGRATION.read_text(encoding="utf-8"))
                connection.execute(
                    "INSERT INTO conversations(id, title) VALUES (?, ?)",
                    ("conversation-1", "Prueba"),
                )
                connection.execute(
                    """
                    INSERT INTO broker_tasks(
                        id, idempotency_key, request_json, remote_status
                    ) VALUES (?, ?, ?, ?)
                    """,
                    ("task-1", "stable-key", "{}", "queued"),
                )
                with self.assertRaises(sqlite3.IntegrityError):
                    connection.execute(
                        """
                        INSERT INTO broker_tasks(
                            id, idempotency_key, request_json, remote_status
                        ) VALUES (?, ?, ?, ?)
                        """,
                        ("task-2", "stable-key", "{}", "queued"),
                    )
                connection.execute(
                    """
                    INSERT INTO messages(
                        id, conversation_id, role, status, sequence_no
                    ) VALUES (?, ?, ?, ?, ?)
                    """,
                    ("message-1", "conversation-1", "user", "complete", 1),
                )
                with self.assertRaises(sqlite3.IntegrityError):
                    connection.execute(
                        """
                        INSERT INTO messages(
                            id, conversation_id, role, status, sequence_no
                        ) VALUES (?, ?, ?, ?, ?)
                        """,
                        ("message-2", "conversation-1", "assistant", "complete", 1),
                    )

    def test_recovery_only_claims_non_terminal_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            with closing(self.connect(path)) as connection:
                connection.executescript(MIGRATION.read_text(encoding="utf-8"))
                for task_id, status, state in (
                    ("active", "generating", "polling"),
                    ("complete", "completed", "terminal"),
                    ("failed", "failed", "terminal"),
                    ("cancelled", "cancelled", "terminal"),
                    ("orphaned", "unknown_remote", "orphaned"),
                ):
                    connection.execute(
                        """
                        INSERT INTO broker_tasks(
                            id, idempotency_key, request_json, remote_status, local_state
                        ) VALUES (?, ?, '{}', ?, ?)
                        """,
                        (task_id, f"key-{task_id}", status, state),
                    )
                changed = connection.execute(
                    RECOVERY_QUERY.read_text(encoding="utf-8")
                ).rowcount
                states = dict(
                    connection.execute(
                        "SELECT id, local_state FROM broker_tasks ORDER BY id"
                    )
                )
            self.assertEqual(1, changed)
            self.assertEqual("recovery_pending", states["active"])
            self.assertEqual("terminal", states["complete"])
            self.assertEqual("terminal", states["failed"])
            self.assertEqual("terminal", states["cancelled"])
            self.assertEqual("orphaned", states["orphaned"])

    def test_remote_identity_and_request_survive_recovery(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            with closing(self.connect(path)) as connection:
                connection.executescript(MIGRATION.read_text(encoding="utf-8"))
                request = json.dumps(
                    {
                        "idempotency_key": "stable-key",
                        "content": {"prompt": "durable"},
                    }
                )
                connection.execute(
                    """
                    INSERT INTO broker_tasks(
                        id, idempotency_key, request_json, remote_status, local_state
                    ) VALUES ('local-1', 'stable-key', ?, 'not_submitted', 'created')
                    """,
                    (request,),
                )
                connection.execute(
                    """
                    UPDATE broker_tasks
                    SET remote_task_id = 'remote-1',
                        remote_status = 'generating',
                        local_state = 'polling'
                    WHERE id = 'local-1'
                    """
                )

                changed = connection.execute(
                    RECOVERY_QUERY.read_text(encoding="utf-8")
                ).rowcount
                recovered = connection.execute(
                    """
                    SELECT remote_task_id, idempotency_key, request_json, local_state
                    FROM broker_tasks WHERE id = 'local-1'
                    """
                ).fetchone()

                connection.execute(
                    """
                    UPDATE broker_tasks
                    SET remote_status = 'completed',
                        local_state = 'terminal',
                        result_json = '{"result_markdown":"ok"}'
                    WHERE id = 'local-1'
                    """
                )
                changed_after_terminal = connection.execute(
                    RECOVERY_QUERY.read_text(encoding="utf-8")
                ).rowcount

            self.assertEqual(1, changed)
            self.assertEqual("remote-1", recovered[0])
            self.assertEqual("stable-key", recovered[1])
            self.assertEqual(json.loads(request), json.loads(recovered[2]))
            self.assertEqual("recovery_pending", recovered[3])
            self.assertEqual(0, changed_after_terminal)

    def test_chat_turn_is_atomic_and_context_is_traceable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            with closing(self.connect(path)) as connection:
                connection.executescript(MIGRATION.read_text(encoding="utf-8"))
                connection.execute(
                    "INSERT INTO conversations(id, title) VALUES ('conv-1', 'Nueva conversación')"
                )
                context = [
                    {"message_id": "user-1", "role": "user", "text": "Hola"}
                ]
                request = {
                    "idempotency_key": "turn-key-1",
                    "content": {"prompt": "Hola"},
                }

                with connection:
                    connection.execute(
                        """
                        INSERT INTO messages(
                            id, conversation_id, role, status, sequence_no
                        ) VALUES ('user-1', 'conv-1', 'user', 'complete', 1)
                        """
                    )
                    connection.execute(
                        """
                        INSERT INTO message_parts(
                            id, message_id, ordinal, kind, content_text
                        ) VALUES ('part-user-1', 'user-1', 0, 'text', 'Hola')
                        """
                    )
                    connection.execute(
                        """
                        INSERT INTO messages(
                            id, conversation_id, role, status, sequence_no
                        ) VALUES ('assistant-1', 'conv-1', 'assistant', 'pending', 2)
                        """
                    )
                    connection.execute(
                        """
                        INSERT INTO broker_tasks(
                            id, conversation_id, request_message_id,
                            response_message_id, idempotency_key, request_json,
                            remote_status, local_state
                        ) VALUES (
                            'task-1', 'conv-1', 'user-1', 'assistant-1',
                            'turn-key-1', ?, 'not_submitted', 'created'
                        )
                        """,
                        (json.dumps(request),),
                    )
                    connection.execute(
                        "UPDATE messages SET broker_task_id = 'task-1' WHERE id = 'assistant-1'"
                    )
                    connection.execute(
                        """
                        INSERT INTO context_snapshots(
                            id, broker_task_id, strategy_version,
                            estimated_tokens, final_context_json
                        ) VALUES ('context-1', 'task-1', 'window-v1', 1, ?)
                        """,
                        (json.dumps(context),),
                    )
                    connection.execute(
                        """
                        INSERT INTO context_sources(
                            id, snapshot_id, source_type, source_id, ordinal,
                            reason, estimated_tokens, excerpt
                        ) VALUES (
                            'source-1', 'context-1', 'message', 'user-1', 0,
                            'current_user_turn', 1, 'Hola'
                        )
                        """
                    )

                persisted = connection.execute(
                    """
                    SELECT m.role, m.status, p.content_text
                    FROM messages m
                    LEFT JOIN message_parts p
                      ON p.message_id = m.id AND p.ordinal = 0
                    WHERE m.conversation_id = 'conv-1'
                    ORDER BY m.sequence_no
                    """
                ).fetchall()
                snapshot = connection.execute(
                    """
                    SELECT strategy_version, final_context_json
                    FROM context_snapshots WHERE broker_task_id = 'task-1'
                    """
                ).fetchone()
                source = connection.execute(
                    """
                    SELECT source_id, reason, excerpt
                    FROM context_sources WHERE snapshot_id = 'context-1'
                    """
                ).fetchone()

                self.assertEqual(
                    [("user", "complete", "Hola"), ("assistant", "pending", None)],
                    persisted,
                )
                self.assertEqual("window-v1", snapshot[0])
                self.assertEqual(context, json.loads(snapshot[1]))
                self.assertEqual(
                    ("user-1", "current_user_turn", "Hola"),
                    source,
                )

                with self.assertRaises(sqlite3.IntegrityError):
                    with connection:
                        connection.execute(
                            """
                            INSERT INTO messages(
                                id, conversation_id, role, status, sequence_no
                            ) VALUES ('user-rollback', 'conv-1', 'user', 'complete', 3)
                            """
                        )
                        connection.execute(
                            """
                            INSERT INTO messages(
                                id, conversation_id, role, status, sequence_no
                            ) VALUES ('assistant-conflict', 'conv-1', 'assistant', 'pending', 2)
                            """
                        )

                rolled_back = connection.execute(
                    "SELECT COUNT(*) FROM messages WHERE id = 'user-rollback'"
                ).fetchone()[0]
                self.assertEqual(0, rolled_back)

    def test_terminal_result_materializes_once_and_survives_reopen(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "chatygpt.db"
            with closing(self.connect(path)) as connection:
                connection.executescript(MIGRATION.read_text(encoding="utf-8"))
                connection.execute(
                    "INSERT INTO conversations(id, title) VALUES ('conv-1', 'Prueba')"
                )
                connection.execute(
                    """
                    INSERT INTO messages(
                        id, conversation_id, role, status, sequence_no
                    ) VALUES ('user-1', 'conv-1', 'user', 'complete', 1)
                    """
                )
                connection.execute(
                    """
                    INSERT INTO messages(
                        id, conversation_id, role, status, sequence_no
                    ) VALUES ('assistant-1', 'conv-1', 'assistant', 'pending', 2)
                    """
                )
                connection.execute(
                    """
                    INSERT INTO broker_tasks(
                        id, remote_task_id, conversation_id, request_message_id,
                        response_message_id, idempotency_key, request_json,
                        remote_status, local_state
                    ) VALUES (
                        'task-1', 'remote-1', 'conv-1', 'user-1',
                        'assistant-1', 'turn-key-1', '{}',
                        'generating', 'polling'
                    )
                    """
                )
                connection.execute(
                    "UPDATE messages SET broker_task_id = 'task-1' WHERE id = 'assistant-1'"
                )

                result_json = json.dumps({"result_markdown": "Respuesta durable"})
                for _ in range(2):
                    with connection:
                        connection.execute(
                            """
                            UPDATE broker_tasks
                            SET remote_status = 'completed',
                                local_state = 'terminal',
                                result_json = ?,
                                terminal_at = datetime('now')
                            WHERE id = 'task-1'
                            """,
                            (result_json,),
                        )
                        connection.execute(
                            """
                            UPDATE messages SET status = 'complete'
                            WHERE id = 'assistant-1'
                            """
                        )
                        connection.execute(
                            """
                            INSERT INTO message_parts(
                                id, message_id, ordinal, kind, content_text
                            ) VALUES (
                                lower(hex(randomblob(16))), 'assistant-1', 0,
                                'markdown', 'Respuesta durable'
                            )
                            ON CONFLICT(message_id, ordinal) DO UPDATE SET
                                kind = excluded.kind,
                                content_text = excluded.content_text
                            """
                        )
                connection.commit()

            with closing(self.connect(path)) as reopened:
                assistant = reopened.execute(
                    """
                    SELECT m.status, p.kind, p.content_text
                    FROM messages m
                    JOIN message_parts p
                      ON p.message_id = m.id AND p.ordinal = 0
                    WHERE m.id = 'assistant-1'
                    """
                ).fetchone()
                part_count = reopened.execute(
                    "SELECT COUNT(*) FROM message_parts WHERE message_id = 'assistant-1'"
                ).fetchone()[0]
                task = reopened.execute(
                    """
                    SELECT remote_task_id, remote_status, local_state, result_json
                    FROM broker_tasks WHERE id = 'task-1'
                    """
                ).fetchone()
                integrity = reopened.execute("PRAGMA integrity_check").fetchone()[0]

            self.assertEqual(
                ("complete", "markdown", "Respuesta durable"),
                assistant,
            )
            self.assertEqual(1, part_count)
            self.assertEqual(("remote-1", "completed", "terminal"), task[:3])
            self.assertEqual(
                {"result_markdown": "Respuesta durable"},
                json.loads(task[3]),
            )
            self.assertEqual("ok", integrity)


class ContractFixtureTests(unittest.TestCase):
    def test_smoke_fixture_is_local_only_and_idempotent(self) -> None:
        payload = json.loads(CONTRACT.read_text(encoding="utf-8"))
        self.assertTrue(payload["idempotency_key"])
        self.assertEqual("single", payload["execution"]["strategy"])
        self.assertEqual("local_only", payload["risk"]["data_classification"])
        self.assertFalse(payload["model_requirements"]["cloud_allowed"])
        self.assertEqual(0, payload["model_requirements"]["max_cost_usd"])

    def test_no_secret_values_are_declared_in_persisted_settings(self) -> None:
        migration = MIGRATION.read_text(encoding="utf-8").lower()
        self.assertNotIn("api_key", migration)
        self.assertNotIn("admin_token", migration)
        self.assertIn("check (sensitivity = 'public')", migration)


class BuildConfigurationTests(unittest.TestCase):
    def test_windows_icon_required_by_tauri_is_a_real_ico(self) -> None:
        content = WINDOWS_ICON.read_bytes()
        self.assertGreater(len(content), 1_024)
        self.assertEqual(b"\x00\x00\x01\x00", content[:4])

    def test_node_typescript_config_does_not_emit_import_extensions(self) -> None:
        config = json.loads(NODE_TSCONFIG.read_text(encoding="utf-8"))
        compiler = config["compilerOptions"]
        self.assertTrue(compiler["allowImportingTsExtensions"])
        self.assertTrue(compiler["noEmit"])


if __name__ == "__main__":
    unittest.main()
