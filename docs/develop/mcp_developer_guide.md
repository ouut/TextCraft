# TextCraft 5.0: MCP (Model Context Protocol) 开发者与 AI 协同开发指南

> **当前分支状态（feat/simulator-upgrade，2026-11-10 基座改造后）**：宿主工具集新增快照 v3 检查点（`create_checkpoint` / `rollback_checkpoint`）、状态哈希（`state_hash`）与确定性评测（`run_epoch`＋宿主侧断言）；服务端新增 `--eval` 评测模式，冻结全部写类工具与 Scheme 非确定性设施（见第五节）。模型脚本位于模型目录（`--scripts-dir`，默认 `models/roguelike/`）。模型状态的唯一权威是 Flecs ECS（含 `WorldState` 单例），不存在独立的 `*world-state*`。

> [!IMPORTANT]
> **TextCraft 5.0 MCP 核心设计哲学**：
> TextCraft 采用 **「单进程 `--mcp-port` HTTP JSON-RPC 2.0 管道 + Flecs ECS 权威底座透视 + Agent 自动驾驶与无头推演 + Scheme 动态可扩展 MCP 工具注册表」** 架构。
> - **同进程实时同屏**：AI 客户端通过 HTTP 连接正在运行的游戏服务端，AI 的每一次指令、传送、刷怪或代码热改都会在人类玩家的浏览器视口与 HUD 日志中毫秒级同步展现！
> - **权威 ECS 状态透视**：通过 `ecs_inspect`、`ecs_query` 与 `ecs_modify`，AI 可以亚微秒级穿透 C-Core 内存直接检索、内省和原子修改万级实体、解剖学肢体树与化学液体池！
> - **无头虚拟推演与时空旅行**：支持以任意时间步长 `dt` 进行无头仿真推演 (`sim_step`)、确定性随机种子 (`seed`)、状态作用域差分 (`state_diff`) 与隔离的智能体黑板记忆 (`mcp_storage_*`)；确定性评测请走 `run_epoch`（宿主断言在被评测 VM 之外求值），`assert_state` 仅限 dev 模式调试。
> - **零编译动态造工具**：AI 与开发者无需修改或重新编译任何 Rust 代码，只需在模型脚本（或在线 `!` REPL，限 dev 模式）中调用 `(mcp:def-tool ...)` 并搭配 `(ecs:* ...)` 原语即可随时扩展全新的 ECS MCP 工具！

---

## 一、 架构原理与数据流拓扑

```mermaid
flowchart LR
    subgraph AIClient["AI 客户端 (Antigravity / Cursor / Claude Code)"]
        LLM["LLM / Agent"]
    end

    subgraph TextCraftProcess["TextCraft 单进程服务端 (--port 8081 --mcp-port 8082 -w)"]
        MCPHttp["Axum MCP HTTP 监听器<br>POST /mcp (JSON-RPC 2.0)"]
        Chan["无锁 mpsc + oneshot 通道"]
        TickLoop["主循环 tick(dt) -> drain_mcp_requests()"]
        HostBridge["宿主核心桥接工具<br>game_command / player_input / reload_scripts"]
        SchemeVM["Steel Scheme VM<br>(mcp:def-tool 动态注册表；状态在 ECS WorldState 单例)"]
        WSGateway["WebSocket 网关 (:8081)"]
    end

    subgraph Browser["人类玩家浏览器 (http://127.0.0.1:8081)"]
        WebUI["<pre id='screen'> + 浮动 HUD"]
    end

    LLM <-->|HTTP JSON-RPC 2.0| MCPHttp
    MCPHttp <--> Chan <--> TickLoop
    TickLoop --> HostBridge & SchemeVM
    SchemeVM -->|快照与事件实时推流| WSGateway <--> WebUI
```

### 为什么采用单进程 `--mcp-port` 通道转发？
1. **绝对线程安全与零锁竞争**：`SchemeLogic`（内含 Steel VM 与 Flecs 世界句柄）驻留在主循环线程。Axum MCP HTTP Handler 收到 JSON-RPC 2.0 请求后，通过 `mpsc::unbounded_channel` 将请求投递给主循环，并在 `TextCraftServer::tick()` 的 `drain_mcp_requests()` 阶段顺序执行，彻底消除多线程并发读写虚拟机状态的竞态风险。
2. **与浏览器玩家共享同一世界**：传统 Stdio MCP 会启动一个孤立的子进程，导致 AI 看不到浏览器中正在游玩的玩家。通过 `--mcp-port 8082`，AI 直接接入人类玩家所在的同一个内存世界。
3. **ANSI 自动剥离（Token 极简友好）**：游戏推给浏览器的是带 24-bit TrueColor ANSI 转义序列的字符流，而 MCP 接口通过 `strip_ansi_codes`（及 Scheme 宿主函数 `(engine:strip-ansi str)`）自动剥离颜色控制码，向大模型返回干净紧凑的纯文本 ASCII 2D 网格画面。

---

## 二、 启动与 AI 客户端配置指南

### 2.1 启动带 MCP 端口与热重载的服务端

在开发与 AI 结对编程时，推荐同时开启 `--mcp-port` 与 `-w`（2 秒周期脚本热重载）：

```bash
cargo run -- --port 8081 --mcp-port 8082 -w --log-level debug
```

启动成功后：
- **浏览器游戏入口**：`http://127.0.0.1:8081`（例如访问 `http://127.0.0.1:8081/#/alice/admin`）
- **MCP HTTP JSON-RPC 2.0 端点**：`http://127.0.0.1:8082/mcp`（同时兼容根路径 `http://127.0.0.1:8082/`）

### 2.2 在 AI 客户端中配置 TextCraft MCP Server

#### Antigravity / Cursor / Windsurf (`mcp_config.json` 或 `.cursor/mcp.json`)

```json
{
  "mcpServers": {
    "textcraft": {
      "url": "http://127.0.0.1:8082/mcp"
    }
  }
}
```

#### Claude Code CLI

```bash
claude mcp add --transport http textcraft http://127.0.0.1:8082/mcp
```

#### 使用 `curl` 手动验证 MCP 接口

```bash
# 1. MCP 握手初始化 (initialize)
curl -s -X POST http://127.0.0.1:8082/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}'

# 2. 获取所有可用工具列表 (包含宿主桥接工具 + Scheme 动态工具)
curl -s -X POST http://127.0.0.1:8082/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
```

---

## 三、 宿主核心桥接工具：直接复用游戏命令与输入管线

Rust 宿主层内置了 **3 个永不失效的核心桥接工具**（即使 `.scm` 脚本被改坏，AI 依然可以通过它们修复并重载脚本）：

### 1. `game_command` —— 复用游戏底部命令输入框与 `!` 管理员 REPL
直接复用游戏内命令输入管线，支持两种模式：
- **普通游戏命令**（不以 `!` 开头）：调用 `logic.on_command(player_id, command)`，执行后自动返回该玩家最新的纯文本 `ascii_screen` 视口与 `hud` 状态。
  ```json
  {
    "name": "game_command",
    "arguments": {
      "player_id": 1,
      "command": "(where)"
    }
  }
  ```
  常用内置命令：`(help)`、`(where)`、`(heal)`、`(say "你好！")`、`(dw)`、`(fps 60)`、`(theme "cyberpunk")`。
- **Scheme 实时 REPL 求值**（以 `!` 开头）：直接调用 `logic.eval_repl(code)` 在虚拟机全局环境中求值任意 Scheme 表达式并返回结果（拥有最高宿主权限，无需玩家在线 Token 鉴权）：
  ```json
  {
    "name": "game_command",
    "arguments": {
      "command": "! (set-player 1 (hash-insert (ensure-player 1) \"gold\" 9999))"
    }
  }
  ```

### 2. `player_input` —— 模拟玩家键盘移动、UI 点击与鼠标交互
直接复用 `process_player_input(player_id, input)`，模拟玩家操作并返回执行后的纯文本 `ascii_screen` 与 `hud`：
- **模拟按键移动/交互 (`type: "key"`)**：
  ```json
  {
    "name": "player_input",
    "arguments": {
      "player_id": 1,
      "type": "key",
      "code": "KeyD"
    }
  }
  ```
- **模拟 HUD 动作 (`type: "ui"`)**：
  ```json
  {
    "name": "player_input",
    "arguments": {
      "player_id": 1,
      "type": "ui",
      "action": "use_item",
      "payload": { "id": "potion" }
    }
  }
  ```

### 3. `reload_scripts` —— 热重载与干净重置 `.scm` 脚本
无视 `--watch` 开关或 2 秒轮询间隔，立即扫描并重编译当前模型目录（`--scripts-dir`，默认 `models/roguelike/`）下的全部 `.scm` 根脚本文件。
支持可选参数 `clean: bool`（**默认 `true`**）：
- **干净重置模式（`clean: true`，默认/缺省）**：
  编译新脚本后，**不恢复旧状态**，主动重新调用 `(engine:on-init)`，以全新初始状态重置世界并刷新画面（递增渲染版本号），适合 AI Agent 重新初始化世界或开发新游戏原型时使用。
  ```json
  {
    "name": "reload_scripts",
    "arguments": {
      "clean": true
    }
  }
  ```
  （注：调用时不传 `arguments` 或省略 `clean` 字段同样默认执行干净重置。）
- **无损热补丁模式（`clean: false`）**：
  保持原有的无损热重载逻辑，在编译前经 `(engine:export-state)` 导出当前 WorldState 单例状态，并在编译成功后经 `(engine:import-state)` 完整恢复（ECS 实体不随脚本重编译清空；用于玩家在线游玩时无缝打热补丁）。
  ```json
  {
    "name": "reload_scripts",
    "arguments": {
      "clean": false
    }
  }
  ```
- **自动安全回滚**：
  若新脚本存在括号不匹配、未定义变量等语法或编译错误，无论 `clean` 参数为何值，均会自动安全回滚恢复重载前的原有世界状态，并返回详细编译错误栈（`isError: true`），方便 AI Agent 改完 `.scm` 文件后立即自检与修复。

### 4. `pause_clock` 与 `resume_clock` —— 虚拟模拟时钟暂停与恢复
- `pause_clock`: 将主物理时钟暂停。世界 `tick(dt)` 推进立即停止，但 WebSocket 网络消息收发、心跳与 MCP 请求正常保活，方便 AI 介入冻结现场并执行精确诊断。
- `resume_clock`: 恢复物理时钟定时驱动。

### 5. `step_simulation` / `sim_step` —— 虚拟时钟多步推演、确定性种子与输入注入
支持以任意步长 `dt` 单步或连续推演指定步数 `steps`，支持确定性伪随机种子 `seed`，支持按键长按持续时间 `hold_ticks`，以及状态投影和范围差分：
```json
{
  "name": "step_simulation",
  "arguments": {
    "steps": 3,
    "dt": 0.016,
    "seed": 12345,
    "inputs": [
      {
        "step": 0,
        "player_id": 1,
        "type": "key",
        "code": "KeyD",
        "action": "down"
      }
    ],
    "state_projection": ["players.1.x", "players.1.hp"],
    "include_snapshot": true,
    "snapshot_player_id": 1,
    "include_state_diff": true,
    "diff_path_prefix": "players.1",
    "diff_exclude_keys": ["chunks"]
  }
}
```
- `seed`: 可选 64 位整型种子，推演开始前配置确定性 PRNG，相同状态 + 相同种子 + 相同按键 = 100% 确定性时空复现。
- `hold_ticks`: 可选长按帧数，在执行指定步数期间自动维持按键按压并在结束时释放，解锁即时物理跳跃与连续移动。
- `state_projection`: 可选路径投影列表（如 `["players.1.x", "tick_count"]`），从世界状态按需投影，避免返回巨型全量状态。
- `diff_exclude_keys`: 增量差分排除键（**缺省默认排除 `["chunks"]`**），屏蔽数十万字节不变瓦片噪声。

### 6. `get_state_diff` / `state_diff` —— 轻量级微秒级状态差分比较与路径范围过滤
对比前后两个 JSON 状态树或当前内存世界状态，支持指定前缀路径和排除键：
```json
{
  "name": "get_state_diff",
  "arguments": {
    "path_prefix": "players.1",
    "exclude_keys": ["chunks"]
  }
}
```
以扁平化路径输出：
- `changed`: 如 `"players.1.x": { "old": 5, "new": 6 }`
- `added`: 如 `"players.2": { ... }`
- `removed`: 如 `"score": 0`

### 7. `query_state` —— 状态按需投影与字段掩码 (Field Masking)
按需抓取指定的点分隔路径字段，将数十 KB 甚至几百 KB 的巨型世界状态树压缩为仅含核心关注字段的微型 JSON：
```json
{
  "name": "query_state",
  "arguments": {
    "paths": ["tick_count", "players.1.x", "players.1.hp", "monsters"]
  }
}
```
返回仅包含请求路径的树状 JSON 对象，极大节省大模型上下文 Token。

### 8. `eval_assertion` / `assert_state` —— 批量断言与富差值对比 (TDD 单元测试套件)

> **注意（基座改造后）**：这两个工具是在被测模型同一个 Scheme VM 内求值断言，只适合 dev 模式调试。正式评测必须走 `run_epoch` 的**宿主侧断言**（第 22 条）：断言以 `{path, op, expected}` 数据形式给出，由 Rust 宿主对导出的快照 v3 状态求值，被评测的模型代码无法干预。在 `--eval` 模式与 `run_epoch` 执行期间，`eval_assertion` / `assert_state` 会被门禁拒绝。

在带有看门狗保护的 Scheme 沙箱中求值单个或批量断言表达式，支持富差值诊断：
- **单表达式模式**：支持 `expr` 或 `code` 字符串参数。
- **批量模式 (`exprs: string[]`)**：
  ```json
  {
    "name": "eval_assertion",
    "arguments": {
      "exprs": [
        "(= (+ 2 3) 5)",
        "(assert:equal (get-gold 1) 20)",
        "(assert:true (= (get-hp 1) 100))"
      ]
    }
  }
  ```
  返回结构化聚合测试报告：
  ```json
  {
    "all_passed": false,
    "total": 3,
    "passed": 2,
    "failed": 1,
    "results": [ ... ],
    "failures": [
      {
        "expr": "(assert:equal (get-gold 1) 20)",
        "expected": 20,
        "actual": 15
      }
    ]
  }
  ```

### 9. `mcp_store_set` / `mcp_storage_set` —— 写入专用 MCP 存储 / 智能体黑板
在 Rust 宿主专用内存存储（Blackboard）中存储或更新任意 JSON 数据，完全不污染游戏脚本的业务状态或正常游戏存档：
```json
{
  "name": "mcp_store_set",
  "arguments": {
    "key": "exploration_plan",
    "value": { "phase": 1, "target": "dungeon_north", "rooms_visited": [101, 102] },
    "namespace": "agent_alpha"
  }
}
```
- 参数：`key: string`（必需），`value: any`（必需，任意基本类型/对象/数组），`namespace?: string`（可选命名空间，默认 `"default"`）。

### 10. `mcp_store_get` / `mcp_storage_get` —— 读取专用 MCP 存储数据项
根据键名和可选命名空间获取数据。若不存在返回 `{"found": false, "value": null}`：
```json
{
  "name": "mcp_store_get",
  "arguments": {
    "key": "exploration_plan",
    "namespace": "agent_alpha"
  }
}
```

### 11. `mcp_store_delete` / `mcp_storage_delete` —— 删除专用 MCP 存储数据项
从专用存储中删除指定数据项：
```json
{
  "name": "mcp_store_delete",
  "arguments": {
    "key": "exploration_plan",
    "namespace": "agent_alpha"
  }
}
```

### 12. `mcp_store_list` / `mcp_storage_keys` —— 列出指定命名空间或匹配前缀的键
列出当前专用存储中的所有键，支持按命名空间过滤或按前缀过滤：
```json
{
  "name": "mcp_store_list",
  "arguments": {
    "namespace": "agent_alpha",
    "prefix": "explor"
  }
}
```

### 13. `mcp_store_clear` —— 清空专用存储
清空专用存储中全部数据，或仅清空指定命名空间下的数据：
```json
{
  "name": "mcp_store_clear",
  "arguments": {
    "namespace": "agent_alpha"
  }
}
```

### 14. `mcp_store_export` —— 导出专用 MCP 存储
将专用存储完整备份至磁盘文件：
```json
{
  "name": "mcp_store_export",
  "arguments": {
    "path": "backups/agent_memory.json"
  }
}
```

### 15. `mcp_store_import` —— 导入专用 MCP 存储
从磁盘文件恢复专用存储（支持 `overwrite` 覆盖或合并）：
```json
{
  "name": "mcp_store_import",
  "arguments": {
    "path": "backups/agent_memory.json",
    "overwrite": true
  }
}
```

### 16. `ecs_inspect` —— Flecs ECS 单实体全图谱深度内省与原地打补丁
用于根据实体数值 ID (`eid`) 深度读取实体在 Flecs C 内存中的所有组件、关系对与动态属性，并支持在内省时原地打补丁修改：
```json
{
  "name": "ecs_inspect",
  "arguments": {
    "eid": 1024,
    "modify": {
      "set_props": { "hp": 150, "state": "enraged" },
      "add_tags": ["Elite", "Burning"]
    }
  }
}
```
返回实体完整结构化 JSON：
- `id` / `name`：实体数值 ID 与名称字符串；
- `pos`：坐标 `{ "x": 12, "y": -4, "layer": 2 }`；
- `glyph`：图元与穿透 `{ "ch": "M", "fg": [255, 60, 60], "bg": [0, 0, 0], "solid": true, "opaque": false }`；
- `tags`：所附带的标签数组，如 `["Monster", "Hostile", "Elite"]`；
- `dynamic_props`：动态属性字典，如 `{ "hp": 150, "max_hp": 200, "attack": 18 }`；
- `relationships`：实体关系对，如 `[ { "relation": "PartOf", "target": 100 } ]`；
- `player_session`：若绑定了玩家会话，返回关联的网络 `player_id`。

### 17. `ecs_query` —— Flecs C 原生查询 DSL 极速空间检索
直接暴露 Flecs 官方 C 查询引擎 DSL，微秒级在十万级实体中筛选匹配的实体，并返回匹配实体的内省详情列表：
```json
{
  "name": "ecs_query",
  "arguments": {
    "query": "GridPos, Monster, !Dead",
    "limit": 20
  }
}
```
- `query`：Flecs 查询表达式，例如：
  - `"GridPos, Monster"`：拥有位置且标记为怪物的活跃实体；
  - `"Liquid, (CoveredWith, $target)"`：液体与沾染关系查询；
  - `"Item, !Equipped"`：未被装备的地表物品；
- `limit`：返回实体数量上限（默认 50，防止超大列表撑爆模型上下文）。

### 18. `ecs_modify` —— 细粒度原子修改 ECS 实体
对指定实体执行精细的物理、组件与关系增删改，亚微秒级生效且不产生垃圾回收负担：
```json
{
  "name": "ecs_modify",
  "arguments": {
    "eid": 1024,
    "set_pos": { "x": 15, "y": 8, "layer": 2 },
    "set_glyph": { "ch": "E", "fg": [255, 200, 50], "solid": true },
    "set_props": { "hp": 80 },
    "delete_props": ["poison_timer"],
    "add_tags": ["Frozen"],
    "remove_tags": ["Burning"],
    "relate": { "relation": "PartOf", "target": 500 },
    "despawn": false
  }
}
```
- `set_pos`：更新空间网格坐标与渲染图层（同时自动刷新 `SpatialGrid` 空间哈希）；
- `set_glyph`：更新字符显示、前景色/背景色及阻挡/遮光属性；
- `set_props` / `delete_props`：原子更新或删除动态属性键值对；
- `add_tags` / `remove_tags`：添加或剥离标签；
- `relate` / `unrelate`：建立或解除关系对（如 `EquippedOn`、`PartOf`）；
- `despawn: true`：物理销毁实体及其关联的子级关系树。

### 19. `create_checkpoint` —— 宿主快照 v3 检查点（全量因果状态）
由 Rust 宿主实现：把当前全部因果状态拍成快照 v3（Flecs 世界——含 `WorldState` 单例与全部实体/标签/关系——＋ PRNG 状态 ＋ tick），以标签存入 MCP 黑板的 `checkpoint` 命名空间（黑板只是存储介质，不随回滚被回滚）：
```json
{
  "name": "create_checkpoint",
  "arguments": { "tag": "before_boss" }
}
```
- `tag` 可选，默认 `"latest"`。
- 这是当前唯一的检查点正道：覆盖 ECS 侧状态（旧的脚本版 checkpoint 只存 Scheme 状态，见第四节的兼容垫片说明）。

### 20. `rollback_checkpoint` —— 回滚到宿主检查点
按标签恢复 `create_checkpoint` 拍下的快照（世界按实体 id 原样重建，PRNG 与 tick 一并恢复）。导入兼容 v2 快照（其 `world_state` 段会灌入 WorldState 单例）；存入的不是快照格式的历史杂值会被显式报版本错误，不会静默接受。标签不存在时返回未找到。

### 21. `state_hash` —— 当前状态的规范哈希
无参数。返回当前快照 v3 的 SHA-256 哈希（规范序列化：JSON 键递归排序后经 `sha2` 计算）。相同因果状态必然得到相同哈希，是确定性与回滚验收的判据。

### 22. `run_epoch` —— 确定性评测一轮（干净重置 → 装载 → 种子 → 步进+输入 → 宿主断言 → 报告）
```json
{
  "name": "run_epoch",
  "arguments": {
    "scripts_dir": "tests/fixtures/models/market_sim",
    "seed": 7,
    "steps": 10,
    "dt": 0.016,
    "inputs": [
      { "step": 0, "player_id": 1, "type": "key", "code": "KeyB", "action": "down" }
    ],
    "assertions": [
      { "path": "accounts.alice.balance", "op": "eq", "expected": 946 },
      { "path": "tick_count", "op": "exists" }
    ]
  }
}
```
- 参数：`scripts_dir`（模型目录，内含 `model.json` manifest，缺失字段按默认值推导；manifest 的 name/state_schema 与快照不符会被显式拒绝）、`seed`（缺省取 manifest 的 `default_seed`）、`steps`、`dt`（固定步长，缺省 0.016）、`inputs`（按步注入）、`assertions`（可省略，缺省读 manifest 指向的断言文件）。
- 断言格式 `{path, op, expected}`：`path` 是对导出状态的点分路径（如 `players.1.hp`、`tick_count`）；`op` 取 `eq` / `ne` / `gt` / `gte` / `lt` / `lte` / `exists` / `absent`；由宿主在被评测 VM 之外求值。
- 返回的 epoch 报告不含任何时间戳/耗时，同配置双跑逐字节相同，字段包括：`mode: "eval"`、`host`（宿主指纹：`crate_version` / `flecs_ecs_rev` / `steel_core_version`，均为编译期常量）、`dt: {source: "fixed", value}`、`scripts_hash`、`assertions_hash`、`seed`、`steps`、`final_state_hash`、`all_passed`、`results`。
- epoch 执行期间自动启用 eval 门禁（工具层＋语言层，见第五节），结束后恢复（常驻 `--eval` 除外）。

---

## 四、 Scheme 动态 MCP 工具系统 (`mcp:def-tool`)

### 4.1 核心范式：检查点已收归宿主，黑板只作存储介质

基座改造前，检查点是脚本用黑板原语自举的（只存 Scheme 侧状态，覆盖不到 ECS 实体与 PRNG，是回滚丢肢体/液体状态的根源）。改造后**检查点由 Rust 宿主实现**：`create_checkpoint` / `rollback_checkpoint` 是宿主工具（第 19、20 条），快照 v3 覆盖全部因果状态，MCP 黑板（`engine:raw-mcp-store-*` 与 `(mcp-store:set! ...)` 等封装）只充当按标签存放快照的介质，本身仍是 Agent 记忆、不是世界状态，恢复快照不会回滚黑板。

兼容垫片：`models/roguelike/prelude.scm` 仍保留旧的 `(checkpoint:save tag)` / `(checkpoint:restore tag)` 函数，但它们只经 `state:all` / `state:replace!` 存取 WorldState 单例的键值、**不含 ECS 实体与 PRNG**，仅供旧模型代码平滑过渡，新代码与 Agent 评测一律使用宿主检查点工具。

### 4.1 预置的 Scheme 游戏开发与调试工具 (`models/roguelike/main.scm`)

示例模型 `main.scm` 默认通过 `(mcp:def-tool ...)` 注册了 4 个高频工具：

| 工具名称 | 参数 (`arguments`) | 功能说明 |
| :--- | :--- | :--- |
| `get_world_summary` | `{}` | 返回 `tick_count`、`target_fps`、所有玩家摘要（坐标、HP、金币、在线状态）与所有存活怪物列表（自动剔除庞大的 `chunks` 原始瓦片数据） |
| `get_viewport` | `{ "player_id"?: 1 }` | 返回指定玩家的坐标 (`x`, `y`)、视口尺寸 (`cols`, `rows`)、去除 ANSI 后的纯文本 `ascii_screen` 画面以及完整 `hud` 数据 |
| `teleport_player` | `{ "player_id"?: 1, "x": 5, "y": -3 }` | 将指定玩家瞬移至 `(x, y)`，向其 HUD 日志推送 `[mcp] AI 将你传送至 (x, y)`，并触发画面刷新 |
| `spawn_monster` | `{ "id": "boss_1", "name": "炎魔", "glyph": "B", "x": 4, "y": 2, "hp"?: 120, "attack"?: 18 }` | 在 `(x, y)` 处动态生成新怪物，经 `state:*` 门面写入单例的 `"monsters"` 键与 `'actor` 图层，并向全服玩家广播警告日志 |

### 4.2 如何用 `(mcp:def-tool ...)` 动态开发新的 MCP 工具

模型 `prelude.scm` 提供了极简的声明式注册函数：

```scheme
(mcp:def-tool name description input-schema-hash handler-proc)
```

- `name`：工具英文名称字符串（如 `"build_wall_rect"`）。
- `description`：工具用途说明（展示给 AI 客户端的 `tools/list`）。
- `input-schema-hash`：符合 JSON Schema 规范的 Scheme `hash`（如 `(hash "type" "object" "properties" ...)`）。
- `handler-proc`：接收单参数 `args`（由调用方的 `arguments` JSON 对象自动转换而来的 Scheme `hash`），返回任意可序列化为 JSON 的 Scheme 数据结构（推荐返回 `hash`）。

#### 实战示例：在 `models/roguelike/main.scm` 中新增「矩形造墙工具」

只需在模型 `main.scm` 末尾追加以下代码并保存（开启 `-w` 时 2 秒内自动生效，或调用 `reload_scripts` 立即生效）：

```scheme
(mcp:def-tool
  "build_stone_room"
  "在指定矩形区域 (x1, y1) 到 (x2, y2) 建造石墙房间"
  (hash
    "type" "object"
    "properties" (hash
                   "x1" (hash "type" "integer" "description" "左上角 X")
                   "y1" (hash "type" "integer" "description" "左上角 Y")
                   "x2" (hash "type" "integer" "description" "右下角 X")
                   "y2" (hash "type" "integer" "description" "右下角 Y"))
    "required" (list "x1" "y1" "x2" "y2"))
  (lambda (args)
    (let ((x1 (map-get args "x1" -3))
          (y1 (map-get args "y1" -3))
          (x2 (map-get args "x2" 3))
          (y2 (map-get args "y2" 3))
          (count 0))
      (let loop-y ((y y1))
        (if (<= y y2)
            (begin
              (let loop-x ((x x1))
                (if (<= x x2)
                    (begin
                      (if (or (= x x1) (= x x2) (= y y1) (= y y2))
                          (begin
                            (world:set-tile x y 'terrain
                              (hash "glyph" "#" "fg" '(180 180 190) "bg" '(40 40 45) "solid" #t "name" "stone_wall"))
                            (set! count (+ count 1)))
                          #f)
                      (loop-x (+ x 1)))
                    #f))
              (loop-y (+ y 1)))
            #f))
      (engine:broadcast "system" (string-append "[mcp] AI 建造了石墙房间，共放置 " (number->string count) " 块石墙！"))
      (hash "ok" #t "walls_placed" count))))
```

#### 运行期在线自举：通过 `game_command` 零文件修改注册临时 MCP 工具

AI 甚至无需修改磁盘文件，直接通过 `game_command` 发送 `! (mcp:def-tool ...)` 即可在内存中实时注册一个新 MCP 工具并立即调用：

```json
{
  "name": "game_command",
  "arguments": {
    "command": "! (mcp:def-tool \"gift_gold\" \"Give gold to player\" (hash \"type\" \"object\") (lambda (args) (let* ((pid (map-get args \"player_id\" 1)) (amt (map-get args \"amount\" 100)) (p (ensure-player pid)) (new-g (+ (map-get p \"gold\" 0) amt))) (set-player pid (hash-insert p \"gold\" new-g)) (player:add-log pid \"system\" (string-append \"[mcp] AI 赠送了你 \" (number->string amt) \" 金币！\")) (hash \"ok\" #t \"gold\" new-g))))"
  }
}
```
随后调用 `tools/list` 即可发现 `gift_gold`，并通过 `tools/call` 直接使用！

---

## 五、 eval 评测模式（`--eval`）：开发能力在评测期冻结

服务端有两种运行模式。**dev 模式**（默认）保留全部 v5.0 开发面：`!` REPL 在线求值、`ecs_modify` 直接改实体、`reload_scripts` 热重载、运行期 `(mcp:def-tool ...)` 注册新工具、同 VM 的 Scheme 断言——开发期它们是效率工具。**eval 模式**专为可信评测：以 `--eval` 启动后常驻开启门禁，`run_epoch` 执行期间也会临时开启同一套门禁。门禁只在两处生效，dev 模式行为零改动：

### 5.1 MCP 分发层门禁（`src/runtime.rs` 的 `eval_block_reason`，全工具一处拦截）

| 调用 | eval 下结果 |
| :--- | :--- |
| `ecs_modify` | 拒绝 |
| `reload_scripts` | 拒绝 |
| `create_checkpoint` / `rollback_checkpoint` | 拒绝（只许 `run_epoch` 内部使用快照） |
| `eval_assertion` / `assert_state` | 拒绝（断言改走 `run_epoch` 宿主侧求值） |
| `game_command` 且命令以 `!` 开头 | 拒绝（普通游戏命令放行） |
| `ecs_inspect` 带 `modify` 补丁 | 拒绝（等价于 `ecs_modify` 的后门）；纯内省放行 |
| Scheme 动态工具 | 冻结到门禁开启瞬间已注册的集合（allowlist）；之后运行期新注册的在 eval 下不可见、不可调用 |
| 其余只读/推进工具（`query_state`、`state_diff`、`state_hash`、`sim_step`、`player_input` 等） | 放行 |

### 5.2 Steel 语言层门禁（`src/logic/mod.rs` 的 `set_eval_language_gate`）

工具层冻结挡不住模型代码自己在 VM 里取墙钟/系统熵，因此 eval 下还会把 Steel 标准库中的非确定性设施替换为报错桩（含 `#%prim.` 拼写），并同步替换模块注册表项，防止用 `(require-builtin ...)` 把原函数重新引入：

- `steel/time`：全部时间函数（墙钟）；
- `steel/random`：OS 熵随机（与宿主已种子化的 `engine:random-int` 无关，后者照常可用）；
- `steel/meta`：环境变量读写、宿主指纹，以及 `eval` / `Engine::new` 等代码加载逃逸口；
- `steel/filesystem`、`steel/ports` / `steel/io`：任意文件与标准输入输出读写；
- `steel/process`：子进程。

门禁关闭（epoch 结束且非常驻 eval）时原绑定完整恢复。当前仓库内模型脚本均未使用上述被禁设施（已 grep 核查），门禁对它们透明。

### 5.3 确定性边界

eval 与 `run_epoch` 的可复现保证只覆盖无头路径：epoch 以固定 `dt` 步进、PRNG 为宿主自持 xoshiro256++（状态进快照）。live 实时循环的 `dt` 仍取墙钟流逝时间，其确定性不受保证（见 `docs/project/known_limitations.md` (c)）。
