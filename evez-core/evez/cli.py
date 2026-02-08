"""CLI interface for EVEZ Core."""

from __future__ import annotations

import argparse
import logging
import sys
from typing import Sequence

from .db import resolve_db_path
from .commands.about import run_about, run_imprint
from .commands.init import run_init
from .commands.observations import run_add, run_delete, run_list, run_update

LOGGER = logging.getLogger(__name__)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="evez", description="EVEZ Core CLI")
    parser.add_argument(
        "--db",
        "--database",
        dest="database",
        help="Path to the SQLite database",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="count",
        default=0,
        help="Increase logging verbosity (use -vv for debug)",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("about", help="Show Owner imprint and database info")
    subparsers.add_parser("imprint", help="Show full provenance statement")

    init_parser = subparsers.add_parser("init", help="Initialize database schema")
    init_parser.set_defaults(command="init")

    add_parser = subparsers.add_parser("add", help="Add a new observation")
    add_parser.add_argument("--content", required=True, help="Observation content")
    add_parser.add_argument("--tags", help="Comma-separated tags")
    add_parser.add_argument("--location", help="Location string")
    add_parser.add_argument(
        "--timestamp",
        type=int,
        help="Unix timestamp (defaults to now)",
    )

    list_parser = subparsers.add_parser("list", help="List observations")
    list_parser.add_argument("--limit", type=int, default=20)
    list_parser.add_argument("--offset", type=int, default=0)

    update_parser = subparsers.add_parser("update", help="Update an observation")
    update_parser.add_argument("id", type=int, help="Observation ID")
    update_parser.add_argument("--content")
    update_parser.add_argument("--tags")
    update_parser.add_argument("--location")
    update_parser.add_argument("--timestamp", type=int)

    delete_parser = subparsers.add_parser("delete", help="Delete an observation")
    delete_parser.add_argument("id", type=int, help="Observation ID")
    delete_parser.add_argument(
        "--force",
        action="store_true",
        help="Delete without confirmation",
    )

    return parser


def configure_logging(verbosity: int) -> None:
    if verbosity >= 2:
        level = logging.DEBUG
    elif verbosity == 1:
        level = logging.INFO
    else:
        level = logging.WARNING
    logging.basicConfig(level=level, format="%(levelname)s: %(message)s")


def main(argv: Sequence[str] | None = None) -> None:
    parser = build_parser()
    args = parser.parse_args(argv)
    configure_logging(args.verbose)
    db_path = resolve_db_path(args.database)

    try:
        if args.command == "about":
            run_about(db_path)
        elif args.command == "imprint":
            run_imprint()
        elif args.command == "init":
            run_init(db_path)
        elif args.command == "add":
            run_add(db_path, args.content, args.tags, args.location, args.timestamp)
        elif args.command == "list":
            run_list(db_path, args.limit, args.offset)
        elif args.command == "update":
            run_update(
                db_path,
                args.id,
                args.content,
                args.tags,
                args.location,
                args.timestamp,
            )
        elif args.command == "delete":
            run_delete(db_path, args.id, args.force)
        else:
            parser.error(f"Unknown command {args.command}")
    except ValueError as exc:
        LOGGER.error("%s", exc)
        sys.exit(1)
    except Exception as exc:  # pragma: no cover - safeguard
        LOGGER.exception("Unexpected error: %s", exc)
        sys.exit(1)
