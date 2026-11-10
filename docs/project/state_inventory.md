# 状态清单（State Inventory）· TextCraft 基座（feat/simulator-upgrade）

> **范围裁定后的读法（2026-11-10）**：基座状态以**宿主状态**为准——Flecs 世界（`EcsWorld`）、PRNG 状态、宿主计数与配置（第 2 节中所有者为宿主的条目及第 4 节）。**第二轮改造（2026-11-10）起双存储已消灭**：原肉鸽 `*world-state*` 的全部键（chunks/players/monsters/timers 等）迁入宿主管理的单例实体 `WorldState` 的 `DynamicProps`，成为 Flecs 世界的一部分，随 `ecs_world.entities[]` 进出快照 v3；它们仍是**示例模型 `models/roguelike/` 的模型状态，非基座状态**，基座只提供单例承载与 `state:*` 门面原语，不理解其 schema。快照 v3 的 checkpoint 由 Rust 宿主实现（`SchemeLogic::create_checkpoint/rollback_checkpoint` 与同名 MCP 工具），不再由模型脚本承担。

> 本文件是《TextCraft 基座改造指南》第 1 项的交付物：逐项登记基座的全部状态——所有者、物理位置、写入口、是否进快照及理由。
> **规则：今后新增任何状态，必须先登记进这张表；快照格式（v3）评审以此表为全集。**
> 状态分类：**因果状态**（不同则未来演化不同，必须进快照）／**派生状态**（可由因果状态完整重建，不进快照）／**非世界状态**（传输层、Agent 记忆、配置等，不进快照并注明理由）。

## 1. 模拟时钟（tick）——权威裁定

现状有三份 tick 计数，关系如下：

| 计数 | 位置 | 语义 |
|---|---|---|
| `tick_count` | WorldState 单例实体的 `DynamicProps`（`models/roguelike/main.scm` 的 `engine:on-tick` 经 `state:set!` 每步 +1） | 模型自有计数；驱动 chemistry/怪物 AI 的 modulo-30 调度、导出与断言，是模型语义的一部分 |
| `server_tick` | 宿主 `game_server` 的 `ServerHandle`（`src/runtime.rs` 三处 `increment_server_tick()` / `server_tick()`） | 宿主主循环计数；其值作为 `tick-id` 参数传给 `engine:on-tick` |
| `*current-tick*` | `models/roguelike/prelude.scm` 全局 | `engine:on-tick` 开头由 `tick-id` 赋值（`models/roguelike/main.scm`），即 `server_tick` 的镜像；被按键时长计算与日志时间戳读取，因此**间接影响演化** |

**裁定（本清单起生效，第 2 项落地；第二轮改造后物理位置变更）**：`tick_count` 是唯一的权威模拟时钟，其物理位置已随双存储消灭迁入 **WorldState 单例实体（ECS 侧）**，快照顶层 `tick` 从单例读出。快照只存 `tick_count`；恢复后宿主传给 `engine:on-tick` 的 `tick-id` 必须由 `tick_count` 推导（恢复值 + 步进），不得再用独立递增的 `server_tick`。`server_tick` 降级为传输层计数（快照发送去重/心跳），不进快照——它对世界演化的唯一通道就是 `tick-id` 参数，切断该通道后即与演化无关。`*current-tick*` 是镜像，不进快照，恢复后由宿主在下一次 `on-tick` 时重设。

## 2. 因果状态（必须进快照 v3）

快照 v3 顶层形状：`{format: "textcraft-snapshot", version: 3, model: {name, state_schema, scripts_hash}, tick, prng: {algo, state}, ecs_world}`（`src/logic/mod.rs` 的 `export_snapshot`）——不再有独立的 `world_state` 段；v2 存档可兼容读取（其 `world_state` 段在导入时灌入 WorldState 单例），写出永远是 v3。

| # | 状态 | 所有者 | 物理位置 | 写入口 | 快照字段 |
|---|---|---|---|---|---|
| 1 | 原 `*world-state*` 全部顶层键：`tick_count`、`elapsed_sec`、`target_fps`、`idle_timer`、`players`、`saved_players`、`monsters`、`timers`，及按 chunk 分键的地图 `chunk:<cx>,<cy>`（旧单键 `chunks` 由 prelude 首触时无损拆分迁移） | 模型 | WorldState 单例实体（`SchemeLogic::new` 急切创建，id 固定 518）的 `DynamicProps` | 脚本经 `state:get`/`state:set!` 等门面原语（`src/logic/ecs_ffi.rs`）更新 | `ecs_world.entities[]`（单例实体条目的 `dynamic_props`） |
| 2 | Flecs 全部实体：`GridPos`、`VisualGlyph`、`PlayerSession`、`DynamicProps` 四个组件 | 宿主（世界容器）＋模型（内容） | `src/ecs/world.rs` 的 `EcsWorld.world` | `ecs:*` FFI（`src/logic/ecs_ffi.rs`）、`EcsWorld` 方法 | `ecs_world.entities[]` |
| 3 | 实体标签：`Monster`、`Player`、`Item`、`Terrain`、`Dead`、`Liquid`、`Burning` | 同上 | `src/ecs/components.rs`，Flecs tag | `ecs:*` FFI | `ecs_world.entities[].tags` |
| 4 | 实体关系：`PartOf`、`EquippedOn`、`CoveredWith`（关系对；宿主另接受任意关系名，见 market_sim 的 `Owns` 等） | 同上 | `src/ecs/components.rs`，Flecs relationship | `ecs:*` FFI（`ecs:relate`/`ecs:unrelate`，prelude 语法糖 `relate!`/`part-of` 等） | `ecs_world.entities[].relations` |
| 5 | PRNG 内部状态 | 宿主 | `src/logic/mod.rs` 的 `SchemeLogic.prng`（`Arc<Mutex<prng::Prng>>`，自持 xoshiro256++，`src/logic/prng.rs`；第 3 项落地，StdRng 与 `thread_rng` 已清零） | `engine:raw-random-int`、`engine:random-int`、`(random:seed!)`、`set_random_seed` | `prng` |
| 6 | 权威模拟时钟 `tick_count` | 模型 | 同第 1 项中的键（WorldState 单例内，单列以便裁定，见第 1 节） | `engine:on-tick` 经 `state:set!` | `tick`（从单例读出，顶层单列供宿主使用） |

注：第 2–4 项的单实体序列化形状复用 `EcsWorld::inspect_entity` 的现有输出（实际键名 `position`/`glyph{char,fg,bg,solid,opaque}`/`dynamic_props`/`player_session`/`tags`/`relationships[{relation,target_id,target_name}]`），不另发明格式。

**实体 id 保持（第 2 项落地）**：`flecs_ecs` 绑定提供 `World::make_alive`，快照导入先用它逐个预留原 id、再建动态标签/关系实体，实测 id 可原样保持（往返测试见 `src/ecs/world.rs` 单元测试与 `tests/snapshot_v2_test.rs`），故不采用 id_map 重写方案；`import_world_json` 仍返回 id_map 供校验，出现非恒等映射时宿主打警告日志。

**实体引用字段登记**：经核查，roguelike 的模型状态（现位于 WorldState 单例内）**不保存** Flecs 实体 id——玩家/怪物以字符串 id（`"goblin_1"`）＋坐标与 ECS 关联，anatomy 的肢体 id 只在单次调用的返回值里传递、不落盘。`DynamicProps` 的值由模型自定，把 eid 存进 props 须先在本表登记：现有登记一例——实证模型 `market_sim` 的订单槽位 props 存 `buyer_eid`（买家实体 id）；其 id 在快照导入时由 `make_alive` 原样保持、槽位池设计使回滚后不新分配 id，故无需重写，但回滚后新 spawn 实体 id 不续号的一般性 caveat 见 known_limitations (b) 遗留子项 ③。另注意快照导出的“游戏实体”判据（本轮 A 组任务 1 已修订）：携带任何组件、标签、关系或名字的游戏实体都进快照（纯标签/纯关系实体可往返），flecs 基础设施实体（定义实体、flecs 命名空间）排除；未知 Rust 组件的值不序列化、仅在 unknown_components 字段列名，详见 docs/project/known_limitations.md (b)。

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
| MCP 黑板 `mcp_storage` | `src/types.rs` 的 `McpStorage`（宿主与 `SchemeLogic` 各持一份 `Arc` 引用，同一存储） | Agent 的记忆，不是世界状态；恢复快照不得回滚它。注意：宿主快照 v3 的 checkpoint 借它作存储介质（键 `checkpoint:<tag>`），不改变其性质 |
| MCP 动态工具注册表 `*mcp-tools-meta*` / `*mcp-tools-handlers*` | `models/roguelike/prelude.scm` | 属于模型 θ（脚本装载时注册），不属于运行状态；换模型靠重装脚本，不靠快照 |
| `*mutation-registry*` | `models/roguelike/mutation.scm` | 静态定义数据（变异定义表），装载期由脚本填充，属 θ |
| `*chunk-size*`、`*esc*`、`*ansi-reset*`、各 `LAYER-*` 常量 | `models/roguelike/prelude.scm` | 常量，属 θ |
| 玩家网络连接、`player_tokens` | `src/runtime.rs` | 传输层/鉴权；但连接对应的 `PlayerSession` 组件属 Flecs 状态，随第 2 项进快照 |
| `outbound_queue` 待发送消息队列 | `src/logic/mod.rs` 的 `SchemeLogic` | 瞬时量；快照只允许在 tick 边界拍摄，此时队列应已排空 |
| `registered_hooks`、`extra_version` | `src/logic/mod.rs` 的 `SchemeLogic` | 由当前装载的脚本推导的缓存，装载时重建 |
| 脚本文件监视器 `watcher`、热重载检查时刻 `last_reload_check` | `src/logic/mod.rs`、`src/runtime.rs` | 开发工具状态，与世界演化无关 |
| 配置：`time_scale`、`autosave_sec`、`snapshot_heartbeat_ms`、`log_level`、`is_paused`、`admin_token` | `src/runtime.rs` 的 `TextCraftServer` | 启动配置；其中 `is_paused` 是控制面状态而非世界状态，epoch 协议另行管理 |
| 存档文件路径 `save_file` | `src/logic/mod.rs` | 配置；存档文件内容本身是快照 v3 的一种落盘形态（v2 旧存档仍可兼容读取） |

## 5. 评审问答（验收用）

- 随便指一份状态，它在快照的哪个字段？——因果状态查第 2 节表；查不到即未登记，先补表再谈实现。
- 恢复快照后哪些东西被允许和恢复前不同？——只有第 3、4 节列出的派生/非世界状态。
- 相同输入下回滚重演与不间断运行结果是否应一致？——第 2 节全部字段一致即应一致（状态哈希相等，第 4 项落地后以哈希为判据）。
