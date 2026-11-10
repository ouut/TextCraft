# Steel Scheme 代码开发最佳实践：从入门到“快到起飞”

> 本文档由 TextCraft 团队结合现代 Lisp 哲学、SICP 精髓以及高并发游戏引擎的实战经验总结而成。  
> 旨在帮助开发者与 AI Agent 彻底摆脱命令式思维枷锁，消除工具摩擦，掌握“心智负担最低、开发效率极致”的 Steel Scheme 编程之道。

---

## 目录
- [一、 编辑器革命：消除括号的心智负担](#一-编辑器革命消除括号的心智负担)
- [二、 极致的 REPL 驱动与实时调试 (Live Coding)](#二-极致的-repl-驱动与实时调试-live-coding)
- [三、 数据驱动优先 (Data-Driven Programming)](#三-数据驱动优先-data-driven-programming)
- [四、 流水线穿透：消灭“洋葱式嵌套” (Threading Macros)](#四-流水线穿透消灭洋葱式嵌套-threading-macros)
- [五、 函数式与状态哲学：全局纯净，局部高效](#五-函数式与状态哲学全局纯净局部高效)
- [六、 宏的克制与边界 (First Rule of Macro Club)](#六-宏的克制与边界-first-rule-of-macro-club)
- [七、 模块化、导入与热重载工程规范](#七-模块化导入与热重载工程规范)
- [八、 刻意练习与心智跃迁路线](#八-刻意练习与心智跃迁路线)

---

## 一、 编辑器革命：消除括号的心智负担

新手觉得 Scheme 难，核心痛点往往在于“数括号、对齐括号”消耗了过多的工作记忆。**高阶 Lisper 从不肉眼数括号，他们操作的是语法树（AST）。**

### 1. 核心工具链推荐
- **VS Code**：推荐安装 **Parinfer** 或 **Paredit** 扩展（重点开启 `smart` 模式）。
- **Emacs / Doom Emacs**：配置 `paredit`、`smartparens` 或 `lispy`。
- **JetBrains (IDEA / CLion)**：安装 **Rainbow Brackets** 与 **Parinfer** 插件。

### 2. 实践准则
1. **拥抱 Parinfer（缩进即结构）**：
   - 在 `smart` 模式下，括号的闭合完全根据你的代码缩进自动推断并补齐。写 Scheme 的手感瞬间变得像 Python 一样行云流水，开发者只需关心缩进和语义，再也不用手动输入 `))))`。
2. **掌握结构化编辑（S-Expression Editing）**：
   - 彻底戒掉用退格键单个删括号的习惯。
   - 掌握结构化操作手势：
     - **Slurp（吸入）**：将右侧或外侧的表达式吸入当前括号。
     - **Barf（吐出）**：将括号内最后一个元素推出当前括号。
     - **Wrap / Unwrap**：用括号快速包裹某个表达式，或剥离最外层括号。
     - **Transpose**：调换两个相邻 S-Expression 的位置。

---

## 二、 极致的 REPL 驱动与实时调试 (Live Coding)

Scheme 的灵魂是 **REPL（Read-Eval-Print Loop）**。如果你还在“写完代码 -> 保存 -> 终端重启进程 -> 复现场景 -> 验证”的死循环中，开发效率将被严重拖慢。

### 1. 自底向上编程 (Bottom-Up Development)
在传统命令式开发中，通常先搭建庞大的框架与接口。在 Scheme 中，推荐**搭积木式的自底向上开发**：
1. 先在 REPL 里调通最底层的一个原子变换函数（如计算单格曼哈顿距离、扣减血量边界）；
2. 立即在 REPL 传入测试数据检验返回值；
3. 将经过验证的原子函数组合成顶层业务逻辑；
4. 整个过程中，服务器从未停机，业务状态完好保留。

### 2. TextCraft 在线 REPL 实战技巧
TextCraft 提供了开箱即用的 REPL 调试通道（dev 模式；`--eval` 评测模式下 `!` REPL 被门禁，见 `mcp_developer_guide.md` 第五节）：
- **Web 终端前端 `!` 快捷 REPL**：
  访问 `http://127.0.0.1:8081/#/admin_user/admin`，在底部命令框中以 `!` 开头直接输入任意 Scheme 表达式：
  ```scheme
  ! (+ 100 200)
  ! (get-player 1)
  ! (set-player 1 (hash-insert (ensure-player 1) "gold" 9999))
  ```
  输入即求值，执行结果毫秒级回显到聊天日志栏中。
- **通过 MCP 接口调用 REPL**：
  在 AI Agent 辅助编程时，可通过 HTTP JSON-RPC 接口直接调用 `eval_repl`，动态检验世界状态与函数行为。

---

## 三、 数据驱动优先 (Data-Driven Programming)

在面向对象（OOP）世界中，人们倾向于把“数据”与“行为”绑定为 Class/Object；而在高级 Scheme 开发中，**最卓越的抽象是将一切逻辑编码为纯数据（Plain Data Trees）**。

### 1. 实体与世界状态的纯数据化
不要用复杂的对象系统或嵌套闭包来表示游戏实体，直接使用不可变哈希字典（`hash`）与列表（`list`）：

```scheme
;; 推荐：透明、可穿透、可直接 JSON 序列化的纯数据
(define (make-card id name icon type cost damage)
  (hash "id" id
        "name" name
        "icon" icon
        "type" type
        "cost" cost
        "damage" damage))

;; 一个具有增益效果的怪物定义
(hash "name" "熔岩巨兽"
      "hp" 120
      "defense" 15
      "tags" '("fire" "elite")
      "drops" (list (hash "item" "fire_gem" "chance" 0.35)))
```

### 2. 数据驱动的巨大红利
- **零成本跨语言传输**：纯数据哈希字典可以直接通过 WebSocket 序列化推给前端 Web 客户端（`hud` 字段），前端直接拿原生 JS Object 使用。
- **磁盘存档零转换**：在 TextCraft 中，模型状态以纯数据存放在 ECS 的 WorldState 单例里，`(engine:export-state)` 直接返回 `(state:all)`；宿主存档（快照 v3）把单例随 ECS 世界一并序列化，读档直接恢复，不需要任何繁琐的 ORM 或 DTO 映射。
- **热重载永不崩溃**：纯数据中没有任何无法序列化的 Lambda 闭包指针，热重载时代码逻辑完全更新，而世界状态无缝保留。

---

## 四、 流水线穿透：消灭“洋葱式嵌套” (Threading Macros)

传统 Lisp 广受诟病的一点是“从内向外”的洋葱式嵌套，阅读长链条调用时眼睛需要在多层括号间来回跳跃。  
在 Steel Scheme 中，内置了强劲的**穿透宏（Threading Macros）**，让数据像工业传送带一样自顶向下清晰流动。

### 1. 线程首宏：`->` (Thread-First)
将上一步的求值结果，自动注入为下一个表达式的**第一个参数**。极其适合操作哈希表、字典与关联结构：

```scheme
;; 传统洋葱式写法（难以一眼看出处理流向）：
(hash-insert (hash-insert (ensure-player pid) "online" #t) "gold" (+ (get-gold pid) 50))

;; 使用 -> 穿透宏（自上而下，清爽无比）：
(-> (ensure-player pid)
    (hash-insert "online" #t)
    (hash-insert "gold" (+ (get-gold pid) 50))
    (hash-insert "hp" 100))
```

### 2. 线程尾宏：`->>` (Thread-Last)
将上一步的求值结果，自动注入为下一个表达式的**最后一个参数**。极其适合操作列表、向量和序列变换：

```scheme
;; 过滤所有血量 > 0 的怪物，按攻击力降序排序，取前 3 只的名字
(->> (get-monsters)
     (filter (lambda (m) (> (map-get m "hp" 0) 0)))
     (sort-by-attack)
     (take 3)
     (map (lambda (m) (map-get m "name" ""))))
```

> **心法**：操作单个实体/Map 时用 `->`；操作集合/List 时用 `->>`。心智负担瞬间降低一半以上！

---

## 五、 函数式与状态哲学：全局纯净，局部高效

在函数式编程界，“严禁使用任何 `set!`”是一种极端的教条主义。真正的工程级最佳实践是：**“外部纯净无副作用，内部局部快如闪电”**。

### 1. 全局状态：单一权威 + 值不可变（Single Authority, Immutable Values）
- 全局状态只有一个家：Flecs ECS。实体性状态用 `ecs:*` 组件/关系，玩家表、地图分块等键值状态存 WorldState 单例、一律经 `state:get` / `state:set!` 门面读写——**不要再定义自己的全局状态变量**，那会绕开快照与状态哈希、制造第二份权威（旧的 `*world-state*` 已因此被消灭）。
- 门面里存的**值**仍坚持用持久化不可变操作构造（如 `hash-insert`, `hash-remove` 先算出新 hash 再 `state:set!` 写回），避免原地改值带来的别名与回滚问题；同时注意 char/symbol 经门面往返会降为 string，闭包不可序列化、禁止入内。
- 任何影响画面的全局状态改变都应当是显式版本化的（例如调用 `(bump-visual-version!)`），这能让 Rust 宿主以 `< 0.05ms` 的微秒级开销完成视口光栅化去重。

### 2. 局部算法：务实利用局部可变性与尾递归
- **尾递归（TCO）**：当编写状态机演化、图遍历时，写成标准尾递归形式，Steel 编译器会直接优化为机器级无堆栈溢出的循环跳转：
  ```scheme
  (define (sum-range n acc)
    (if (<= n 0)
        acc
        (sum-range (- n 1) (+ acc n)))) ;; 尾递归调用，零栈开销
  ```
- **算法内部允许局部突变**：在编写复杂的局部排序、A* 寻路标记 `visited`、构建临时的 16x16 字符缓冲区时，使用局部向量与局部突变（`let` 内局部的 `vector-set!` 或临时变量更新）能够获得 $O(1)$ 的原生性能，同时因为变量生命周期限制在局部闭包内，对外部世界没有任何副作用污染。

### 3. 高性能流变换：Transducers（传感器）
在每秒 60 帧的高频逻辑中，如果链式调用 `(map ... (filter ... (take ... lst)))`，会产生多轮中间列表分配。Steel 内置了零分配管道 **Transducers**：
```scheme
(require-builtin steel/transducers)

;; 将 mapping 与 filtering 熔合成单一遍历管道，直接倾倒进目标容器
(into-list (compose (filtering odd?) (mapping (lambda (x) (* x 2))))
           '(1 2 3 4 5 6 7 8))
;; => '(2 6 10 14)
```

---

## 六、 宏的克制与边界 (First Rule of Macro Club)

Lisp 拥有世界上最强大的元编程能力，但新手最容易犯的错误是**“逢啰嗦必写宏”**，最终产出一堆无法阅读、无法调试、丢失错误行号的“私有天书”。

### 1. 宏俱乐部第一定律
> **First Rule of Macro Club: Don't Write Macros!**  
> 如果一个功能可以通过普通函数或高阶函数（Higher-Order Function）优雅实现，**绝不要写宏**。

### 2. 什么时候才必须写宏？
只有当你需要解决普通函数**由于预先求值（Eager Evaluation）无法达成**的场景时，才写宏：
1. **改变代码执行时机 / 延迟求值**：例如实现 `(when condition body...)`、`(while ...)`，如果用函数，无论条件是否成立参数都会被提前计算。
2. **改变变量绑定作用域 / 语法糖结构**：例如实现 `(with-player-context pid body...)`，自动注入 `player` 变量到作用域内。
3. **消除重复且无法用高阶函数折叠的控制流**：例如异常保护封装、自定义 DSL 声明式 DSL。

### 3. 宏编写规范
- 优先使用卫语句宏 `define-syntax` 与 `syntax-rules`，它具有卫生宏（Hygienic Macro）特性，绝不意外污染外部局部变量。
- 在编写复杂宏之前，务必先在测试中进行宏展开（`macroexpand`）验证输出形态。

---

## 七、 模块化、导入与热重载工程规范

在多脚本协作的工程中，保持清晰的模块导入边界能让项目长期健壮可维护。

### 1. 模块导入三大准则
```scheme
;; 1. 引入 Rust 宿主编译进二进制的底层模块（原语加速库）
(require-builtin steel/time)
(require-builtin steel/filesystem as fs.)
(require-builtin steel/tcp as tcp.)

;; 2. 引入 Steel 标准库随附的扩展模块
(require "steel/result")
(require "steel/option")

;; 3. 引入游戏工程内部的相对路径模块（使用 provide 明确导出公有符号）
(require "prelude.scm")
(require "task1_input.scm")
```

### 2. 模块导出规范：只导出接口，隐藏内部实现
在自定义模块文件（如 `task1_input.scm`）底部，使用 `provide` 显式列出对外的公共接口，避免内部辅助函数污染主命名空间：
```scheme
;; task1_input.scm
(define (helper-parse-cmd raw) ...)  ;; 内部函数，不导出
(define (handle-player-move pid dir) ...) ;; 外部接口

(provide handle-player-move)
```

### 3. 确定性规范：评测期可用的设施边界（TextCraft 项目约束）
- 游戏逻辑的随机一律走宿主 `(engine:random-int ...)`（xoshiro256++，状态进快照）；时间语义用模型自维护的 `tick_count`，不要读墙钟。
- `steel/time`、`steel/random`、`steel/meta`、`steel/filesystem`、`steel/ports`、`steel/process` 在 `--eval` / `run_epoch` 期间会被宿主替换为报错桩（含 `(require-builtin ...)` 重新引入的路径）。把它们用于调试可以，但**不要让玩法逻辑依赖它们**，否则评测时直接报错、且 dev 下的运行结果不可复现。

---

## 八、 刻意练习与心智跃迁路线

要实现从命令式到“心智负担最低”的跃迁，建议按以下三步进行针对性刻意练习：

1. **第一阶段：语法脱敏与结构化肌肉记忆（1~3 天）**
   - 在编辑器中配置好 **Parinfer**。
   - 找 10 道简单的 LeetCode / 数组操作题目，强迫自己**不用任何 `for`/`while` 循环**，全用递归或 `map`、`filter`、`foldl` 实现。熟悉列表递归拆解 `(car / cdr)` 的节奏。
2. **第二阶段：数据流与穿透管道内化（1 周）**
   - 全面改用 `->` 和 `->>` 重写复杂的多层调用。
   - 练习用纯 Map（哈希表）表达复杂业务实体，感受“数据即代码”的松耦合魅力。
3. **第三阶段：阅读经典与自制微型求值器（进阶）**
   - 精读《计算机程序的构造和解释》（SICP）前三章。
   - 亲手用 Scheme 实现一个元循环求值器（Metacircular Evaluator）。当你真正理解解释器如何通过 `eval` 和 `apply` 互相递归驱动整个程序世界时，任何复杂的业务系统对你而言都是降维打击！
