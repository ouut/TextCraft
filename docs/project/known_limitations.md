# 已知结构性残留（Known Limitations）· TextCraft 基座（feat/simulator-upgrade）

> 本文件登记基座改造（第 1–8 项）完成后**仍然存在**的结构性限制：不是待办清单里的普通 bug，而是当前架构下靠约定、靠环境或靠上游版本维持的边界。每条按「现状 / 影响 / 触发条件」记录，处理任何一条前先读对应条目与引用文档。
>
> 口径来源（2026-11-10 核对）：`docs/project/state_inventory.md`（状态全集与快照 v2 字段）、`docs/project/baseline_v5_test_results.md`（改造后全量 130 个测试、稳定通过 129）、`docs/develop/scheme_developer_guide.md` 第 7 节（核心原语 vs 网格原语清单）、`Cargo.toml` / `Cargo.lock`（flecs_ecs 锁定版本）与 `src/ecs/world.rs`、`src/runtime.rs` 源码。

---

## (a) 双存储并集与同步约定：`*world-state*` ＋ Flecs 世界

**现状**
同一份世界状态物理上存两处：模型侧的 Scheme 全局 `*world-state*`（经 `engine:export-state` / `engine:import-state` 进出快照的 `world_state` 字段），与宿主侧的 Flecs 世界（`src/ecs/world.rs` 的 `EcsWorld`，含 `GridPos` / `VisualGlyph` / `PlayerSession` / `DynamicProps` 四个组件、实体标签与 `PartOf` / `EquippedOn` / `CoveredWith` 关系，进快照的 `ecs_world.entities[]` 字段）。快照 v2 覆盖的是这个**并集**（外加 PRNG 状态与顶层 `tick`，见 state_inventory 第 2 节）。两处之间没有自动同步机制：单一权威靠 state_inventory 的登记规则维持——新增任何状态必须先登记所有者、物理位置与快照字段。经核查，当前示例模型的 `*world-state*` 内**不保存** Flecs 实体 id（玩家/怪物以字符串 id ＋坐标与 ECS 关联），所以快照导入用 `make_alive` 保 id 时模型侧无需重写引用；但若某模型把实体 id 存进 `DynamicProps`，必须先在 state_inventory 登记该键名，基座才会据此处理——目前无此用法。

**影响**
同一个事实若被模型同时写进两边，两份副本的更新顺序与一致性完全由模型代码自守，基座不校验、不报警。快照能忠实保存并集在拍摄时刻的样子、状态哈希也能对上，但两边语义已经分叉的世界照样能通过哈希与往返测试，问题只会在后续演化中暴露。

**触发条件**
新模型（或现有模型扩展）对同一事实双写：例如位置既存 `*world-state*` 的实体表、又用 `ecs:set-pos` 写 `GridPos`，且两条写路径不是每次都成对执行；或模型开始把 Flecs 实体 id 落进 `*world-state*` / `DynamicProps` 却未在 state_inventory 登记。

---

## (b) 通用性实证只有一个，且它没用 ECS

**现状**
「基座能跑非网格模型」目前只有一个实证：夹具模型 `tests/fixtures/models/counter_economy/`（双账户计息/转账的经济模拟），由 `tests/epoch_fixture_test.rs` 验证其在同一二进制上跑通完整 epoch 且结果确定。该夹具的 `main.scm` 明确**不调用任何 `ecs:*`**、不用 `GridPos` / 字形 / 视口，全部状态只放在它自己的 `*world-state*` 里。与此同时，网格组件并未从宿主拆出：`EcsWorld::new`（`src/ecs/world.rs`）仍无条件注册 `GridPos`、`VisualGlyph` 等组件，scheme_developer_guide 第 7 节也注明核心/网格原语的划分「只做契约/装载层分类，不改变 `src/ecs/` 的组件注册；网格组件仍在基座内注册，非网格模型不使用它们」。快照的「游戏实体」判据已在本轮放宽（A 组任务 1）：携带任何组件、标签、关系或名字的游戏实体都进快照，纯标签/纯关系实体可往返（有专门测试）；flecs 基础设施实体（组件/标签/关系定义实体、flecs 命名空间）被排除。但仍有两点残留：① 导出只对四个已知组件（GridPos/VisualGlyph/DynamicProps/PlayerSession）序列化**值**，实体上未知的 Rust 组件只在 `unknown_components` 字段列出名字、值不进快照（不再静默丢失，但也不完整）；② 动态标签/关系定义实体若已被移除全部引用，与同名 spawn 实体结构上不可区分，可能被当作命名实体导出。

**影响**
「用 ECS 的非网格模型」这条路径仍未被完整验证过：例如一个只用 `DynamicProps` ＋标签/关系建模、不用网格坐标的模型，其实体创建与 epoch 评测是否全程成立，没有端到端测试背书；若模型引入宿主未知的 Rust 组件类型，其值会因上条残留 ① 而不在快照内。当前能声称的通用性上限是「不用 ECS 的非网格模型可跑、已知组件构成的 ECS 状态可往返」，不能外推为「任意 ECS 模型可跑」。

**触发条件**
新模型选型时使用 ECS 容器原语（`ecs:spawn`、`ecs:set` 写 `DynamicProps`、标签、关系等）但不使用网格原语，或模型中出现不带任何核心组件的实体。

---

## (c) live 模式墙钟 dt：确定性只覆盖无头路径

**现状**
实时服务循环（`src/runtime.rs` 的 `run_loop`）的 dt 取真实流逝时间：`let dt = elapsed.as_secs_f32() * self.time_scale;`，其中 `elapsed` 是两帧之间 `Instant` 的差，随后经 `tick(dt)` → `engine:on-tick dt tick-id` 传给模型。模型会把这个浮点 dt 累加进状态——如示例模型把 dt 累加到 `*world-state*` 的 `elapsed_sec` 与 `idle_timer`（均为快照 `world_state` 字段内的因果状态）。相对地，确定性路径不取墙钟：`run_epoch` 的 dt 来自参数（缺省 0.016），`simulate_steps` 以调用方给定的固定 dt 无睡眠步进。因此现有的种子化 PRNG、状态哈希与 epoch 可复现保证，口径上只覆盖 `run_epoch` / 无头步进路径，不覆盖 live 模式。

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
