# Entrance Runtime Contract

Run all commands from `entrance-src/`.

## CLI

```bash
cargo run -p entrance-app --bin entrance -- status
cargo run -p entrance-app --bin entrance -- drawer summary
cargo run -p entrance-app --bin entrance -- hive summary
cargo run -p entrance-app --bin entrance -- hive loop create --title "Local loop" --goal "Run the Hive loop MVP" --runtime codex
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime codex
cargo run -p entrance-app --bin entrance -- hive loop run 1 --runtime local --decision reject
cargo run -p entrance-app --bin entrance -- hive issue list
cargo run -p entrance-app --bin entrance -- hive issue decide 1 request-review --body "Need human call"
cargo run -p entrance-app --bin entrance -- launcher list
```

`hive loop run` returns the local compiler trace for the round: policy rows,
versioned typed packet envelopes, admission receipts, evidence, and verdict.
Admission gate failures are recorded as rejected receipts and returned as
blocked verdicts/issues instead of escaping as raw CLI errors.
The MVP runtime set is `local` and `codex`; unsupported runtime names are
reported as blocked verdicts. The `codex` runtime uses a read-only
`codex exec` worker and records stdout, stderr, and last-message transcript
data inside execution evidence.
For evaluator-path testing, `hive loop run` accepts
`--decision keep|reject|needs-review|blocked`.
Human decisions are available through `hive issue decide <id>
<retry|request-review|cancel>` and are recorded as operator comments while
also moving the linked loop contract state.

## Daemon

```bash
cargo run -p entrance-app --bin entrance -- daemon
cargo run -p entrance-app --bin entrance -- daemon stdio
cargo run -p entrance-app --bin entrance -- daemon http
```

The stdio daemon accepts one JSON invoke request per line and returns one JSON
response per line. The HTTP daemon exposes:

- `GET /health`
- `POST /invoke`

## Config And Data

- Default app root: `~/.entrance`
- Override: `ENTRANCE_APP_ROOT`
- Config: `~/.entrance/entrance.toml`
- Database: `~/.entrance/data/entrance.db`
- Vault key: `~/.entrance/vault.key`
- Drawer filesystem root: configured by `[drawer].root`

Invalid config must fail startup instead of silently defaulting.
