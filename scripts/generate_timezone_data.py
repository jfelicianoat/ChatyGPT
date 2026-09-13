"""Generate the compact IANA transition table embedded by the desktop app.

The input is Tcl's generated tzdata directory (for example the copy bundled
with Git for Windows). IANA timezone data is in the public domain.
"""

from __future__ import annotations

import argparse
import re
import struct
from pathlib import Path


ROW = re.compile(r"\{(-?\d+)\s+(-?\d+)\s+[01]\s+[^}]+\}")
ALIAS = re.compile(r"set TZData\(:[^)]+\) \$TZData\(:([^)]+)\)")


def load_zones(root: Path) -> dict[str, tuple[tuple[int, int], ...]]:
    direct: dict[str, tuple[tuple[int, int], ...]] = {}
    aliases: dict[str, str] = {}
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        name = path.relative_to(root).as_posix()
        text = path.read_text(encoding="utf-8")
        rows = tuple((int(at), int(offset)) for at, offset in ROW.findall(text))
        if rows:
            direct[name] = rows
            continue
        alias = ALIAS.search(text)
        if alias:
            aliases[name] = alias.group(1)

    def resolve(name: str, seen: frozenset[str] = frozenset()) -> tuple[tuple[int, int], ...]:
        if name in direct:
            return direct[name]
        if name in seen or name not in aliases:
            raise ValueError(f"unresolved timezone alias: {name}")
        rows = resolve(aliases[name], seen | {name})
        direct[name] = rows
        return rows

    for name in aliases:
        resolve(name)
    return direct


def write_database(zones: dict[str, tuple[tuple[int, int], ...]], output: Path) -> None:
    tables = sorted(set(zones.values()))
    table_ids = {table: index for index, table in enumerate(tables)}
    payload = bytearray(b"CTZ1")
    payload += struct.pack("<I", len(tables))
    for table in tables:
        payload += struct.pack("<I", len(table))
        for timestamp, offset in table:
            payload += struct.pack("<qi", timestamp, offset)
    payload += struct.pack("<I", len(zones))
    for name, table in sorted(zones.items()):
        encoded = name.encode("utf-8")
        payload += struct.pack("<H", len(encoded)) + encoded
        payload += struct.pack("<I", table_ids[table])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(payload)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    write_database(load_zones(args.source), args.output)


if __name__ == "__main__":
    main()
