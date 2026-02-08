"""SQLite schema definition and migrations."""

from __future__ import annotations

from typing import Iterable

SCHEMA_VERSION = 1


def schema_statements() -> Iterable[str]:
    return [
        """
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        )
        """.strip(),
        """
        CREATE TABLE IF NOT EXISTS observations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,
            content TEXT NOT NULL,
            tags TEXT,
            location TEXT
        )
        """.strip(),
        "CREATE INDEX IF NOT EXISTS idx_observations_timestamp ON observations(timestamp)",
        "CREATE INDEX IF NOT EXISTS idx_observations_location ON observations(location)",
    ]


def migration_statements(from_version: int, to_version: int) -> Iterable[str]:
    if from_version == 0 and to_version == 1:
        return schema_statements()
    raise ValueError(f"Unsupported migration path {from_version} -> {to_version}")
