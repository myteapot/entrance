# Bootstrap Mapping

> Status: active old-to-new path map for `MYT-64`

## Old To New

| Old path | New path |
| --- | --- |
| `A:/.agents/duet/SKILL.md` | `harness/bootstrap/duet/SKILL.md` |
| `A:/.agents/duet/roles/arch.md` | `harness/bootstrap/duet/roles/arch.md` |
| `A:/.agents/duet/roles/dev.md` | `harness/bootstrap/duet/roles/dev.md` |
| `A:/.agents/duet/roles/agent.md` | `harness/bootstrap/duet/roles/agent.md` |
| `A:/.agents/nota/identity.md` | `harness/bootstrap/nota/identity.md` |
| `A:/.agents/nota/rules.md` | `harness/bootstrap/nota/rules.md` |

## Transition Rule

- `A:/.agents` source files remain untouched
- these copied files are now the canonical repo-side bootstrap copies
- runtime cutover is a later step and is not implied by this mapping alone

## Explicit Non-Imports

- `A:/.agents/nota/todo.md`
- `A:/.agents/nota/data/store.db`
- `A:/.agents/nota/data/store.json`
- `A:/.agents/nota/scripts/control.py`
- `A:/.agents/nota/scripts/db.py`
- `A:/.agents/.worktrees/...`
