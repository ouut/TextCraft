# 已知结构性残留（Known Limitations）· TextCraft 基座（feat/simulator-upgrade）

> 本文件登记基座改造（第 1–8 项）完成后**仍然存在**的结构性限制：不是待办清单里的普通 bug，而是当前架构下靠约定、靠环境或靠上游版本维持的边界。每条按「现状 / 影响 / 触发条件」记录，处理任何一条前先读对应条目与引用文档。
>
> 口径来源（2026-11-10 核对）：`docs/project/state_inventory.md`（状态全集与快照 v2 字段）、`docs/project/baseline_v5_test_results.md`（改造后全量 130 个测试、稳定通过 129）、`docs/develop/scheme_developer_guide.md` 第 7 节（核心原语 vs 网格原语清单）、`Cargo.toml` / `Cargo.lock`（flecs_ecs 锁定版本）与 `src/ecs/world.rs`、`src/runtime.rs` 源码。

---

## (a) 双存储并集与同步约定：`*world-state*` ＋ Flecs 世界 —— 【已解决 · 第二轮改造（2026-11-10）】

**解决形态**
Flecs 世界已成为唯一权威存储。宿主在 `SchemeLogic::new` 时急切创建单例实体 `WorldState`（Flecs 首个 spawn，id 固定 518），原 `*world-state*` 的全部因果状态（地图、玩家表、`tick_count` 等）以顶层键为单位存入该实体的 `DynamicProps`（JSON 键值）；roguelike 的地图按 chunk 分键（`chunk:<cx>,<cy>`）避免整图深转换，旧单键 `chunks` 由 prelude 首触时无损拆分迁移。Scheme 侧只保留兼容门面：宿主原语 `state:get` / `state:set!` / `state:has?` / `state:remove!` / `state:all` / `state:replace!` / `state:seed-defaults!` / `state:keys` / `state:entity-id`（`src/logic/ecs_ffi.rs`），仓库内模型（roguelike、counter_economy）已全部迁移到门面，代码中对 `*world-state*` 的直接读写为零（仅剩注释提及）；直接 `(set! *world-state* ...)` 的旧写法就此废弃，新模型不得再用。`tick_count` 的权威随状态迁入单例（ECS 侧），快照顶层 `tick` 从单例读，`server_tick` 仍只是传输层计数。快照升 **v3**：顶层只含 Flecs 世界＋PRNG＋时钟，不再有独立的 `world_state` 段，checkpoint / `run_epoch` / `state_hash` 全走新路径；v2 旧存档可兼容读取（导入时把其 `world_state` 段一次灌入单例），写出永远是 v3。验收见 `tests/snapshot_v3_test.rs`（仅经门面改状态→快照→清空整个 ECS→恢复→哈希一致，且断言顶层无 `world_state` 段）。

**遗留子项（未随本条关闭）**
① `state:get` 读取大值（整个 chunk）仍有 JSON 克隆＋Scheme↔JSON 转换成本：roguelike 的 tick30 基准约 38ms（旧基线约 2ms），在 500ms 看门狗内但余量变薄；若后续要再压性能，需把 tiles 改为实体级存储，属下一轮议题。② Scheme 的 char/symbol 值经门面往返会降为 string（与既有 `ecs:set` 同口径）；现已核查两个模型的状态值无 char/symbol/闭包，无实际漂移点，新模型往单例里存这类值前需先意识到这一点。

---

## (b) 通用性实证只有一个，且它没用 ECS —— 【已解决 · 第二轮改造（2026-11-10）】

**解决形态**
新增真正使用 ECS 的非网格实证模型 `tests/fixtures/models/market_sim/`（市场模拟：账户/商户/库存/订单实体，Balance/Inventory 类数据全经 `ecs:set`/`ecs:get` 写读 `DynamicProps`，`Owns` / `TradesWith` / `PlacedBy` / `FilledBy` / `PartOf` 关系走通用原语 `ecs:relate`，另有 Market/Account/Merchant/Stock/Order 等标签；全程不引用 `*world-state*`、不用 `GridPos`/`SpatialGrid`/任何网格算子）。`tests/market_sim_test.rs` 三个测试背书：① `run_epoch` 固定种子＋输入序列，终局余额/库存/till/成交数由宿主断言精确校验且重跑报告逐字节一致；② 快照→回滚→续跑后 `state_hash` 与导出状态与第一程完全一致，恢复后关系对与标签经 `ecs:target`/`ecs:children`/`ecs:has-tag?` 与快照 JSON 双路断言完整；③ 非网格把关：测试对模型源码 grep 指南 7.2 网格 token 清单（含 `world-state`）断言零出现、反向断言 `ecs:*` 核心原语确有调用，并断言快照中每个实体 `position` 为 null。此前「不用 ECS 的非网格模型」（counter_economy）与「用 ECS 的非网格模型」（market_sim）两条路径至此都有端到端实证。

**遗留子项（未随本条关闭）**
① 导出只对已知组件（GridPos/VisualGlyph/DynamicProps/PlayerSession）序列化**值**：实体上未知的 Rust 组件只在 `unknown_components` 字段列出名字、值不进快照（不静默丢失，但不完整）；模型引入宿主未知组件类型前须先给导出加序列化支持。② 动态标签/关系定义实体若已被移除全部引用，与同名 spawn 实体结构上不可区分，可能被当作命名实体导出。③ **回滚后新 spawn 实体 id 不续号**：快照导入用 `make_alive` 保持已有实体 id，但 Flecs 分配器不从 checkpoint 时的计数续号，回滚后新生成的实体会拿到更大的 id；而 `state_hash` 覆盖 ecs_world 原始 id，这类「回滚后继续 spawn」的模型续跑哈希会与不间断运行发散（因果状态本身逐值一致）。market_sim 的应对是订单实体改为 init 时生成的有界槽位池、交易复用改写；若后续模型需要「回滚后 spawn 仍哈希一致」，须在基座层处理（如哈希口径排除纯分配器 id、或导入时回退分配器计数），属待决问题。

---

## (c) live 模式墙钟 dt：确定性只覆盖无头路径

**现状**
实时服务循环（`src/runtime.rs` 的 `run_loop`）的 dt 取真实流逝时间：`let dt = elapsed.as_secs_f32() * self.time_scale;`，其中 `elapsed` 是两帧之间 `Instant` 的差，随后经 `tick(dt)` → `engine:on-tick dt tick-id` 传给模型。模型会把这个浮点 dt 累加进状态——如示例模型把 dt 累加到 WorldState 单例里的 `elapsed_sec` 与 `idle_timer`（均为快照内的因果状态）。相对地，确定性路径不取墙钟：`run_epoch` 的 dt 来自参数（缺省 0.016），`simulate_steps` 以调用方给定的固定 dt 无睡眠步进。因此现有的种子化 PRNG、状态哈希与 epoch 可复现保证，口径上只覆盖 `run_epoch` / 无头步进路径，不覆盖 live 模式。

**影响**
live 模式下同一局游戏无法按「种子＋输入序列」复现：帧率抖动、机器负载、暂停时长都会改变 dt，进而改变 `elapsed_sec` 等浮点状态与依赖它们的演化（乃至快照哈希）。live 过程中拍摄的快照恢复后继续跑，也不会与不间断运行收敛到同一状态——这不是 bug，是当前 dt 来源决定的。

**触发条件**
在 live（实时服务）模式下运行并期望复现、回放、跨机比对状态哈希，或拿 live 中段的快照做确定性评测。需要确定性结论时应改走 `run_epoch` 或 `simulate_steps` 并固定 dt。

---

## (d) 性能测试红灯与时序测试抖动

**现状**
基线口径（`docs/project/baseline_v5_test_results.md`，改造后结果）：全量 130 个测试稳定通过 129 个。唯一稳定失败是 `phase5_scale_benchmark::test_10000_entities_scale_and_rasterizer_benchmark` 的光栅化性能预算断言（预算 200μs/帧，基线环境慢速共享 VM 实测 697–827μs），属环境问题、非功能回归。此外 `phase5_verification_test` 与 `snapshot_dedup_test` 的多人时序断言在全量高负载下偶发失败（失败集合逐次不同），单独重跑均全过（基线记录 verification 9/9、dedup 4/4）。

**影响**
全量 `cargo test` 在慢速机器上永远有一盏红灯，容易掩盖真正的性能回归，也容易让人养成「红灯不用看」的习惯；时序抖动则会产生与改动无关的偶发红灯。CI 已做区分处理（`.github/workflows/test.yml`）：`phase5_scale_benchmark` 单独一步、`continue-on-error: true` 只记录不挡门禁，其余套件（含时序套件）仍在必过集合内——所以时序偶发红灯在 CI 上仍可能出现，处置方式是先单独重跑该套件确认，而不是直接放行或改断言。

**触发条件**
在慢速/高负载的 runner 或共享 VM 上跑全量测试；或光栅化路径有真实性能变化时（此时性能套件的红灯与环境红灯长得一样，必须在本机或专用基准上单独核对数值，不能默认归因环境）。

---

## (e) flecs_ecs alpha 绑定风险：靠锁 rev 控制

**现状**
ECS 绑定 `flecs_ecs` 仍是 Alpha 阶段的 Rust 绑定（`Cargo.lock` 中版本 0.2.2），其反射与序列化等能力尚属 WIP，因此基座的快照导出/导入没有用 Flecs 自带的反射/序列化，而是宿主在 `src/ecs/world.rs` 里**手工按已知组件序列化**：`export_world_json` 复用 `inspect_entity` 的输出形状逐实体导出，`import_world_json` 逐字段手工解析 `position` / `glyph` / `dynamic_props` / `player_session` / 标签 / 关系，并用一份硬编码名单（`is_structural_tag`）过滤 Flecs 内部与结构性标签名。当前风险靠锁版本控制：`Cargo.toml` 将 flecs_ecs 锁定在 git rev `bf956db99b69d41ee142c9d52054911af28d7e93`，`Cargo.lock` 已提交，CI 统一用 `cargo test --locked`。

**影响**
升级 flecs_ecs 时，绑定 API、组件/标签的内部命名、实体 id 语义（当前保 id 依赖 `World::make_alive`）都可能变化，而快照层对这些细节是硬编码耦合的：升级即使编译通过，快照往返也可能静默丢标签、丢关系或错位组件。锁 rev 本身也意味着上游修复不会自动流入，每次升级都是一次需要显式评估的事件。

**触发条件**
任何 flecs_ecs 升级（改 `Cargo.toml` 的 rev 或更新 `Cargo.lock` 中该包）。升级时必须同步复查：`export_world_json` / `import_world_json` / `inspect_entity` 的字段形状、`is_structural_tag` 名单、`make_alive` 保 id 行为，并以 `tests/snapshot_v2_test.rs` 与 ECS 相关套件（`ecs_ffi_test` 等）全过为最低门槛，快照 v2 的存量存档兼容性另行人工核对。
