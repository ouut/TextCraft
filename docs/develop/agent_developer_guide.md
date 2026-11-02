# TextCraft 3.0: 游戏与前端 HUD 开发者完全指南 (Agent Developer Guide)

> [!IMPORTANT]
> **致阅读本指南的开发者与智能体（Agent）**：
> TextCraft 3.0 采用 **“纯 Scheme 游戏核心 + 零业务耦合 Rust 宿主 + 极简 Web 终端底座 + 单进程 MCP AI 管道”** 架构：
> - **开发游戏玩法与世界逻辑**：只需修改 `scripts/prelude.scm` 与 `scripts/main.scm`，**无需阅读或修改任何 Rust 代码**；
> - **定制前端 HUD 界面与交互**：只需在 `web/index.html` 中调用全局桥接对象 `window.TextCraft`，自由编写原生 HTML/CSS/JS，通过纯 JSON 字典与 Scheme 通信；
> - **使用 MCP 工具与 AI 实时协同开发**：通过 `--mcp-port 8082` 开启 HTTP JSON-RPC 2.0 MCP 接口，并可在 `.scm` 中调用 `(mcp:def-tool ...)` 动态自举扩展新工具，详见 [**《MCP 开发者与 AI 协同开发指南》 (`docs/develop/mcp_developer_guide.md`)**](mcp_developer_guide.md)。

---

## 第一部分：核心架构与心智模型 (Core Mental Model)

```mermaid
flowchart TD
    subgraph Client["Web 前端 (web/index.html)"]
        Screen["<pre id='screen'><br>24-bit ANSI 极速字符流 (自适应字号居中)"]
        SDK["window.TextCraft 桥接 SDK<br>onHud / onEvent / sendAction / sendCmd / sendKey"]
        CustomHUD["自定义 HTML/CSS HUD<br>(状态栏 / 背包 / 对话弹窗 / 日志)"]
    end

    subgraph Host["Rust 宿主容器 (纯管道，零游戏结构体)"]
        Gateway["Axum HTTP + WebSocket 全双工网关<br>(Ping/Pong 保活 + 60s 断线重连宽限期)"]
        Loop["自适应主循环 tick(dt)<br>(1~60 FPS 动态变频 + 脏版本短路去重)"]
        VM["Steel Scheme VM 沙箱与看门狗保护"]
    end

    subgraph Scheme["Scheme 游戏核心 (scripts/)"]
        Prelude["prelude.scm<br>16x16 分块存储 / ANSI RLE 光栅化 / 玩家存档"]
        Main["main.scm<br>世界生成 / 碰撞战斗 / 怪物 AI / 生命周期钩子"]
        State[("*world-state*<br>纯数据全局状态树 (零闭包)")]
    end

    CustomHUD -->|调用| SDK
    SDK -->|C2S: UIAction / Key / Mouse / Cmd| Gateway --> Loop --> VM
    VM -->|触发钩子| Main
    Main <--> State
    Main --> Prelude
    Prelude & Main -->|S2C: Snapshot { cols, rows, screen, hud } + Event| Gateway
    Gateway --> Screen & SDK
    SDK -->|分发数据| CustomHUD
```

### 四大核心开发准则

1. **纯数据世界状态（Pure Data Only）**：
   全局世界状态保存在 `*world-state*` 哈希表中。其中所有数据必须为**可 JSON 序列化的纯数据**（哈希表、列表、整数、浮点数、字符串、布尔值）。**严禁在 `*world-state*` 中存储 Lambda 闭包或函数引用**，以保障热重载状态恢复与磁盘存档 100% 成功。
2. **脏版本标记（Dirty Version Tracking）**：
   引擎内置 `*visual-version*` 计数器。任何修改地图瓦片、玩家属性或双宽模式的操作都会调用 `(bump-visual-version!)`。当版本未变且未到心跳周期时，Rust 宿主会**直接跳过 `render-snapshot` 光栅化**（静止帧开销 `< 0.05ms`）。如果你直接通过 `set!` 修改了影响画面的自定义全局状态，请务必调用 `(bump-visual-version!)`。
3. **分层瓦片与 ANSI RLE 光栅化**：
   世界分为 4 个渲染图层：`terrain`（底层地形）、`surface`（地表物品/设施）、`actor`（生物/NPC/其他玩家）、`overlay`（顶层特效）。`render-viewport-ansi` 会自动合并同行连续相同颜色的 ANSI 转义码（RLE 压缩），大幅降低网络带宽与 DOM 解析开销。
4. **前后端纯数据契约（Pure JSON HUD Contract）**：
   Scheme 在 `(engine:render-snapshot pid)` 中返回的 `"hud"` 字典会被原样序列化为 JSON 推给前端 `TextCraft.onHud(hud)`；前端通过 `TextCraft.sendAction(action, payload)` 发送的交互会被路由至 Scheme 的 `(engine:on-ui-action pid action-map)`。

---

## 第二部分：Scheme 游戏逻辑开发指南 (`scripts/`)

### 2.1 Steel Scheme 常用语法速查

#### 哈希表 (Persistent Hash Map)
Steel 中的哈希表是不可变持久化数据结构（`hash-insert` 与 `hash-remove` 返回新哈希表）：

```scheme
;; 1. 创建哈希表 (键值对交替)
(define my-map (hash "name" "勇者" "hp" 100 "gold" 50))

;; 2. 安全读取字段 (推荐使用 prelude.scm 提供的 map-get，键不存在时返回默认值)
(map-get my-map "hp" 0)        ;; => 100
(map-get my-map "mana" 10)     ;; => 10

;; 3. 插入/更新字段 (返回新哈希表)
(define updated-map (hash-insert my-map "hp" 120))

;; 4. 移除字段
(define removed-map (hash-remove my-map "gold"))

;; 5. 判断键是否存在 & 获取所有键列表
(hash-contains? my-map "name") ;; => #t
(hash-keys->list my-map)       ;; => ("name" "hp" "gold")
```

#### 列表、字符串与算术
```scheme
;; 列表操作
(define items (list "sword" "potion"))
(cons "shield" items)
(append items (list "scroll"))
(for-each (lambda (x) (engine:log x)) items)

;; 字符串与类型转换
(string-append "Hero#" (number->string 1)) ;; => "Hero#1"
(to-string '(1 2 3))
(string=? s1 s2)

;; 整数除法 (网格坐标计算必须保持精确整数!)
(quotient 80 2)                            ;; => 40
(inexact->exact (floor (/ -5 16)))         ;; => -1
```

---

### 2.2 Rust 宿主注入函数 (Host Functions)

Rust 宿主在 Scheme 全局环境中注册了 4 个原生函数，任何脚本可直接调用：

| 宿主函数签名 | 参数说明 | 职责与效果 |
| :--- | :--- | :--- |
| `(engine:send-message pid channel text)` | `pid`: 玩家整数 ID<br>`channel`: 通道名（如 `"system"`, `"combat"`, `"chat"`）<br>`text`: 消息文本 | 向指定在线玩家推送一条独立的 `TextCraftEvent::Message` 事件帧 |
| `(engine:broadcast channel text)` | `channel`: 通道名<br>`text`: 消息文本 | 向全服所有在线玩家广播一条 `TextCraftEvent::Message` 事件帧 |
| `(engine:log text)` | `text`: 日志内容 | 输出到服务端 `tracing` 标准日志（target: `"scheme"`） |
| `(engine:random-int lo hi)` | `lo`, `hi`: 整数边界 | 生成 `[lo, hi]` 闭区间内的均匀伪随机整数 |

---

### 2.3 全局数据结构 Schema (`*world-state*`)

在 `scripts/prelude.scm` 中定义的全局状态树结构如下：

```scheme
(define *world-state*
  (hash
    "tick_count" 0          ;; 全局时钟心跳计数
    "elapsed_sec" 0.0       ;; 游戏累计运行秒数
    "target_fps" 60         ;; 当前目标帧率 (1 ~ 60)
    "idle_timer" 0.0        ;; 距离上次玩家交互的空闲秒数
    "chunks" (hash)         ;; 16x16 分块地图存储: "cx,cy" -> (hash "tiles" (hash "x,y,layer" tile))
    "players" (hash)        ;; 当前在线玩家表: pid-string -> player-hash
    "saved_players" (hash)  ;; 跨会话持久化玩家表: token-string -> player-hash
    "monsters" '()          ;; 活跃怪物列表: (list monster-hash ...)
    "double_width" #f       ;; 全局默认双宽模式开关 (#t / #f)
    "timers" (hash)))       ;; 自定义计时器字典
```

#### 标准瓦片模型 (Tile Schema)
```scheme
(hash
  "glyph" "#"              ;; 单字符字符串，如 "#", ".", "T", "C", "g"
  "fg" '(180 180 190)      ;; 前景色 24-bit RGB 列表 '(R G B)，或 #f
  "bg" '(40 40 45)         ;; 背景色 24-bit RGB 列表 '(R G B)，或 #f（自动透出下层 terrain 背景色）
  "solid" #t               ;; 是否阻挡移动 (#t 阻挡 / #f 可通行)
  "name" "stone_wall"      ;; 瓦片语义标识名
  "meta" (hash ...))       ;; 自定义扩展元数据（如宝箱掉落、NPC 对话、怪物引用等）
```

#### 标准玩家模型 (Player Schema)
```scheme
(hash
  "id" 1                   ;; 当前连接的临时 PlayerId (整数)
  "token" "alice:admin"    ;; 客户端握手原始 Token (支持 "username:auth_token" 或旧版纯 token)
  "auth_token" "admin"     ;; 解析出的权限校验 Token (用于管理员 REPL 鉴权)
  "identity_key" "alice"   ;; 角色唯一身份与 saved_players 存档归档 Key (优先取 username)
  "name" "alice"           ;; 角色显示名称 (同步至 HUD 顶部状态栏)
  "online" #t              ;; 显式在线状态位 (#t 在线 / #f 断线宽限期挂起，用于过滤渲染与怪物索敌)
  "theme" "cyberpunk"      ;; 玩家主题偏好
  "double_width" #f        ;; 玩家个人双宽字符模式开关 (#t / #f)
  "x" 0                    ;; 世界 X 坐标 (整数)
  "y" 0                    ;; 世界 Y 坐标 (整数)
  "cols" 80                ;; 视口列数 (默认 80)
  "rows" 24                ;; 视口行数 (默认 24)
  "hp" 100                 ;; 当前生命值
  "max_hp" 100             ;; 最大生命值
  "mp" 50                  ;; 当前魔法值
  "max_mp" 50              ;; 最大魔法值
  "level" 1                ;; 等级
  "gold" 50                ;; 金币
  "inventory" (list        ;; 背包物品列表
                (hash "id" "sword" "name" "铁剑" "icon" "⚔️" "count" 1)
                (hash "id" "potion" "name" "生命药水" "icon" "🧪" "count" 3))
  "dialog" #f              ;; 当前弹窗对象 (hash "title" ... "text" ... "options" ...) 或 #f
  "logs" (list ...)        ;; 玩家最近 30 条日志列表
  "last_action_tick" 0)
```

---

### 2.4 核心内置 API 参考 (`scripts/prelude.scm`)

#### 1. 地图分块读写 API
```scheme
;; 放置瓦片 (layer 可为 'terrain, 'surface, 'actor, 'overlay)
;; 内部自动计算所属 16x16 Chunk 并触发 (bump-visual-version!)
(world:set-tile x y layer tile-map)

;; 读取指定坐标与图层的瓦片 (不存在返回 #f)
(world:get-tile x y layer)

;; 移除指定坐标与图层的瓦片 (成功返回 #t，不存在返回 #f)
(world:remove-tile x y layer)

;; 清空全部地图分块与怪物列表
(world:clear!)
```

#### 2. 玩家状态与跨会话存档 API
```scheme
;; 获取在线玩家哈希表 (不存在返回 #f)
(get-player player-id)

;; 获取在线玩家，若不存在则自动初始化默认角色
(ensure-player player-id)

;; 更新玩家状态 (同时自动同步至 *world-state*["saved_players"][token] 并递增视觉版本号)
(set-player player-id player-map)

;; 移除在线玩家 (下线时调用，保留 saved_players 中的持久化存档)
(remove-player player-id)

;; 向玩家的 logs 列表追加一条日志 (自动裁剪保留最近 30 条)
(player:add-log player-id channel text)
```

#### 3. 背包与光栅化辅助 API
```scheme
;; 在背包列表中查找指定 item-id (main.scm)
(player:find-item inv item-id)

;; 给玩家增加道具 (自动合并同类堆叠数量)
(player:add-item pid item-id name icon count)

;; 消耗玩家指定数量的道具 (成功返回 #t 并写回状态，数量不足返回 #f)
(player:consume-item pid item-id count)

;; 光栅化玩家当前视口为 24-bit ANSI RLE 字符串
(render-viewport-ansi player-id cols rows)

;; 手动标记画面或 HUD 已变更，通知宿主下一帧重新生成快照
(bump-visual-version!)
```

---

### 2.5 标准生命周期钩子契约 (`scripts/main.scm`)

`scripts/main.scm` 必须定义以下函数供 Rust 宿主调用：

| 钩子函数签名 | 触发阶段 | 职责说明 |
| :--- | :--- | :--- |
| `(engine:on-init)` | 服务启动初始化 | 初始化地图（`init-demo-world`）、生成初始实体（若存在存档文件，宿主随后会自动调用 `engine:import-state` 覆盖恢复） |
| `(engine:target-fps)` | 每帧调度计算前 | 返回整数 `1 ~ 60`，控制宿主主循环节拍（活动时 `60`，空闲 >2s 降为 `30`，空闲 >5s 降为 `15`） |
| `(engine:viewport-size pid)` | 渲染快照时 | 返回 `(list cols rows)`，默认 `'(80 24)` |
| `(engine:hud-config)` | 主题配置查询 | 返回全局 HUD 默认配置字典 |
| `(engine:on-player-join pid token)` | 玩家首次认证入场 | 解析 `username:token`，清理同 `identity_key` 的旧 `pid`，从 `saved_players` 恢复或新建角色并置 `"online" #t` |
| `(engine:on-player-disconnect pid)` | 玩家断线进入 60s 宽限期 | 将玩家 `"online"` 设为 `#f`（保留内存态，立即从他人视口与怪物索敌中隐藏，不删档、不刷屏） |
| `(engine:on-player-reconnect pid)` | 宽限期内恢复连接 | 将玩家 `"online"` 设为 `#t` 并递增视觉版本号（无需重复走 `join` 或刷欢迎日志） |
| `(engine:on-player-leave pid)` | 会话宽限期超时终止 | 调用 `(remove-player pid)` 归档至 `saved_players` 并从活跃表 `"players"` 物理移除 |
| `(engine:on-tick dt tick-id)` | 主循环第 5 阶段 | 累加时钟与空闲计时器，动态调节 FPS，每 N 帧执行怪物就近索敌、X/Y 寻路与攻击结算 |
| `(engine:on-input pid input-map)` | 主循环第 3/4 阶段 | 处理键盘按键（`"type": "key"`）与视口鼠标点击（`"type": "mouse"`），重置空闲计时器 |
| `(engine:on-ui-action pid action-map)`| 主循环第 3/4 阶段 | 处理前端 HUD 发送的交互动作（如 `"use_item"`, `"buy_potion"`, `"close_dialog"`, `"clear_logs"`） |
| `(engine:on-command pid datum raw)` | 主循环第 4 阶段 | 处理玩家输入的控制台命令（`datum` 已由宿主安全 Reader 预解析为 S-表达式，零代码注入风险） |
| `(engine:render-snapshot pid)` | 主循环第 7 阶段 | 返回 `(hash "screen" ansi-str "cols" cols "rows" rows "hud" hud-map)` |
| `(engine:export-state)` | 自动存档/热重载前 | 返回 `*world-state*` |
| `(engine:import-state data)` | 启动读档/热重载后 | 将 `data` 恢复至 `*world-state*` 并调用 `(bump-visual-version!)` |

---

## 第三部分：前后端传输协议与自定义 HUD 开发指南 (`web/index.html`)

### 3.1 下行传输协议 (S2C: Server $\rightarrow$ Client)

服务端通过 WebSocket 发送两类数据包：

#### 1. 状态快照信封 (`Snapshot`)
当 `*visual-version*` 发生变更且结构化 Hash 变化（或到达 `--snapshot-heartbeat-ms` 保活周期）时推送：
```json
{
  "Snapshot": {
    "cols": 80,
    "rows": 24,
    "screen": "\u001b[38;2;70;170;70m\u001b[48;2;18;38;18m\"\"\"\"...\u001b[0m",
    "hud": {
      "theme": "cyberpunk",
      "hp": 85,
      "max_hp": 100,
      "mp": 40,
      "max_mp": 50,
      "level": 1,
      "gold": 50,
      "inventory": [
        { "id": "sword", "name": "铁剑", "icon": "⚔️", "count": 1 },
        { "id": "potion", "name": "生命药水", "icon": "🧪", "count": 3 }
      ],
      "dialog": {
        "title": "神秘商人",
        "text": "欢迎来到前哨站！",
        "options": [
          { "text": "购买生命药水 (25G)", "action": "buy_potion" },
          { "text": "离开", "action": "close_dialog" }
        ]
      },
      "logs": [
        { "channel": "system", "text": "欢迎进入 TextCraft 3.0!" }
      ]
    }
  }
}
```
- `cols` / `rows` / `screen` 由 `web/index.html` 底座自动解析并渲染至 `<pre id="screen">`。
- `hud` 为纯数据字典，结构完全由 Scheme 的 `(build-player-hud pid)` 决定，通过 `TextCraft.onHud((hud, snapshot) => { ... })` 交给前端自定义 HUD 消费。

#### 2. 离散事件信封 (`Event`)
用于瞬时事件通知（登录欢迎、全服聊天广播、管理员 `!` REPL 求值结果、权限错误）：
```json
{ "Event": { "event": "Welcome", "player_id": 1, "message": "Welcome to TextCraft 3.0!" } }
{ "Event": { "event": "Message", "channel": "chat", "text": "Hero#1 说: 你好！" } }
{ "Event": { "event": "Message", "channel": "repl", "text": "=> 30" } }
{ "Event": { "event": "Error",   "text": "Permission denied: admin token required" } }
```

---

### 3.2 上行交互协议 (C2S: Client $\rightarrow$ Server)

前端通过 `window.TextCraft` 发送的交互底层自动路由至对应通道：

| 交互分类 | 前端调用方式 | 底层 C2S JSON 报文 | 触发的 Scheme 钩子与参数 |
| :--- | :--- | :--- | :--- |
| **HUD 动作** | `TextCraft.sendAction(action, payload)` | `{"Command":{"Input":{"type":"ui","action":"...","payload":{...}}}}` | `(engine:on-ui-action pid action-map)`<br>注：`payload` 若为对象，其顶层字段会自动拍平合并至 `action-map` 根部 |
| **键盘按键** | `TextCraft.sendKey(code, mods)` | `{"Command":{"Input":{"type":"key","code":"KeyW","shift":false,...}}}` | `(engine:on-input pid input-map)` |
| **鼠标输入** | `TextCraft.sendMouse(x, y, button)` | `{"Input":{"type":"mouse","x":12,"y":8,"button":"left"}}` | `(engine:on-input pid input-map)` |
| **指令/REPL**| `TextCraft.sendCmd(input)` | `{"Command":{"Input":{"type":"cmd","input":"(help)"}}}` | 普通指令 $\rightarrow$ `(engine:on-command pid datum raw)`<br>`!` 开头指令 $\rightarrow$ 管理员 `eval_repl` |

---

### 3.3 前端 `window.TextCraft` SDK 接口说明

`web/index.html` 已去除所有冗余主题与复杂 CSS，仅保留约 60 行极简暗色布局样式，并在全局暴露 `window.TextCraft`：

```javascript
// 1. 订阅快照中的 HUD 数据更新
TextCraft.onHud((hud, snapshot) => {
  // 在这里自由更新你的自定义 DOM 组件
});

// 2. 订阅服务端离散事件 (Welcome / Message / Error)
TextCraft.onEvent((ev) => {
  // 处理瞬时消息或弹窗
});

// 3. 发送自定义 HUD 交互动作 (内部会自动调用 blurFocus() 防止按钮抢走 WASD 焦点)
TextCraft.sendAction('craft_item', { recipe: 'iron_sword', count: 1 });

// 4. 发送控制台指令或管理员 REPL 表达式
TextCraft.sendCmd('(where)');
TextCraft.sendCmd('! (+ 100 200)');

// 5. 发送键盘或鼠标视口坐标事件
TextCraft.sendKey('KeyW', { shift: false, ctrl: false, alt: false });
TextCraft.sendMouse(10, 5, 'left');
```

> [!TIP]
> **前端自定义 HUD 的焦点守卫原则**：
> 在网页游戏中，点击 `<button>` 会使按钮获得浏览器焦点，导致后续按 `Space` 或 `Enter` 重复触发按钮点击而非游戏操作。使用 `TextCraft.sendAction(...)` 时会自动调用 `TextCraft.blurFocus()` 释放非输入框元素的焦点；如果你编写了不发包的纯前端折叠按钮，也请在点击回调末尾调用 `TextCraft.blurFocus()`。

---

## 第四部分：全栈功能扩展实战配方 (Full-Stack Recipes)

### 配方 1：自定义一套全新的 HUD 面板与交互（如“合成制造台”）

假设你要为游戏增加一个“木材/矿石资源显示 + 合成铁剑按钮”：

#### 第一步：在 Scheme (`scripts/prelude.scm`) 的 `build-player-hud` 中下发新字段
```scheme
(define (build-player-hud player-id)
  (let ((p (ensure-player player-id)))
    (hash
      "hp" (map-get p "hp" 100)
      "max_hp" (map-get p "max_hp" 100)
      "gold" (map-get p "gold" 0)
      ;; 新增自定义资源字段
      "wood" (map-get p "wood" 10)
      "ore"  (map-get p "ore" 5)
      "inventory" (map-get p "inventory" '())
      "dialog" (map-get p "dialog" #f)
      "logs" (map-get p "logs" '()))))
```

#### 第二步：在 Scheme (`scripts/main.scm`) 的 `engine:on-ui-action` 中处理合成动作
```scheme
;; 在 engine:on-ui-action 的 cond 中添加分支：
((equal? action "craft_sword")
 (let ((wood (map-get p "wood" 0))
       (ore  (map-get p "ore" 0)))
   (if (and (>= wood 2) (>= ore 3))
       (let ((p2 (hash-insert (hash-insert p "wood" (- wood 2)) "ore" (- ore 3))))
         (set-player pid p2)
         (player:add-item pid "sword" "精钢长剑" "🗡️" 1)
         (player:add-log pid "system" "🔨 消耗木材x2、矿石x3，成功锻造出【精钢长剑】！"))
       (player:add-log pid "system" "材料不足！需要木材x2、矿石x3。"))))
```

#### 第三步：在前端 (`web/index.html`) 中添加 HTML 元素并绑定 `TextCraft`
```html
<!-- 在 #hud-overlay 中添加自定义面板 -->
<div class="panel" id="craft-panel" style="position:absolute;left:10px;top:56px;">
  <div>🪵 木材: <span id="res-wood">0</span> | 🪨 矿石: <span id="res-ore">0</span></div>
  <button id="btn-craft-sword" style="margin-top:6px;">🔨 锻造精钢剑 (2木 3矿)</button>
</div>

<script>
  // 订阅数据刷新
  TextCraft.onHud((hud) => {
    if (hud.wood !== undefined) document.getElementById('res-wood').textContent = hud.wood;
    if (hud.ore !== undefined)  document.getElementById('res-ore').textContent = hud.ore;
  });

  // 绑定点击动作
  document.getElementById('btn-craft-sword').addEventListener('click', () => {
    TextCraft.sendAction('craft_sword', {});
  });
</script>
```

---

### 配方 2：新增一种带环境伤害的地形瓦片（如：岩浆 Lava）

```scheme
;; 1. 在地图上放置岩浆瓦片 (scripts/main.scm)
(world:set-tile 3 3 'terrain
  (hash "glyph" "~"
        "fg" '(255 120 20)
        "bg" '(120 20 10)
        "solid" #f
        "name" "lava"))

;; 2. 在 engine:on-input 的可通行移动分支 (Walkable tile) 中追加结算：
(let ((cur-terrain (world:get-tile nx ny 'terrain)))
  (if (and cur-terrain (equal? (map-get cur-terrain "name" "") "lava"))
      (let* ((p-now (ensure-player pid))
             (cur-hp (map-get p-now "hp" 100))
             (burned-hp (max 0 (- cur-hp 10))))
        (set-player pid (hash-insert p-now "hp" burned-hp))
        (player:add-log pid "combat" "🔥 炽热的岩浆灼伤了你，生命值 -10！"))
      #f))
```

---

### 配方 3：热重载与管理员实时 REPL 调试技巧

1. **保存即热重载（`--watch` / `-w` 开启，零重启）**：
   开发调试时通过 `cargo run -- --watch --log-level debug`（或 `-w`）启动服务端，宿主会每 2 秒轻量检查一次 `scripts/` 根目录下的 `.scm` 修改时间（未开启 `--watch` 时默认零开销短路）。修改 `scripts/prelude.scm` 或 `scripts/main.scm` 保存后，宿主会自动调用 `(engine:export-state)` 备份当前所有分块与玩家数据，重编译脚本后再通过 `(engine:import-state)` 无缝恢复（若新脚本存在语法错误，宿主会自动回滚状态并保留旧环境运行，不会崩溃）。
2. **管理员 `!` REPL 在线求值与多用户 URL 路由**：
   前端支持 `#/username` 与 `#/username/token` 路由（例如 `http://127.0.0.1:8081/#/alice` 或 `http://127.0.0.1:8081/#/bob/admin`，也兼容 `?user=bob&token=admin` 或 `?token=admin`）。当 URL 显式指定身份时不会覆盖 `localStorage`，方便单浏览器多开标签页联机调试。携带管理员 Token（默认 `admin`）后，在底部命令框输入 `!` 开头的任意 Scheme 代码即可直接操作线上世界：
   - `! (get-player 1)` $\rightarrow$ 查看玩家 1 的完整 Hash 数据
   - `! (set-player 1 (hash-insert (ensure-player 1) "gold" 9999))` $\rightarrow$ 在线修改玩家金币
   - `! (world:set-tile 1 0 'surface (hash "glyph" "$" "fg" '(255 215 0) "bg" #f "solid" #f "name" "gold_coins" "meta" (hash "amount" 500)))` $\rightarrow$ 在 `(1, 0)` 坐标动态刷出一堆 500 金币
