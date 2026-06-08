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

## 还没做完

- 把 `runtime_preflight.v1` 扩展成完整 capability preview：sandbox scope、connector readiness、artifact capture、人类偏好边界，而不仅是 runtime support/probe。
- 把 Evidence Drilldown/Manifest 产品化：完整 transcript 展开、真实远端 receipt 归档、真实 artifact manifest 生成/内容校验、payload schema diff、更完整的 blocker decision workflow。
- Productize MCP：真实客户端配置、协议兼容测试、verified actor identity、权限边界、远程 connector 绑定。
- Productize Linear/GitHub connector：真实 token 验证、状态映射、幂等 comment/readback、漂移恢复、rate-limit/retry 策略。
- Productize issue timeline：筛选/折叠、远端 issue comment 映射、inline decision 的操作后刷新状态、receipt drilldown 和更强的 blocked action provenance。
- Productize issue transition policy：当前已经有 kernel registry/report snapshot/audit 绑定；还缺版本迁移、状态机测试、Panel 操作后刷新、远端 issue 状态映射和更完整的 policy lifecycle。
- Hardening workers：sandbox、环境脱敏、heartbeat、resume/cancel/replacement、timeout recovery、跨进程 durable failure attribution。
- Reviewer gates 继续加强：目标漂移检测、score vector 计算、keep/reject/block 证据要求，以及需要人类偏好时的选项生成。
- 正式 compiler IR：从 archive 中提升为 current truth，并把 loop contract、packet、receipt、evidence、verdict、policy registry lifecycle 变成版本化 runtime 对象。

## 下一轮建议

优先把 issue transition policy 补到状态机测试和远端状态映射，同时把 issue timeline inline decision、Evidence Drilldown/Manifest 接到真实 agent/connector 产物：操作后刷新、receipt drilldown、真实 artifact manifest 生成与内容校验、完整 transcript 展开、blocker decision workflow。
