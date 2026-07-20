from __future__ import annotations

import argparse
import json
import os
import sqlite3
import sys
import urllib.error
import urllib.request
from datetime import datetime
from pathlib import Path


def broker_health(base_url: str, endpoint: str) -> str:
    url = f"{base_url.rstrip('/')}/{endpoint.lstrip('/')}"
    try:
        with urllib.request.urlopen(url, timeout=3) as response:
            body = response.read(2048).decode("utf-8", errors="replace")
            return f"HTTP {response.status}: {body}"
    except urllib.error.HTTPError as error:
        body = error.read(2048).decode("utf-8", errors="replace")
        return f"HTTP {error.code}: {body}"
    except Exception as error:  # noqa: BLE001 - diagnostic boundary
        return f"NO ACCESIBLE: {error}"


def compact_error(raw: str | None) -> str:
    if not raw:
        return "(sin detalle persistido)"
    try:
        value = json.loads(raw)
        return json.dumps(value, ensure_ascii=False, indent=2)
    except json.JSONDecodeError:
        return raw


def build_report(database_path: Path, broker_url: str) -> str:
    lines = [
        "Diagnostico de adjuntos de ChatyGPT",
        f"Fecha: {datetime.now().astimezone().isoformat(timespec='seconds')}",
        f"Broker: {broker_url}",
        f"health/live: {broker_health(broker_url, '/health/live')}",
        f"health/ready: {broker_health(broker_url, '/health/ready')}",
        "",
    ]
    if not database_path.exists():
        lines.append(f"ERROR: no se encontro la base de ChatyGPT en {database_path.parent}")
        return "\n".join(lines)

    connection = sqlite3.connect(database_path, timeout=5)
    connection.row_factory = sqlite3.Row
    try:
        version = connection.execute("PRAGMA user_version").fetchone()[0]
        lines.append(f"Esquema SQLite: {version}")
        rows = connection.execute(
            """
            SELECT display_name, media_type, size_bytes, broker_file_id,
                   ingestion_status, ingestion_error_json, created_at, updated_at
            FROM attachments
            ORDER BY updated_at DESC
            LIMIT 10
            """
        ).fetchall()
    finally:
        connection.close()

    lines.append(f"Adjuntos recientes: {len(rows)}")
    for index, row in enumerate(rows, start=1):
        lines.extend(
            [
                "",
                f"[{index}] {row['display_name']}",
                f"  Tipo: {row['media_type'] or '(desconocido)'}",
                f"  Tamano: {row['size_bytes']} bytes",
                f"  Estado: {row['ingestion_status']}",
                f"  Broker file_id: {row['broker_file_id'] or '(todavia no asignado)'}",
                f"  Creado: {row['created_at']}",
                f"  Actualizado: {row['updated_at']}",
                "  Error:",
                *[f"    {line}" for line in compact_error(row['ingestion_error_json']).splitlines()],
            ]
        )
    if not rows:
        lines.append("No hay adjuntos persistidos.")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    local_app_data = os.environ.get("LOCALAPPDATA")
    if not local_app_data:
        print("LOCALAPPDATA no esta definido.", file=sys.stderr)
        return 1
    database_path = Path(local_app_data) / "es.jfeliciano.chatygpt" / "chatygpt.db"
    broker_url = os.environ.get(
        "CHATYGPT_BROKER_BASE_URL", "http://192.168.1.52:8765"
    )
    try:
        report = build_report(database_path, broker_url)
        args.output.write_text(report, encoding="utf-8")
    except Exception as error:  # noqa: BLE001 - diagnostic boundary
        print(f"No se pudo crear el diagnostico: {error}", file=sys.stderr)
        return 1
    print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
