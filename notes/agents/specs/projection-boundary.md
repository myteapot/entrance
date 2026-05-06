# Projection Boundary

> Scope: define what may write truth, what may only project truth, and how projection failure is represented

## Planes

- `Truth`
  DB-backed canonical semantic and runtime ownership
- `Cold Docs`
  durable knowledge objects canonicalized in DB and optionally exported to files
- `Hot Root`
  six-file retained projection for current operator continuity
- `Oracle`
  minimal route projection for human entry

## Write order

1. validate the write against runtime law
2. write canonical truth first
3. record projection work, freshness, or dirty debt
4. project to files, GUI, CLI, or MCP

## Projection classes

- `oracle_projection`
  route-only projection for human entry
- `hot_root_projection`
  six-file operator projection
- `cold_doc_projection`
  exported documentation projection
- `ui_projection`
  GUI / CLI / MCP read surface derived from truth

## Boundary law

- projections may not outrank truth
- hand-edited projected files are not automatically canonicalized
- import is an explicit ingress, not a side effect of touching files
- projection failure must leave a dirty or repairable truth trace
- replay from DB must be sufficient to rebuild retained projections

## Freshness law

- each projection target should know its source truth revision
- each projection target should know whether it is fresh or dirty
- each projection target should know which repair path can refresh it

## Failure law

- `truth write succeeded, projection failed` means dirty projection rather than failed truth
- `truth write failed` means no downstream projection should claim success
- `projection skipped by policy` is different from `projection stale` and different again from `projection failed`
