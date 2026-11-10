# v5.0 基线测试结果（改造前，Muse 云 VM 环境）

- 环境：Ubuntu 24.04，共享 CPU（约 1.5 核），`cargo test --no-fail-fast`，2026-11-10。
- 总计 **105 个测试：102 通过，3 失败**。3 个失败均为环境敏感型，改造验收以「不新增失败」为准：

| 失败测试 | 原因 |
|---|---|
| `phase5_scale_benchmark::test_10000_entities_scale_and_rasterizer_benchmark` | 性能预算断言：本机光栅化 697–827 μs/帧 ≥ 200 μs 预算，慢速共享 VM 所致 |
| `phase5_verification_test::test_task_5_1_multi_user_url_token_routing_and_offline_ghost_filtering` | 多人快照时序断言（"Alice should receive initial snapshot"），慢速 VM 时序敏感 |
| `phase5_verification_test::test_task_5_1_admin_disconnect_grace_period_and_reconnect` | 断线重连时序断言，慢速 VM 时序敏感 |

- 其余套件全绿：lib 单元 46、advanced_mcp 6、ecs_ffi 6、graceful_hooks 4、headless 4、integration 10、mcp_storage 6、phase4 4、rasterizer_benchmark 1、server_gateway 2、snapshot_dedup 4。

---

## 改造后结果（feat/simulator-upgrade，第 2–8 项 + 范围裁定迁移后，2026-11-10）

- 全量 `cargo test --no-fail-fast` 共 **130 个测试**（基线 105 + 新增 25：lib 内 snapshot/prng/world 单元 8、snapshot_v2 3、prng 3、epoch 4、epoch_fixture 2、eval_mode 2、manifest 3）。
- **稳定通过 129 个**。唯一稳定失败仍是基线已有的 `phase5_scale_benchmark::test_10000_entities_scale_and_rasterizer_benchmark`（光栅化性能预算，慢速共享 VM），与基线一致，非新增回归。
- `phase5_verification_test` 与 `snapshot_dedup_test` 的时序断言在本机全量负载下偶发失败（失败集合逐次不同，含基线在册的 grace-period 用例），单独重跑均全过（verification 9/9、dedup 4/4），属基线已注明的环境敏感类。
- 测试变更清单（无套件被移除）：
  - 全部套件的模型路径 `scripts/` → `models/roguelike/`（脚本迁移，仅路径调整，phase4/phase5 玩法套件保留并通过）；
  - `mcp_storage_test` 的 checkpoint 存储形状断言迁至快照 v2 包装（`world_state` 嵌套 + format/version 断言），回滚行为断言不变；
  - `integration_tests::test_8_state_persistence_shutdown_and_restart` 的存档文件形状断言迁至 v2（同上），重启恢复断言不变；
  - `integration_tests` 两处 `ServerArgs` 字面构造补 `eval: false`（第 5 项新字段，dev 默认）。

---

> **编者注（2026-11-10 文档刷新时追加，不改动上文任何数字）**：本文件是改造前基线与第 2–8 项改造后结果的**历史快照**，上文「改造后结果」的 130/129 不是当前数字。其后还有两轮：协同修复轮后全量 137 个、稳定通过 134；第二轮（双存储消灭＋market_sim）后全量 144 个、稳定通过 143，唯一稳定失败始终为 `phase5_scale_benchmark` 的光栅化性能预算。现状口径以 `docs/project/known_limitations.md` (d) 为准。
