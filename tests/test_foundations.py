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


if __name__ == "__main__":
    unittest.main()
