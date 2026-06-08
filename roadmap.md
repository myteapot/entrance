# Entrance Local Roadmap

Last updated: 2026-06-09

## 本轮 Stop Point

- Entrance 的当前方向明确为“透明可观察的自动推进系统”：一个 kernel 管 issue/status/comment/ledger/policy，一组 agent 以 `Developer + Reviewer` 为核心推进，Reviewer 无效且 3 轮预算耗尽时 fallback 到 `Blocked` issue。
- 本地 MVP 已有 `Explorer -> Developer -> Reviewer` 串行 loop、SQLite ledger、typed packet/admission/evidence/verdict、issue/comment 控制面、MCP stdio surface、Panel issue board、Review Queue、worker lifecycle observability。
- 本轮新增 runtime preflight 一等观察面：CLI `hive loop preflight <id>`、daemon `hive_loop_runtime_preflight`、MCP `entrance://loops/{loop_id}/runtime-preflight`、MCP issue control packet summary、Panel selected issue 的 Runtime Preflight block。
- 本轮继续新增 loop dashboard 最小切片：CLI `hive loop dashboard <id>`、daemon `hive_loop_dashboard`、MCP `entrance://loops/{loop_id}/dashboard`、Panel selected issue 的 Loop Dashboard block，用一份 contract 汇总 issue state、kernel gate、Developer/Reviewer worker lane、Reviewer budget、human decision actions、health 和 next actions。
- 本轮继续把 Loop Dashboard 推进到 round drilldown：每轮展示 packet/admission/evidence/verdict grouping、retry lineage、blocker、worker/receipt counts。
- 本轮继续新增 evidence drilldown 最小切片：CLI `hive loop evidence-drilldown <id>`、daemon `hive_loop_evidence_drilldown`、MCP `entrance://loops/{loop_id}/evidence-drilldown`、Panel selected issue 的 Evidence Drilldown block，展示 worker receipt、transcript/payload excerpt、remote receipt 摘要、artifact/path hint、payload key diff、blocker 和 blocker-bound decision surface。
- 本轮继续把 Evidence Drilldown blocker 绑定到 issue action contract：evidence-level blocker 和 Reviewer budget fallback 的 loop-level blocker 都会暴露 primary action、retry/review/cancel/comment command、confirmation policy 和 review queue/policy resource。
- 本轮继续新增 evidence manifest 最小切片：CLI `hive loop evidence-manifest <id>`、daemon `hive_loop_evidence_manifest`、MCP `entrance://loops/{loop_id}/evidence-manifest`、Panel selected issue 的 Evidence Manifest block，展示 payload/receipt/transcript/artifact entries、digest coverage、path verification state 和 next actions。
- 本轮继续把 issue activity timeline 推进成 issue-first 控制面：`hive issue timeline <id>` / `hive issue timeline-item <id> <item-id>` / `hive_issue_timeline` / `entrance://issues/{issue_id}/timeline` / `entrance://issues/{issue_id}/timeline/items/{item_id}` / Panel Activity Timeline 现在会暴露 round groups、item permalink、Blocked/Needs Review human decision surface、primary action、retry/review/cancel/comment command、operator confirmation receipt provenance、confirmation policy 和 issue-control/review-queue resource。
- 本轮继续新增 issue transition policy 最小切片：CLI `hive issue transition-policy <id>`、daemon `hive_issue_transition_policy`、MCP `entrance://issues/{issue_id}/transition-policy`、MCP issue control packet resource pointer、Panel selected issue 的 Transition Policy block，用一份 `entrance.hive.issue_transition_policy.v1` 汇总当前 state class、allowed/blocked actions、confirmation receipt contract、Reviewer fallback budget、policy owner/scope 和 linked resources。
- 本轮继续把 issue transition policy 绑定到 kernel policy registry：`hive policy registry --compact` 现在暴露 `issue_transitions` registry，`issue_transition_policy.v1` report 嵌入 registry snapshot，`hive loop audit` 增加 `issue_transition_policy` check 来校验 allowed/blocked action coverage、confirmation contract 和 Reviewer fallback budget。
- 本轮继续把 issue/status/comment 执行路径绑定到 transition admission：`hive issue comment/decide/run/retry-run` 会先通过 kernel transition policy，operator comment/decision payload 写入 `entrance.hive.issue_transition_admission.v1` receipt，CLI retry/review/cancel 需要 `--human-confirmed`，issue surface audit 会校验 transition admission 与 evidence/comment 绑定。
- 本轮继续补齐 Panel 操作后刷新：Panel 写 issue ledger 的操作，包括 create/run/retry/review/cancel/comment、issue mirror sync/publish/verify/readback/admit/roundtrip、connector publish/roundtrip execute 和 fixture demo，都会刷新 board 并强制重新读取 selected issue 的 Transition Policy、Loop Dashboard、Evidence Drilldown、Evidence Manifest、Activity Timeline、Runtime Preflight 和 Worker Lifecycle。
- 本轮继续把 issue transition policy 推进成可验证状态机：`issue_transitions.state_machine` 现在随 `hive policy registry --compact` / MCP policy registry 暴露每个状态的 allowed/blocked action、gate、confirmation、terminal/human-decision class，并补了状态矩阵测试来校验真实 issue action surface 与 registry 不漂移，包括 loop-bound `run` 和 retryable runtime-rejected `Canceled` 条件。
- 本轮继续把远端 issue 状态映射推进到 policy registry：`hive policy registry --compact` 现在暴露 remote-fixture/GitHub/Linear status mapping，GitHub write/readback 使用 issue state/state_reason，Linear write/readback 先用 state name 或 description status marker 做受限校验，remote write plan 和 readback detail 都会携带同一份 `status_mapping`。
- 本轮继续把 Linear status mapping 推进到配置驱动写入：`entrance.toml` 支持 `connectors.linear.status_mappings.<HiveStatus>.remote_state_id`，provider registry/remote contract/connector queue 会暴露 configured mapping，Linear GraphQL update 会写入 configured `stateId`，readback 会优先校验 `state.id` 再 fallback 到 state name/status marker。
- 本轮继续把 issue 级 connector control 暴露到 agent 和 Panel：新增 `entrance.hive.issue_connector_control.v1` 摘要，MCP `entrance_issue_control` / `entrance://issues/{issue_id}/control` 现在携带 provider、publish/admission gate、remote target、remote write plan、当前 `status_mapping` 和 configured mappings；Panel connector strip 也显示当前 issue 的 status mapping chip。

## 还没做完

- 把 `runtime_preflight.v1` 扩展成完整 capability preview：sandbox scope、connector readiness、artifact capture、人类偏好边界，而不仅是 runtime support/probe。
- 把 Evidence Drilldown/Manifest 产品化：完整 transcript 展开、真实远端 receipt 归档、真实 artifact manifest 生成/内容校验、payload schema diff、更完整的 blocker decision workflow。
- Productize MCP：真实客户端配置、协议兼容测试、verified actor identity、权限边界、远程 connector 绑定。
- Productize Linear/GitHub connector：真实 token 验证、Linear workflow discovery/migration、幂等 comment/readback、漂移恢复、rate-limit/retry 策略。
- Productize issue timeline：筛选/折叠、远端 issue comment 映射、inline decision 的操作后刷新状态、receipt drilldown 和更强的 blocked action provenance。
- Productize issue transition policy：当前已经有 kernel registry/report snapshot/audit 绑定、execution-time transition admission receipt、Panel 操作后 selected issue control surface 刷新、系统化状态机矩阵测试、provider status mapping policy 和 Linear configured stateId mapping；还缺版本迁移、状态映射 discovery/migration 和更完整的 policy lifecycle。
- Hardening workers：sandbox、环境脱敏、heartbeat、resume/cancel/replacement、timeout recovery、跨进程 durable failure attribution。
- Reviewer gates 继续加强：目标漂移检测、score vector 计算、keep/reject/block 证据要求，以及需要人类偏好时的选项生成。
- 正式 compiler IR：从 archive 中提升为 current truth，并把 loop contract、packet、receipt、evidence、verdict、policy registry lifecycle 变成版本化 runtime 对象。

## 下一轮建议

优先把 connector status mapping 补到 live token-backed validation 和 drift recovery，同时把 issue timeline inline decision、Evidence Drilldown/Manifest 接到真实 agent/connector 产物：receipt drilldown、真实 artifact manifest 生成与内容校验、完整 transcript 展开、blocker decision workflow。
