# Entrance Local Roadmap

Last updated: 2026-06-09

## 本轮 Stop Point

- Entrance 的当前方向明确为“透明可观察的自动推进系统”：一个 kernel 管 issue/status/comment/ledger/policy，一组 agent 以 `Developer + Reviewer` 为核心推进，Reviewer 无效且 3 轮预算耗尽时 fallback 到 `Blocked` issue。
- 本地 MVP 已有 `Explorer -> Developer -> Reviewer` 串行 loop、SQLite ledger、typed packet/admission/evidence/verdict、issue/comment 控制面、MCP stdio surface、Panel issue board、Review Queue、worker lifecycle observability。
- 本轮新增 runtime preflight 一等观察面：CLI `hive loop preflight <id>`、daemon `hive_loop_runtime_preflight`、MCP `entrance://loops/{loop_id}/runtime-preflight`、MCP issue control packet summary、Panel selected issue 的 Runtime Preflight block。
- 本轮继续新增 loop dashboard 最小切片：CLI `hive loop dashboard <id>`、daemon `hive_loop_dashboard`、MCP `entrance://loops/{loop_id}/dashboard`、Panel selected issue 的 Loop Dashboard block，用一份 contract 汇总 issue state、kernel gate、Developer/Reviewer worker lane、Reviewer budget、human decision actions、health 和 next actions。

## 还没做完

- 把 `runtime_preflight.v1` 扩展成完整 capability preview：sandbox scope、connector readiness、artifact/evidence manifest、人类偏好边界，而不仅是 runtime support/probe。
- 把 Loop Dashboard 从最小摘要推进成完整 dashboard：round timeline、packet/admission/evidence/verdict grouping、retry lineage、blocker decision surface。
- Productize MCP：真实客户端配置、协议兼容测试、verified actor identity、权限边界、远程 connector 绑定。
- Productize Linear/GitHub connector：真实 token 验证、状态映射、幂等 comment/readback、漂移恢复、rate-limit/retry 策略。
- Hardening workers：sandbox、环境脱敏、heartbeat、resume/cancel/replacement、timeout recovery、跨进程 durable failure attribution。
- Reviewer gates 继续加强：目标漂移检测、score vector 计算、keep/reject/block 证据要求，以及需要人类偏好时的选项生成。
- 正式 compiler IR：从 archive 中提升为 current truth，并把 loop contract、packet、receipt、evidence、verdict、policy registry lifecycle 变成版本化 runtime 对象。

## 下一轮建议

优先做 Loop Dashboard 的 drilldown 切片：按 round 展示 packet/admission/evidence/verdict grouping 和 retry lineage，让“哪一轮为什么失败、哪份证据支撑下一步”一屏可见。
