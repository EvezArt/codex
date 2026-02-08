"""CRUD commands for observations."""

from __future__ import annotations

import time
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from ..db import connect, initialize_db


@dataclass(frozen=True)
class Observation:
    id: int
    timestamp: int
    content: str
    tags: str | None
    location: str | None


def _row_to_observation(row: tuple) -> Observation:
    return Observation(
        id=int(row[0]),
        timestamp=int(row[1]),
        content=str(row[2]),
        tags=row[3],
        location=row[4],
    )


def run_add(
    db_path: Path,
    content: str,
    tags: str | None,
    location: str | None,
    timestamp: int | None,
) -> None:
    initialize_db(db_path)
    observation_timestamp = int(timestamp if timestamp is not None else time.time())
    with connect(db_path) as connection:
        cursor = connection.execute(
            """
            INSERT INTO observations (timestamp, content, tags, location)
            VALUES (?, ?, ?, ?)
            """.strip(),
            (observation_timestamp, content, tags, location),
        )
        observation_id = cursor.lastrowid
    print(f"Added observation {observation_id} at {observation_timestamp}.")


def run_list(db_path: Path, limit: int, offset: int) -> None:
    initialize_db(db_path)
    with connect(db_path) as connection:
        rows = connection.execute(
            """
            SELECT id, timestamp, content, tags, location
            FROM observations
            ORDER BY timestamp DESC, id DESC
            LIMIT ? OFFSET ?
            """.strip(),
            (limit, offset),
        ).fetchall()
    observations = [_row_to_observation(row) for row in rows]
    if not observations:
        print("No observations found.")
        return
    for observation in observations:
        print(
            f"{observation.id} | {observation.timestamp} | {observation.content}"
            f" | tags={observation.tags or '-'} | location={observation.location or '-'}"
        )


def _build_update_pairs(
    content: str | None, tags: str | None, location: str | None, timestamp: int | None
) -> Iterable[tuple[str, object]]:
    if content is not None:
        yield ("content", content)
    if tags is not None:
        yield ("tags", tags)
    if location is not None:
        yield ("location", location)
    if timestamp is not None:
        yield ("timestamp", int(timestamp))


def run_update(
    db_path: Path,
    observation_id: int,
    content: str | None,
    tags: str | None,
    location: str | None,
    timestamp: int | None,
) -> None:
    initialize_db(db_path)
    updates = list(_build_update_pairs(content, tags, location, timestamp))
    if not updates:
        raise ValueError("No fields provided for update.")
    assignments = ", ".join([f"{column} = ?" for column, _ in updates])
    values = [value for _, value in updates]
    values.append(observation_id)
    with connect(db_path) as connection:
        cursor = connection.execute(
            f"UPDATE observations SET {assignments} WHERE id = ?",
            tuple(values),
        )
        if cursor.rowcount == 0:
            raise ValueError(f"Observation {observation_id} not found.")
    print(f"Updated observation {observation_id}.")


def run_delete(db_path: Path, observation_id: int, force: bool) -> None:
    initialize_db(db_path)
    if not force:
        confirmation = input(
            f"Delete observation {observation_id}? Type 'yes' to confirm: "
        ).strip()
        if confirmation.lower() != "yes":
            print("Delete cancelled.")
            return
    with connect(db_path) as connection:
        cursor = connection.execute(
            "DELETE FROM observations WHERE id = ?", (observation_id,)
        )
        if cursor.rowcount == 0:
            raise ValueError(f"Observation {observation_id} not found.")
    print(f"Deleted observation {observation_id}.")
