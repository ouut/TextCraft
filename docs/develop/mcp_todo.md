# TextCraft 3.0: MCP 深度优化与自闭环演进 TODO 规划

> [!NOTE]
> 本文档记录了在跑通 Milestone 1（无头推演与状态差分）、Milestone 2（专用存储黑板与时光机快照）以及干净重载机制后，进一步将 TextCraft 打磨为**工业级 Agent-in-the-Loop 自驱游戏开发引擎**所识别出的 **8 个核心演进问题与优化项**，供后续深入讨论与排期落地。

---

## 优化任务总览与优先级矩阵

| 序号 | 核心问题 | 影响维度 | 优先级 | 预期收益 |
| :---: | :--- | :--- | :---: | :--- |
| **1** | **可控随机种子与确定性仿真 (Deterministic PRNG & Seed)** | 稳定性 / 可复现性 | **P0** | 彻底消除测试偶发抖动（Flaky Tests），保证 Bug 100% 确定性单步复现 |
| **2** | **批量断言与富差值反馈 (Batch Assertions & Rich Diff)** | 交互效率 / Token | **P0** | 一次 MCP 请求执行全量测试套件，给出 `expected vs actual` 语义差值 |
| **3** | **状态按需投影与字段掩码 (State Projection / Mask)** | Token 上下文经济学 | **P0** | 避免百 KB 全量状态撑爆大模型 Context Window，按需抓取关键字段 |
| **4** | **时序事件流与全局遥测 (Temporal Event Telemetry)** | 观测精度 / 调试 | **P1** | 推演时输出精确到帧（Step ID）的结构化事件，覆盖全局环境与怪兽行为 |
| **5** | **状态增量路径过滤 (State Diff Path Scoping)** | 比对性能 / 噪声消除 | **P1** | 排除数十万字节的不变瓦片（`chunks`），只聚焦特定实体差异 |
| **6** | **运行期性能剖析与热点打点 (Performance Profiler)** | 自主性能优化 | **P1** | 细分 `ai/physics/render` 耗时，帮助 Agent 定位低效算法并重构代码 |
| **7** | **脚本模块化与子目录动态加载 (Modular Require/Load)** | 代码工程化管理 | **P2** | 突破单层平铺目录限制，支持复杂游戏的子模块划分与作用域隔离 |
| **8** | **连续输入与物理时序 (Continuous Input & KeyUp/Hold)** | 游戏品类扩展 | **P2** | 支持按键释放（KeyUp）与长按持续时间，解锁即时物理与平台跳跃类玩法 |

---

## 八大演进问题详述

### 1. 可控随机种子与确定性仿真（Deterministic PRNG & Seed）

- **现状分析**：
  目前宿主暴露的 `(engine:random-int min max)` 直接基于 Rust 系统的 `rand::thread_rng()` 硬件随机熵。
- **核心痛点**：
  - **测试偶发失败（Flaky Tests）**：战斗暴击、随机掉落、怪物随机游走等机制会导致 Agent 编写的单元测试跑 10 次有 1~2 次失败，让 Agent 陷入错误的 Bug 排查死循环；
  - **Bug 不可复现**：在长时间无头推演（如 1000 步）中偶然触发崩溃时，缺少 Seed 导致无法重放相同的时空路径进行现场抓包。
- **建议方案**：
  - 引入可控伪随机发生器（如 PCG / ChaCha8），支持显式设置种子：
    - Scheme 脚本接口：`(engine:set-seed! seed)`
    - MCP 工具参数：在 `step_simulation` 中增加可选参数 `"seed": 123456`
  - 相同初始状态 + 相同种子 + 相同按键序列 = **100% 绝对一致的推演轨迹**。

---

### 2. 批量断言与富差值对比（Batch Assertions & Rich Diff）

- **现状分析**：
  目前的 `eval_assertion` 工具一次只能求值一个 Scheme 表达式，若结果为 `#f` 仅返回生硬的 `Assertion failed: expression evaluated to #f`。
- **核心痛点**：
  - **通信轮次膨胀**：一个稍微复杂的玩法验收通常需要验证 5~10 个属性（金币、HP、坐标、背包道具数量、任务状态），Agent 必须连续发起多次 HTTP 请求，对话上下文膨胀且等待网络往返拖慢思考；
  - **缺少数值差（Diff）**：Agent 无法获知当前实际值是多少（例如：期望金币为 20，实际金币为 15），需要再调一次 REPL 查询当前值，增加了交互成本。
- **建议方案**：
  - 支持批量断言输入：`eval_assertions(exprs: string[])`；
  - 在 Scheme 端提供更丰富的断言原语，如 `(assert-equal actual expected [label])`、`(assert-true cond [label])`；
  - 返回结构化断言报告：
    ```json
    {
      "all_passed": false,
      "passed_count": 3,
      "failed_count": 1,
      "failures": [
        { "expr": "(assert-equal (get-gold 1) 20)", "expected": 20, "actual": 15, "label": "gold check" }
      ]
    }
    ```

---

### 3. 状态按需投影与字段掩码（State Projection & Field Masking）

- **现状分析**：
  目前的 `export_state()` 或 `step_simulation(include_snapshot: true)` 返回的是整个游戏世界或玩家的完整 JSON 树。
- **核心痛点**：
  - 随着地图瓦片（`chunks`）、怪物列表、物品数据库的扩张，状态树体积可达数百 KB；
  - 大模型每次推演都接收巨型 JSON，极易超出上下文限制，且 Token 成本极高；
  - Agent 大多数时候在特定测试场景中只关心 2~3 个具体字段。
- **建议方案**：
  - 在状态查询接口中引入路径掩码（Projection / JSONPath）：
    ```json
    {
      "name": "query_state",
      "arguments": {
        "paths": ["players.1.hp", "players.1.x", "monsters.boss_1.hp"]
      }
    }
    ```
  - 宿主仅提取指定键路径的值并按树状组装返回，将响应报文从数十 KB 压减至数百字节。

---

### 4. 时序事件流与全局遥测（Temporal Event Telemetry & Step Attribution）

- **现状分析**：
  目前的事件与日志仅通过 `player:add-log` 保存在各玩家的 `p.logs` 队列中（最多 30 条），随 `hud.logs` 返回。
- **核心痛点**：
  - **无帧号时序绑定**：调用 `step_simulation(steps: 20)` 推进 20 步时，产生的所有日志混在一个列表里，无法断言“第几步触发了陷阱”、“第几步击杀了怪物”；
  - **缺少全局事件池**：如果发生环境变化（昼夜交替、天气变化、非玩家所在区域的怪物行为），在无玩家接入或无头仿真时，日志无法归属到具体玩家身上，Agent 无法感知。
- **建议方案**：
  - 建立全局事件环形缓冲（Event Sink），每条事件包含 `{ "step": 12, "tick": 1420, "type": "combat|dialog|system", "payload": {...} }`；
  - 在 `step_simulation` 的输出中增加可选字段 `"events": [...]`，仅返回在当次推演步数内产生的结构化事件。

---

### 5. 状态增量路径过滤（State Diff Path Scoping）

- **现状分析**：
  目前的 `compute_json_diff` 会递归对比前后两个完整状态树。
- **核心痛点**：
  - 如果游戏生成了大片地图瓦片（`chunks`），算法需要递归对比成百上千个瓦片哈希；
  - 即使地图完全没变，全量 Diff 依然会产生不必要的 CPU 递归比对，且如果地图有微小动态更新，会产生大量与逻辑测试无关的瓦片 Diff 噪声。
- **建议方案**：
  - 给 `get_state_diff` 与 `step_simulation(include_diff: true)` 增加作用域过滤参数：
    - `path_prefix: "players.1"`（只比对指定子树）
    - `exclude_keys: ["chunks"]`（忽略不需要对比的大型静态资产键）

---

### 6. 运行期性能剖析与热点打点（Performance Profiler & Tick Breakdown）

- **现状分析**：
  目前宿主仅能返回单帧/多步的总推演耗时 `duration_ms`。
- **核心痛点**：
  - 当 Agent 自行实现复杂玩法（如 A* 寻路、大量 NPC 视野检测、物理弹道计算）时，如果写法不当引入了 $O(N^2)$ 计算瓶颈导致掉帧；
  - Agent 无法获知“具体是哪一段 Scheme 代码占用了 90% 的 CPU 耗时”，无法进行针对性的算法优化与重构。
- **建议方案**：
  - 在 Scheme 脚本层或宿主调用层提供轻量耗时采样打点：
    `(prof:start "monster_ai") ... (prof:end "monster_ai")`；
  - 暴露 `get_profiler_summary` 工具，输出各模块的平均耗时百分比（例如：`ai: 65%, collision: 25%, render: 10%`），赋能 Agent 自主性能调优。

---

### 7. 脚本模块化与子目录动态加载（Modular Require/Load）

- **现状分析**：
  目前的脚本加载器仅扁平扫描 `scripts/*.scm` 根目录下的所有文件并注入全局环境。
- **核心痛点**：
  - 当游戏规模变大（上千行代码）时，无法分目录组织文件（如 `scripts/systems/inventory.scm`、`scripts/maps/dungeon_1.scm`）；
  - 缺少显式的模块隔离与引用原语，容易产生全局变量同名覆盖与加载顺序依赖隐患。
- **建议方案**：
  - 完善 Steel VM 的 `load` / `require` 路径支持，支持按相对路径动态加载子脚本；
  - 允许在 Scheme 内部调用 `(load "systems/combat.scm")` 进行按需引用。

---

### 8. 连续输入与物理时序（Continuous Input & KeyUp/Hold Duration）

- **现状分析**：
  目前 `PlayerInput::Key` 仅模拟单一瞬时按键（KeyDown）。
- **核心痛点**：
  - 对于网格回合制、解谜类、卡牌类、传统地牢 RPG 游戏完全足够；
  - 但若 Agent 想开发具有物理惯性、跳跃高度随按压时间变化（平台跳跃类）、长按加速/松开刹车（赛车类）等即时动作玩法时，无法表达“按住未松开”和“松开瞬间”的事件。
- **建议方案**：
  - 扩展 `PlayerInput::Key`：增加 `action: "down" | "up" | "press"`，或在仿真指令中支持传入持续帧数：
    `{ "player_id": 1, "type": "key", "code": "KeySpace", "duration_ticks": 5 }`。

---

## 后续讨论与落地建议

建议后续按如下阶段逐步细化并实现：
1. **第一阶段（立竿见影）：P0 级效率与稳定性增强**
   - 优先落地：**可控随机种子（Seed）** + **批量断言（Batch Assert）** + **状态按需查询（Path Mask）**。
   - 预期效果：彻底解决测试抖动，并大幅削减 Agent 与服务端的 MCP 对话轮次与 Token 消耗。
2. **第二阶段：P1 级深度可观测性**
   - 落地：**帧级时序事件流（Temporal Events）** + **Diff 作用域过滤** + **性能剖析器（Profiler）**。
3. **第三阶段：P2 级表现力与工程扩展**
   - 落地：**模块化加载** + **长按/物理输入支持**。
