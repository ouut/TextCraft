# TextCraft 版本演进与架构变更日志 (Changelog)

本文档记录 TextCraft 从 `v3.0.0` 到 `v3.1.0` 的核心架构演进、全量缺陷修复、性能优化及开发者体验升级。

---

## [v3.1.0] - 2026-11-02

### 1. CLI 参数重构、2s 热重载按需开启与五级日志规范
- **显式 `--watch` (`-w`) 脚本热重载**：
  - 移除旧版反向参数 `--no-watch-scripts`，新增符合直觉的 `-w, --watch` 参数（默认 `false`）。
  - 未传入 `--watch` 时，主循环 `tick()` Stage 1 直接通过 `self.logic.watch_enabled()` 布尔短路，实现生产环境 **0 磁盘 I/O 开销**。
  - 开启 `--watch` 时，热重载文件修改时间轮询间隔由 500ms 调整为 **2 秒 (`Duration::from_secs(2)`)**，兼顾开发实时反馈与极低系统调用开销。
- **新增 `--log-level` 参数与五级日志体系 (`LogLevel`)**：
  - 支持 `--log-level <error|warn|info|debug|trace>`（默认 `info`），若环境中显式设置了 `RUST_LOG` 则优先遵循 `RUST_LOG`。
  - 严格规范五级日志职责边界：
    - `error`：主循环 `tick` 致命执行异常、自动存档写盘失败。
    - `warn`：Scheme 钩子执行报错/看门狗超时拦截、热重载脚本编译语法错误及状态回滚、存档读取失败回退、非管理员越权请求 `!` REPL 被拒。
    - `info`：服务启动监听摘要（`addr`、`watch`、`log_level`）、`Ctrl+C` 优雅停机、脚本热重载成功、玩家四态连接生命周期事件（`PlayerJoined`、`PlayerDisconnected`、`PlayerReconnected`、`PlayerTerminated`）、Scheme 脚本调用 `(engine:log ...)`。
    - `debug`：原子存档落盘成功（避免每 30 秒在 `info` 级别刷屏）、启动读档恢复成功、收到玩家控制台指令（`Cmd`）与管理员 `AdminEval` 执行。
    - `trace`：高频按键/鼠标/UI 动作分发、快照发送与脏版本/Hash 去重跳过详情、每 2 秒一次的 `check_reload` 目录扫描检查。

### 2. 多人会话四态生命周期与 URL 路由 (`#/username/token`)
- **前端多用户与管理员 URL 路由 (`web/index.html`)**：
  - 实现 `resolveClientIdentity()`，支持 `#/username`、`#/username/token`（推荐 Hash 路由）、`?/username/token`、`?user=username&token=token`、`/username/token` 以及旧版 `?token=xxx`。
  - **多开标签页隔离**：URL 未指定身份时默认读写 `localStorage`（`textcraft_user` / `textcraft_token`）；当 URL 显式指定身份时**不覆盖** `localStorage`，支持单浏览器同时打开 `#/alice`、`#/bob`、`#/admin_user/admin` 进行多角色联机调试。
  - 支持 `hashchange` 事件监听，地址栏修改 `#/alice` $\rightarrow$ `#/bob` 时自动切换身份并重连 WebSocket；顶部状态栏 `#player-title` 实时展示当前角色名。
- **Scheme 四态会话生命周期与纯 $O(1)$ 状态更新 (`scripts/prelude.scm` & `scripts/main.scm`)**：
  - 新增 `parse-identity-token` 拆分 `username:auth_token`，优先以 `username` 作为 `"identity_key"` 索引 `"saved_players"` 跨会话存档。
  - 引入显式 `"online"` 布尔状态位，对齐四态生命周期钩子：
    1. `(engine:on-player-join pid token)`：仅在入场时执行一次同 `identity_key` 的旧 `pid` 顶号清理，恢复存档或创建默认角色，置 `"online" #t`。
    2. `(engine:on-player-disconnect pid)`：断线进入 60s 宽限期时将 `"online"` 设为 `#f`（保留 `"players"` 内存态但从他人视口隐藏，不删档、不刷日志）。
    3. `(engine:on-player-reconnect pid)`：宽限期内重连时将 `"online"` 恢复为 `#t` 并递增视觉版本号（不重复追加欢迎日志）。
    4. `(engine:on-player-leave pid)`：宽限期超时终止时归档至 `"saved_players"` 并从 `"players"` 物理移除。
  - 移除 `set-player` 内部的线性遍历去重循环，使每次移动/交互的 `set-player` 恢复为纯粹的 **$O(1)$ 哈希表更新**。
  - **纯只读渲染与离线过滤**：`build-player-hud` 改为调用只读 `(get-player pid)`（杜绝渲染阶段调用 `ensure-player` 误创幽灵记录）；`render-viewport-ansi`、怪物 AI 索敌与 `(say ...)` 广播均严格过滤 `"online" = #t` 的在线玩家。
- **Rust 宿主组合 Token 管理员鉴权 (`src/runtime.rs`)**：
  - `TextCraftServer::is_admin_authorized` 同时兼容旧版完整匹配（`raw_token == admin_token`）与 `username:auth_token` 组合格式（提取 `:` 后的 `auth_token` 校验，`# /alice` 留空 Token 时无管理员权限）。

### 3. 极简前端底座与 `window.TextCraft` 开发者 SDK
- **精简 `web/index.html` 与剥离冗余样式**：
  - 移除冗余的多主题 CSS 代码，保留约 60 行干净的暗色毛玻璃 HUD 基础布局与自适应等宽字号 `<pre id="screen">` 核心视口。
  - 暴露全局 `window.TextCraft` SDK（`sendAction`、`sendCmd`、`sendKey`、`sendMouse`、`onHud`、`onEvent`、`blurFocus`、`parseAnsi`、`resolveClientIdentity`），支持开发者与 Agent 通过纯 JSON 字典零门槛扩展自定义 HUD 组件。
- **新增综合开发文档**：
  - 新增 `docs/agent_developer_guide.md`，涵盖核心心智模型、Steel Scheme 速查、内置 API、前后端传输协议契约与全栈扩展实战配方。

### 4. 核心安全修复、主循环极速优化与 ANSI RLE 压缩
- **消除代码注入风险**：`SchemeLogic::on_command` 改用受看门狗保护的纯 Scheme Reader (`__engine:parse-datum`) 解析 S-表达式，彻底杜绝恶意闭合括号注入死循环。
- **主循环脏版本号短路与结构化 Hash**：
  - 引入 `*visual-version*` 计数器与 `hash_json_value` 结构化哈希（消除每帧 `hud.to_string()` 堆分配）。
  - 10 名在线玩家静止帧 `tick(0.016)` 平均耗时降至 **`< 0.05ms`**。
- **ANSI 同行颜色状态合并 (RLE)**：
  - `render-viewport-ansi` 预索引在线玩家坐标并按 16 列缓存 Chunk 查找，同行连续相同前景/背景色自动合并 ANSI 转义序列，单帧 80×24 视口字符串体积从 ~75KB 压缩至 **~3.4KB**（减少 95%），ANSI Reset 数量从 1920 次降至 ~115 次。

---

## [v3.0.0] - 2026-10-25

- 初始发布 TextCraft 3.0 纯 Scheme 驱动混合 UI（Hybrid UI）引擎架构：
  - 零 Rust 业务结构体（移除硬编码网格坐标与地图结构，完全由 Steel Scheme `*world-state*` 驱动）。
  - 16×16 分块世界存储（Chunked World Storage）与四层瓦片模型（`terrain` / `surface` / `actor` / `overlay`）。
  - 单二进制内嵌 Web 终端与自适应动态帧率调度（1 ~ 60 FPS）。
