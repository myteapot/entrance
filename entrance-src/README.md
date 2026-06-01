# Entrance

**Local control plane for coding automation.**

*One Rust binary for durable notes, task ledgers, local secrets, launchers, and a desktop bridge.*

> Entrance keeps project-side state close to the machine:
> persistent notes, a small task ledger, local AES-GCM vault records, app indexing, and a GUI bridge over one runtime.

当前版本是 **V2 Microkernel Preview**：一个 `entrance` 程序提供 CLI 和后台 daemon；桌面端使用 Electron + SolidJS，并通过同一个 daemon 协议调用 Rust runtime。

---

## 一图看懂 / Architecture

![Entrance Architecture](./docs/entrance_architecture.png)

---

## Runtime Surfaces

### Drawer

Durable local storage for notes, imported files, vault records, and version snapshots.

```powershell
.\entrance.exe drawer memory import --title "登录页重构进度" --body "auth middleware 已修，下一步补集成测试"
.\entrance.exe drawer list
.\entrance.exe drawer history
```

```powershell
.\entrance.exe drawer vault store --title "OpenAI" --secret "sk-..."
.\entrance.exe drawer vault list
```

### Hive

Task ledger for dispatch records, engine reports, callbacks, and review state.

```powershell
.\entrance.exe hive dispatch --title "修复登录页 500 错误"
.\entrance.exe hive summary
.\entrance.exe hive engine 1
.\entrance.exe hive review 1 approve
```

Local agent-loop MVP:

```powershell
.\entrance.exe hive loop create --title "README loop" --goal "Run a constrained agent loop" --runtime codex --compact
.\entrance.exe hive loop run 1 --runtime codex
.\entrance.exe hive loop run 1 --runtime codex --compact
.\entrance.exe hive loop run 1 --runtime local --decision needs-review
.\entrance.exe hive loop show 1
.\entrance.exe hive loop trace 1
.\entrance.exe hive loop evidence 1
.\entrance.exe hive loop audit 1
.\entrance.exe hive loop doctor 1
.\entrance.exe hive loop policies 1
.\entrance.exe hive policy registry
.\entrance.exe hive connector registry --compact
.\entrance.exe hive issue list
.\entrance.exe hive issue show 1
.\entrance.exe hive issue connector-admission 1 --compact
.\entrance.exe hive issue comment 1 --body "Reviewed from the local panel"
```

`hive loop run` records a minimal compiler path in SQLite: active policies,
versioned typed packets, receipt-aware admission gates, versioned admission
receipts, stage evidence, and the versioned final verdict.
Add `--compact` to `hive loop create` to print the linked issue card and next
actions instead of the full empty loop report. Add `--compact` to `hive loop run`
to print the Doctor summary after execution instead of the full packet/evidence
transcript-heavy report. Add `--compact` to `hive issue run`,
`hive issue retry-run`, `hive issue show`,
`hive issue comment`, or `hive issue decide` to print the compact issue card
with recent comments, evidence, stages, and next actions.
Pending Doctor next actions prefer the issue-first compact command
`hive issue run <id> --runtime <runtime> --compact` when a loop has a linked
issue.
`hive policy registry` exposes the typed gate registry plus runtime worker
policy for supported runtimes, sandbox mode, timeout bounds, attempt bounds,
required worker receipt fields, and role binding. `hive loop policies <id>`
shows the active policy rows loaded into a specific loop contract.
`hive loop trace <id>` returns the compact round-aware health view, including
the evaluator score vector and current-round worker duration/timeout totals,
without packet transcripts.
`hive loop evidence <id>` returns the compact evidence ledger with stage role,
admission result, worker receipt, packet envelope diagnostics, missing receipts,
operator options, and short transcript excerpts.
`hive loop audit <id>` returns a compiler-style audit over the loop contract,
active policies, runtime policy, stage sequence, stage evidence, typed packets,
packet sequence, admission receipts, worker receipts, verdict packets, and
linked issue surface. The active policy check verifies the canonical
Explorer/Doer/Evaluator route and gate contract. The stage sequence check
rejects duplicate role stages in a loop round and verifies terminal loops still
have the expected current-round stages. The stage evidence check verifies each
expected stage has exactly one stage-bound evidence row with the expected kind.
The packet sequence check rejects duplicate route packets in a loop round. The
worker and runtime policy checks verify that worker receipts carry a role and
that the role still matches the packet writer. The admission check
verifies that every packet has exactly one admission and that the recorded
packet, policy, gate spec, receipt requirements, missing receipts, gate result,
and final admission result still bind to each other. The verdict check verifies
one verdict per round for terminal loops plus decision bindings, score-vector
metrics, gate booleans, human options, reason-code evidence bindings, evaluator
worker bindings, evidence counts, runtime readiness, and admission-rejection
evidence/admission/packet links. The
issue surface check verifies issue status, typed comments, operator
comment/decision evidence, and the author/action/body bindings between evidence
and its linked comment. Runtime policy checks the
current round so a successful retry can replace a previously blocked runtime
attempt.
`hive loop doctor <id>` is the first CLI stop after a run: it combines trace and
audit state into one health summary with counts, failed checks, missing
receipts, worker failures, specific audit failure details, and suggested next
commands.
`hive issue comment <id> --body <text>` records a local issue comment and, when
the issue is bound to a loop, mirrors it into the loop ledger as
`operator_comment` evidence.
Compact issue surfaces include connector mirror status: `hive issue show
<id> --compact` exposes the issue's `connector` block, and `hive issue list
--compact` also returns a `connector_queue` for publish-required mirrors.
`hive connector registry --compact` exposes active/planned issue-surface
providers and the connector admission gate, while `hive issue
connector-admission <id> --compact` dry-runs whether the current mirror can be
routed to `external_issue_surface`.
Supported MVP runtimes are `local` and `codex`; `codex` runs a read-only
`codex exec` worker for each `Explorer`, `Doer`, and `Evaluator` role and
stores the worker transcript plus explicit receipt, timeout, and exit status in
stage evidence, along with runtime duration for each worker and aggregated
round duration in trace/doctor/card views. Codex workers must
return an `{ "ok": true }` JSON receipt to be admitted.
Unknown runtimes return a blocked verdict instead of being silently kept.
The evaluator decision can be overridden for local simulation with
`--decision keep|reject|needs-review|blocked`.
Admission gates reject failed worker receipts, so a role worker with `ok=false`
blocks at the compiler boundary instead of being treated as valid evidence.
`hive loop run` is idempotent for non-`todo` contracts; use
`hive issue retry-run <id>` to record a retry decision and immediately run the
linked loop. `hive issue run <id>` runs a `Todo` issue without requiring the
operator to look up its loop id.
Issue cards expose round-aware trace chips so a retry shows the new current
round separately from the loop's accumulated history. They also expose an
operator trail derived from typed `operator_comment` and `operator_decision`
evidence so the latest human comment, retry, review, or cancel action is visible
without opening raw evidence. Audit failures include both the failed check and
the extracted detail code, such as an evidence-to-comment binding error.

### Launcher

Local application index and launch surface.

```powershell
.\entrance.exe launcher refresh
.\entrance.exe launcher search code
.\entrance.exe launcher list
```

---

## 快速开始 / Quick Start

### 当前推荐：从源码试用

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo build --workspace --release

.\target\release\entrance.exe status
```

### CLI smoke

```powershell
.\target\release\entrance.exe drawer add-note --title "Plan" --body "Ship README"
.\target\release\entrance.exe hive dispatch --title "Refactor pass"
.\target\release\entrance.exe launcher refresh
.\target\release\entrance.exe daemon http
```

### 启动桌面端

```powershell
pnpm dev:electron
```

---

## Plugin Status

| Surface | Responsibility | Status |
|---|---|---|
| **Drawer** | Durable storage: notes, imports, vault, snapshots | ✅ |
| **Hive** | Task ledger: dispatch, engine reports, callbacks, review | ✅ |
| **Launcher** | Local app index and launch surface | ✅ |

---

## 技术栈 / Tech Stack

Rust · Electron · SolidJS · SQLite · TOML

---

## 当前阶段 / Status

**V2 Microkernel Preview** — CLI、daemon bridge 和 Electron GUI 共用同一套 Rust runtime。当前没有独立 MCP server；外部集成应先走 daemon stdio/http invoke 协议。

---

## 许可 / License

[Business Source License 1.1](./LICENSE) · [详情 LICENSES.md](./LICENSES.md) · [商标 TRADEMARKS.md](./TRADEMARKS.md)
