# 状态清单（State Inventory）· TextCraft v5.0 基线

> **范围裁定后的读法（2026-11-10）**：基座状态以**宿主状态**为准——Flecs 世界（`EcsWorld`）、PRNG 状态、宿主计数与配置（第 2 节中所有者为宿主的条目及第 4 节）。凡涉及肉鸽 `*world-state*`（chunks/players/monsters/timers 等）的条目，自脚本迁移起属于**示例模型 `models/roguelike/` 的模型状态，非基座状态**：它们随该模型包的 `engine:export-state/import-state` 进出快照的 `world_state` 字段，基座只提供该字段的通道与校验，不理解其 schema。快照 v2 的 checkpoint 由 Rust 宿主实现（`SchemeLogic::create_checkpoint/rollback_checkpoint` 与同名 MCP 工具），不再由模型脚本承担。

> 本文件是《TextCraft 基座改造指南》第 1 项的交付物：逐项登记基座的全部状态——所有者、物理位置、写入口、是否进快照及理由。
> **规则：今后新增任何状态，必须先登记进这张表；快照格式（v2）评审以此表为全集。**
> 状态分类：**因果状态**（不同则未来演化不同，必须进快照）／**派生状态**（可由因果状态完整重建，不进快照）／**非世界状态**（传输层、Agent 记忆、配置等，不进快照并注明理由）。

## 1. 模拟时钟（tick）——权威裁定

现状有三份 tick 计数，关系如下：

| 计数 | 位置 | 语义 |
|---|---|---|
| `tick_count` | `*world-state*`（`models/roguelike/prelude.scm` 定义，`models/roguelike/main.scm` 的 `engine:on-tick` 每步 +1） | 模型自有计数；驱动 chemistry/怪物 AI 的 modulo-30 调度、导出与断言，是模型语义的一部分 |
| `server_tick` | 宿主 `game_server` 的 `ServerHandle`（`src/runtime.rs` 三处 `increment_server_tick()` / `server_tick()`） | 宿主主循环计数；其值作为 `tick-id` 参数传给 `engine:on-tick` |
| `*current-tick*` | `models/roguelike/prelude.scm` 全局 | `engine:on-tick` 开头由 `tick-id` 赋值（`models/roguelike/main.scm`），即 `server_tick` 的镜像；被按键时长计算与日志时间戳读取，因此**间接影响演化** |

**裁定（本清单起生效，第 2 项落地）**：`*world-state*` 的 `tick_count` 是唯一的权威模拟时钟。快照只存 `tick_count`；恢复后宿主传给 `engine:on-tick` 的 `tick-id` 必须由 `tick_count` 推导（恢复值 + 步进），不得再用独立递增的 `server_tick`。`server_tick` 降级为传输层计数（快照发送去重/心跳），不进快照——它对世界演化的唯一通道就是 `tick-id` 参数，切断该通道后即与演化无关。`*current-tick*` 是镜像，不进快照，恢复后由宿主在下一次 `on-tick` 时重设。

## 2. 因果状态（必须进快照 v2）

| # | 状态 | 所有者 | 物理位置 | 写入口 | 快照字段 |
|---|---|---|---|---|---|
| 1 | `*world-state*` 全部顶层键：`tick_count`、`elapsed_sec`、`target_fps`、`idle_timer`、`chunks`、`players`、`saved_players`、`monsters`、`timers` | 模型 | `models/roguelike/prelude.scm` | 脚本经 `hash-insert` 等更新；宿主经 `engine:export-state/import-state` 读写 | `world_state` |
| 2 | Flecs 全部实体：`GridPos`、`VisualGlyph`、`PlayerSession`、`DynamicProps` 四个组件 | 宿主（世界容器）＋模型（内容） | `src/ecs/world.rs` 的 `EcsWorld.world` | `ecs:*` FFI（`src/logic/ecs_ffi.rs`）、`EcsWorld` 方法 | `ecs_world.entities[]` |
| 3 | 实体标签：`Monster`、`Player`、`Item`、`Terrain`、`Dead`、`Liquid`、`Burning` | 同上 | `src/ecs/components.rs`，Flecs tag | `ecs:*` FFI | `ecs_world.entities[].tags` |
| 4 | 实体关系：`PartOf`、`EquippedOn`、`CoveredWith`（关系对） | 同上 | `src/ecs/components.rs`，Flecs relationship | `ecs:*` FFI（`relate!` 等） | `ecs_world.entities[].relations` |
| 5 | PRNG 内部状态 | 宿主 | `src/logic/mod.rs` 的 `SchemeLogic.rng`（现状 `Option<StdRng>`，第 3 项换自持算法） | `engine:raw-random-int`、`engine:random-int`、`(random:seed!)`、`set_random_seed` | `prng` |
| 6 | 权威模拟时钟 `tick_count` | 模型 | 同第 1 项中的键（单列以便裁定，见第 1 节） | `engine:on-tick` | `tick`（与 `world_state.tick_count` 同值，顶层单列供宿主使用） |

注：第 2–4 项的单实体序列化形状复用 `EcsWorld::inspect_entity` 的现有输出（实际键名 `position`/`glyph{char,fg,bg,solid,opaque}`/`dynamic_props`/`player_session`/`tags`/`relationships[{relation,target_id,target_name}]`），不另发明格式。

**实体 id 保持（第 2 项落地）**：`flecs_ecs` 绑定提供 `World::make_alive`，快照导入先用它逐个预留原 id、再建动态标签/关系实体，实测 id 可原样保持（往返测试见 `src/ecs/world.rs` 单元测试与 `tests/snapshot_v2_test.rs`），故不采用 id_map 重写方案；`import_world_json` 仍返回 id_map 供校验，出现非恒等映射时宿主打警告日志。

**实体引用字段登记**：经核查，`*world-state*` 内**不保存** Flecs 实体 id——玩家/怪物以字符串 id（`"goblin_1"`）＋坐标与 ECS 关联，anatomy 的肢体 id 只在单次调用的返回值里传递、不落盘。因此 id 重映射时 *world-state* 无字段需要重写。`DynamicProps` 的值由模型自定，若某模型把 eid 存进 props，须先在本表登记该键名，基座再据此重写——当前模型无此用法。另注意快照导出的“游戏实体”判据（本轮 A 组任务 1 已修订）：携带任何组件、标签、关系或名字的游戏实体都进快照（纯标签/纯关系实体可往返），flecs 基础设施实体（定义实体、flecs 命名空间）排除；未知 Rust 组件的值不序列化、仅在 unknown_components 字段列名，详见 docs/project/known_limitations.md (b)。

## 3. 派生状态（不进快照，必须可由因果状态重建）

| 状态 | 物理位置 | 重建方式 |
|---|---|---|
| `SpatialGrid` 空间索引 | `src/ecs/spatial.rs`，`EcsWorld.spatial` | 导入快照后由全部实体的 `GridPos`＋`VisualGlyph` 全量重建 |
| `ViewportRasterizer.screen_buffer` | `src/ecs/rasterizer.rs` | 每帧由世界状态重新光栅化，无跨 tick 内容 |
| `*visual-version*` | `models/roguelike/prelude.scm` | 表现层失效计数器；恢复后递增一次即完成失效 |
| `*current-tick*` | `models/roguelike/prelude.scm` | 镜像，见第 1 节裁定 |
| 宿主 `last_snapshots`（玩家快照去重表） | `src/runtime.rs` 的 `TextCraftServer` | 传输层去重缓存；恢复后清空即可，最坏多发一帧全量快照 |

## 4. 非世界状态（不进快照）

| 状态 | 物理位置 | 不进快照的理由 |
|---|---|---|
| MCP 黑板 `mcp_storage` | `src/types.rs` 的 `McpStorage`（宿主与 `SchemeLogic` 各持一份 `Arc` 引用，同一存储） | Agent 的记忆，不是世界状态；恢复快照不得回滚它。注意：现有 v1 checkpoint 借它作存储介质，不改变其性质 |
| MCP 动态工具注册表 `*mcp-tools-meta*` / `*mcp-tools-handlers*` | `models/roguelike/prelude.scm` | 属于模型 θ（脚本装载时注册），不属于运行状态；换模型靠重装脚本，不靠快照 |
| `*mutation-registry*` | `models/roguelike/mutation.scm` | 静态定义数据（变异定义表），装载期由脚本填充，属 θ |
| `*chunk-size*`、`*esc*`、`*ansi-reset*`、各 `LAYER-*` 常量 | `models/roguelike/prelude.scm` | 常量，属 θ |
| 玩家网络连接、`player_tokens` | `src/runtime.rs` | 传输层/鉴权；但连接对应的 `PlayerSession` 组件属 Flecs 状态，随第 2 项进快照 |
| `outbound_queue` 待发送消息队列 | `src/logic/mod.rs` 的 `SchemeLogic` | 瞬时量；快照只允许在 tick 边界拍摄，此时队列应已排空 |
| `registered_hooks`、`extra_version` | `src/logic/mod.rs` 的 `SchemeLogic` | 由当前装载的脚本推导的缓存，装载时重建 |
| 脚本文件监视器 `watcher`、热重载检查时刻 `last_reload_check` | `src/logic/mod.rs`、`src/runtime.rs` | 开发工具状态，与世界演化无关 |
| 配置：`time_scale`、`autosave_sec`、`snapshot_heartbeat_ms`、`log_level`、`is_paused`、`admin_token` | `src/runtime.rs` 的 `TextCraftServer` | 启动配置；其中 `is_paused` 是控制面状态而非世界状态，epoch 协议另行管理 |
| 存档文件路径 `save_file` | `src/logic/mod.rs` | 配置；存档文件内容本身是快照 v2 的一种落盘形态（第 2 项统一格式） |

## 5. 评审问答（验收用）

- 随便指一份状态，它在快照的哪个字段？——因果状态查第 2 节表；查不到即未登记，先补表再谈实现。
- 恢复快照后哪些东西被允许和恢复前不同？——只有第 3、4 节列出的派生/非世界状态。
- 相同输入下回滚重演与不间断运行结果是否应一致？——第 2 节全部字段一致即应一致（状态哈希相等，第 4 项落地后以哈希为判据）。
