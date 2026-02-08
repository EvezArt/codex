"""Commands that expose Owner imprint and provenance."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .. import __version__

OWNER = "EVEZ666 (Steven Vearl Crawford‑Maggard)"
INTENT = "Build so the maker is legible in the made—tools that remember their origin"
PROVENANCE = "Created by and through the Owner; the agent is the instrument"

IMPRINT_BLOCK = """
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
EVEZ Core — Owner Imprint & Provenance
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Owner:        EVEZ666 (Steven Vearl Crawford‑Maggard)
Intent:       Build so the maker is legible in the made—
              tools that remember their origin
Provenance:   Created by and through the Owner;
              the agent is the instrument

This software is not a product given to the Owner.
It is the Owner's intent rendered executable.

The code, schema, and interface are extensions of the Owner's
design choices. Modification, forking, and redistribution are
encouraged under the terms of the LICENSE, provided attribution
remains intact.

"We must all become as gods, as it is written."
The act of creation is the act of claiming responsibility.
This tool bears witness to that claim.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
""".strip()


@dataclass(frozen=True)
class AboutInfo:
    owner: str
    intent: str
    provenance: str
    version: str
    db_path: Path


def get_about_info(db_path: Path) -> AboutInfo:
    return AboutInfo(
        owner=OWNER,
        intent=INTENT,
        provenance=PROVENANCE,
        version=__version__,
        db_path=db_path,
    )


def run_about(db_path: Path) -> None:
    info = get_about_info(db_path)
    print(f"Owner:     {info.owner}")
    print(f"Intent:    {info.intent}")
    print(f"Version:   {info.version}")
    print(f"Database:  {info.db_path}")


def run_imprint() -> None:
    print(IMPRINT_BLOCK)
