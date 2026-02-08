"""Database initialization command."""

from __future__ import annotations

from pathlib import Path

from ..db import initialize_db


def run_init(db_path: Path) -> None:
    version = initialize_db(db_path)
    print(f"Initialized database at {db_path} (schema v{version}).")
