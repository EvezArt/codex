# EVEZ Core

**Owner:** EVEZ666 (Steven Vearl Crawford‑Maggard)  
**Intent:** Build so the maker is legible in the made—tools that remember their origin  
**Provenance:** Created by and through the Owner; the agent is the instrument

## Installation

```bash
pip install -e .
```

```bash
pipx install -e .
```

## Quick Start

```bash
evez init
```

```bash
evez add --content "First observation" --tags "origin,imprint" --location "workbench"
```

```bash
evez list --limit 10
```

## Commands

- `evez init` → Initialize database and schema.
- `evez about` → Print Owner imprint, version, and database path.
- `evez imprint` → Print full provenance statement.
- `evez add` → Create a new observation.
- `evez list` → List observations.
- `evez update` → Update an observation.
- `evez delete` → Delete an observation (requires confirmation or `--force`).

## Configuration

Database path resolution order:

1. `--db` / `--database` CLI flag
2. `EVEZ_DB_PATH` environment variable
3. Default `./evez.sqlite3`

## License

MIT License. See [LICENSE](LICENSE).
