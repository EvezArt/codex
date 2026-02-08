"""Database helpers and migration logic."""

from __future__ import annotations

import logging
import os
import sqlite3
from pathlib import Path
from typing import Iterable

from .schema import SCHEMA_VERSION, migration_statements

LOGGER = logging.getLogger(__name__)


def resolve_db_path(cli_value: str | None) -> Path:
    if cli_value:
        return Path(cli_value).expanduser()
    env_value = os.getenv("EVEZ_DB_PATH")
    if env_value:
        return Path(env_value).expanduser()
    return Path.cwd() / "evez.sqlite3"


def connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(db_path)
    connection.execute("PRAGMA foreign_keys = ON")
    return connection


def _ensure_schema_table(connection: sqlite3.Connection) -> None:
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)"
    )


def get_schema_version(connection: sqlite3.Connection) -> int:
    _ensure_schema_table(connection)
    row = connection.execute(
        "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1"
    ).fetchone()
    if row is None:
        return 0
    return int(row[0])


def set_schema_version(connection: sqlite3.Connection, version: int) -> None:
    connection.execute("DELETE FROM schema_version")
    connection.execute("INSERT INTO schema_version (version) VALUES (?)", (version,))


def apply_statements(connection: sqlite3.Connection, statements: Iterable[str]) -> None:
    for statement in statements:
        LOGGER.debug("Executing SQL: %s", statement)
        connection.execute(statement)


def migrate(connection: sqlite3.Connection, from_version: int, to_version: int) -> None:
    current = from_version
    while current < to_version:
        next_version = current + 1
        apply_statements(connection, migration_statements(current, next_version))
        current = next_version
        set_schema_version(connection, current)


def initialize_db(db_path: Path) -> int:
    with connect(db_path) as connection:
        current_version = get_schema_version(connection)
        if current_version == 0:
            LOGGER.info("Initializing database at schema version %s", SCHEMA_VERSION)
        if current_version > SCHEMA_VERSION:
            raise RuntimeError(
                f"Database version {current_version} is newer than supported {SCHEMA_VERSION}"
            )
        if current_version < SCHEMA_VERSION:
            migrate(connection, current_version, SCHEMA_VERSION)
        return SCHEMA_VERSION
