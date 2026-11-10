# Steel Scheme 0.8.3 内置标准库与核心函数参考手册

> **文档来源**：基于 [steel-core 0.8.3](https://github.com/mattwparas/steel) 源码全量解析整理。

> **运行环境**：TextCraft（当前分支 feat/simulator-upgrade，crate 5.0.0）嵌入的 Steel VM 运行时，steel-core 版本与 `Cargo.toml` 锁定一致（0.8.3）。涵盖所有全局可用的原语（Primitives）、标准库宏、内置语法与标准模块。
>
> **项目绑定提示（非语言内容）**：本项目宿主另行注册了 `ecs:*` / `state:*` / `engine:*` 原语（不在本手册范围，见 `scheme_developer_guide.md` 第 2 节）。另注意评测模式（`--eval` / `run_epoch`）下，宿主会把 `steel/time`、`steel/random`、`steel/meta`、`steel/filesystem`、`steel/ports`、`steel/process` 中的非确定性设施替换为报错桩（含模块注册表），dev 模式不受影响（见 `mcp_developer_guide.md` 第五节）；游戏逻辑请勿依赖这些设施，随机用 `(engine:random-int ...)`、时间用模型 `tick_count`。


## 模块导航目录

- [数值计算与数学库 (Numbers & Math)](#数值计算与数学库-numbers--math)
- [字符串与字符库 (Strings & Chars)](#字符串与字符库-strings--chars)
- [列表与对子库 (Lists & Pairs)](#列表与对子库-lists--pairs)
- [向量库 (Vectors & Immutable Vectors)](#向量库-vectors--immutable-vectors)
- [哈希表库 (Hash Maps)](#哈希表库-hash-maps)
- [哈希集合库 (Hash Sets)](#哈希集合库-hash-sets)
- [字节向量库 (Bytevectors)](#字节向量库-bytevectors)
- [符号库 (Symbols)](#符号库-symbols)
- [端口与流式 I/O (Ports & I/O)](#端口与流式-io-ports--io)
- [文件系统与路径 (Filesystem & Path)](#文件系统与路径-filesystem--path)
- [时间与基准 (Time & Duration)](#时间与基准-time--duration)
- [传感器与流式处理 (Transducers & Streams)](#传感器与流式处理-transducers--streams)
- [网络 TCP 套接字 (Networking TCP)](#网络-tcp-套接字-networking-tcp)
- [HTTP 客户端与请求 (HTTP Client)](#http-客户端与请求-http-client)
- [子进程与系统命令 (Process & Commands)](#子进程与系统命令-process--commands)
- [事件循环与异步轮询 (Async Polling)](#事件循环与异步轮询-async-polling)
- [加密与哈希算法 (Cryptographic Hashes)](#加密与哈希算法-cryptographic-hashes)
- [核心控制流与标准宏 (Syntax & Special Forms)](#核心控制流与标准宏-syntax--special-forms)
- [Steel 常用扩展模块 (Modules: Result, Option, Serde 等)](#steel-常用扩展模块-modules-result-option-serde-等)
- [模块引用规范与最佳实践 (Require Syntax Guide)](#模块引用规范与最佳实践-require-syntax-guide)

---

## 数值计算与数学库 (Numbers & Math)

提供整型、浮点型、有理数、位运算、指数三角函数与数值断言。在 Scheme 中默认全局可用。

### `(abs ...)`

- **Rust 底层绑定**: `abs`
- **函数签名参数**: `(number: &SteelVal)`

Computes the absolute value of the given number.

(abs number) -> number?

* number : number? - The number to compute the absolute value of.

# Examples
```scheme
> (abs 42) ;; => 42
> (abs -42) ;; => 42
> (abs 0) ;; => 0
```


### `(acos ...)`

- **Rust 底层绑定**: `acos`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the arccosine, or inverse cosine, of a value; output is in radians.

(acos n) -> number?

* n : number? - The input value is the cosine of the angle you want and must be from -1 to 1.

# Examples
```scheme
> (acos -1) ;; => 3.141592653589793
> (acos 0) ;; => 1.5707963267948966
> (acos 0.5) ;; => 1.0471975511965976
> (acos 2) ;; => +nan.0
```


### `(angle ...)`

- **Rust 底层绑定**: `angle`
- **函数签名参数**: `(number: &SteelVal)`

Computes the angle `θ` of a complex number `z` where `z = r * (cos θ + i sin θ)` and `r` is the magnitude.

(angle number) -> number?

- number : number?


### `(asin ...)`

- **Rust 底层绑定**: `asin`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the arcsine, or inverse sine, of a value; output is in radians.

(asin n) -> number?

* n : number? - The input value is the sine of the angle you want and must be from -1 to 1.

# Examples
```scheme
> (asin -1) ;; => -1.5707963267948966
> (asin 0) ;; => 0
> (asin 0.5) ;; => 0.5235987755982988
> (asin 2) ;; => +nan.0
```


### `(atan ...)`

- **Rust 底层绑定**: `atan`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the arctangent, or inverse tangent, of a value; output is in radians.

(atan n) -> number?

* n : number? - The input value is the tangent of the angle you want.

# Examples
```scheme
> (atan -1) ;; => -0.7853981633974483
> (atan 0) ;; => 0
> (atan 0.5) ;; => 0.46364760900080615
> (atan 2) ;; => 1.1071487177940906
```


### `(ceiling ...)`

- **Rust 底层绑定**: `ceiling`
- **函数签名参数**: `(number: &SteelVal)`

Rounds the given number up to the nearest integer not less than it.

(ceiling number) -> integer?

* number : real? - The number to round up.

# Examples
```scheme
> (ceiling 42) ;; => 42
> (ceiling 42.1) ;; => 43
> (ceiling -42.1) ;; => -42
```


### `(complex? ...)`

- **Rust 底层绑定**: `complexp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is a complex number

(complex? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (complex? 3+4i) ;; => #t
> (complex? 42) ;; => #t
> (complex? "hello") ;; => #f
```


### `(cos ...)`

- **Rust 底层绑定**: `cos`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the cosine value of the input angle, measured in radians.

(cos n) -> number?

* n : number? - The input angle, in radians.

# Examples
```scheme
> (cos 0) ;; => 1
> (cos 1) ;; => 0.5403023058681398
> (cos 2.0) ;; => -0.4161468365471424
> (cos 3.14) ;; => -0.9999987317275395
```


### `(denominator ...)`

- **Rust 底层绑定**: `denominator`
- **函数签名参数**: `(number: &SteelVal)`

Retrieves the denominator of the given rational number.

(denominator number) -> integer?

* number : number? - The rational number to retrieve the denominator from.

# Examples
```scheme
> (denominator 1/2) ;; => 2
> (denominator 3/4) ;; => 4
> (denominator 4) ;; => 1
```


### `(even? ...)`

- **Rust 底层绑定**: `even`
- **函数签名参数**: `(arg: &SteelVal)`

Checks if the given number is even

(even? n) -> bool?

* n : number? - The number to check for evenness.

# Examples
```scheme
> (even? 2) ;; => #true
> (even? 3) ;; => #false
> (even? 4.0) ;; => #true
```


### `(exact ...)`

- **Rust 底层绑定**: `exact`
- **函数签名参数**: `(number: &SteelVal)`

Converts a number to an exact number.

(exact num) -> number?

* num : number? - The value to convert to exact.

# Examples
```scheme
> (exact 10.0) ;; => 10
> (exact 1.5) ;; => 3/2
> (exact 1.5+2.5i) ;; => 3/2+5/2i
```


### `(exact-integer-sqrt ...)`

- **Rust 底层绑定**: `exact_integer_sqrt`
- **函数签名参数**: `(number: &SteelVal)`

Computes the integer square root of the given non-negative integer.

(exact-integer-sqrt number) -> (integer? integer?)

* number : (and/c integer? positive?) - The non-negative integer to compute the square root for.

# Examples
```scheme
> (exact-integer-sqrt 25) ;; => (5 0)
> (exact-integer-sqrt 35) ;; => (5 10)
```


### `(exact-integer? ...)`

- **Rust 底层绑定**: `exact_integerp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is an exact integer

(exact-integer? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (exact-integer? 42) ;; => #t
> (exact-integer? -42) ;; => #t
> (exact-integer? 4.0) ;; => #f
```


### `(exact? ...)`

- **Rust 底层绑定**: `exactp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is exact.

(exact? val) -> boolean?

* val : any - The value to check for exactness.

# Examples
```scheme
> (exact? 42) ;; => #t
> (exact? 3.14) ;; => #f
> (exact? "hello") ;; => #f
```


### `(exp ...)`

- **Rust 底层绑定**: `exp`
- **函数签名参数**: `(left: &SteelVal)`

Returns Euler’s number raised to the power of z.

(exp z) -> number?

* z : number? - The number to raise e to the power of.

# Examples
```scheme
> (exp 0) ;; => 1
> (exp 2) ;; => 7.38905609893065
> (exp 1.5) ;; => 4.4816890703380645
```


### `(expt ...)`

- **Rust 底层绑定**: `expt`
- **函数签名参数**: `(left: &SteelVal, right: &SteelVal)`

Raises the left operand to the power of the right operand.

(expt base exponent) -> number?

* base : number? - The base number.
* exponent : number? - The exponent to raise the base to.

# Examples
```scheme
> (expt 2 3) ;; => 8
> (expt 2.0 0.5) ;; => 1.4142135623730951
> (expt 9 0.5) ;; => 3
```


### `(finite? ...)`

- **Rust 底层绑定**: `finitep`
- **函数签名参数**: `(number: &SteelVal)`

Returns `#t` if the given number is finite.

(finite? number) -> boolean?

* number : number? - The number to check for finiteness.

# Examples
```scheme
> (finite? 42) ;; => #t
> (finite? 0.1) ;; => #t
> (finite? +inf.0) ;; => #f
> (finite? -inf.0) ;; => #f
> (finite? +nan.0) ;; => #f
```


### `(float? ...)`

- **Rust 底层绑定**: `floatp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is a floating-point number

(float? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (float? 42) ;; => #f
> (float? 3.14) ;; => #t
> (float? #t) ;; => #f
```


### `(floor ...)`

- **Rust 底层绑定**: `floor`
- **函数签名参数**: `(number: &SteelVal)`

Rounds the given number down to the nearest integer not larger than it.

(floor number) -> number?

* number : real? - The number to compute the floor for.

# Examples
```scheme
> (floor 3.14) ;; => 3
> (floor 4.99) ;; => 4
> (floor -2.5) ;; => -3
```


### `(imag-part ...)`

- **Rust 底层绑定**: `imag_part`
- **函数签名参数**: `(value: &SteelVal)`

Returns the imaginary part of a number

(imag-part number) -> number?

# Examples
```scheme
> (imag-part 3+4i) ;; => 4
> (imag-part 42) ;; => 0
```


### `(inexact ...)`

- **Rust 底层绑定**: `inexact`
- **函数签名参数**: `(number: &SteelVal)`

Converts a number to an inexact number.

(inexact num) -> number?

* num : number? - The number to convert from exact to inexact.

# Examples
```scheme
> (inexact 10) ;; => 10
> (inexact 1/2) ;; => 0.5
> (inexact 1+2i) ;; => 1+2i
```


### `(inexact? ...)`

- **Rust 底层绑定**: `inexactp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is inexact.

(inexact? val) -> boolean?

* val : any - The value to check for inexactness.

# Examples
```scheme
> (inexact? 42) ;; => #f
> (inexact? 3.14) ;; => #t
```


### `(infinite? ...)`

- **Rust 底层绑定**: `infinitep`
- **函数签名参数**: `(number: &SteelVal)`

Returns `#t` if the given number is infinite.

(infinite? number) -> boolean?

* number : number? - The number to check for infiniteness.

# Examples
```scheme
> (infinite? 42) ;; => #f
> (infinite? -nan.0) ;; => #f
> (infinite? +inf.0) ;; => #t
```


### `(int? ...)`

- **Rust 底层绑定**: `intp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is an integer, an alias for `integer?`

(int? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (int? 42) ;; => #t
> (int? 3.14) ;; => #f
> (int? "hello") ;; => #f
```


### `(integer? ...)`

- **Rust 底层绑定**: `integerp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is an integer, an alias for `int?`

(integer? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (integer? 42) ;; => #t
> (integer? 3.14) ;; => #f
> (integer? "hello") ;; => #f
```


### `(magnitude ...)`

- **Rust 底层绑定**: `magnitude`
- **函数签名参数**: `(number: &SteelVal)`

Computes the magnitude of the given number.

(magnitude number) -> number?

* number : number? - The number to compute the magnitude for.

# Examples
```scheme
> (magnitude 3+4i) ;; => 5
> (magnitude 5) ;; => 5
> (magnitude -5) ;; => 5
```


### `(make-polar ...)`

- **Rust 底层绑定**: `make_polar`
- **函数签名参数**: `(r: &SteelVal, theta: &SteelVal)`

Make a complex number out of a magnitude `r` and an angle `θ`, so that the result is `r * (cos θ + i sin θ)`

(make-polar r θ) -> number?

- r : real?
- theta : real?


### `(make-rectangular ...)`

- **Rust 底层绑定**: `make_rectangular`
- **函数签名参数**: `(re: &SteelVal, im: &SteelVal)`

Create a complex number with `re` as the real part and `im` as the imaginary part.

(make-rectangular re im) -> number?

- re : real?
- im : real?


### `(nan? ...)`

- **Rust 底层绑定**: `nanp`
- **函数签名参数**: `(value: &SteelVal)`

Returns `#t` if the real number is Nan.

(nan? value) -> boolean?

* value : real? - The value to check

```scheme
(nan? +nan.0) => #t
(nan? 100000) => #f
```


### `(negative? ...)`

- **Rust 底层绑定**: `negativep`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given real number is negative.

(negative? num) -> boolean?

* num : real? - The real number to check for negativity.

# Examples
```scheme
> (negative? 0) ;; => #f
> (negative? 1) ;; => #f
> (negative? -1) ;; => #t
```


### `(number? ...)`

- **Rust 底层绑定**: `numberp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is a number

(number? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (number? 42) ;; => #t
> (number? "hello") ;; => #f
> (number? 'symbol) ;; => #f
```


### `(numerator ...)`

- **Rust 底层绑定**: `numerator`
- **函数签名参数**: `(number: &SteelVal)`

Retrieves the numerator of the given rational number.

(numerator number) -> number?

* number : number? - The rational number to retrieve the numerator from.

# Examples
```scheme
> (numerator 3/4) ;; => 3
> (numerator 5/2) ;; => 5
> (numerator -2) ;; => -2
```


### `(odd? ...)`

- **Rust 底层绑定**: `odd`
- **函数签名参数**: `(arg: &SteelVal)`

Checks if the given number is odd

(odd? n) -> bool?

* n : number? - The number to check for oddness.

# Examples
```scheme
> (odd? 2) ;; => #false
> (odd? 3) ;; => #true
> (odd? 5.0) ;; => #true
```


### `(positive? ...)`

- **Rust 底层绑定**: `positivep`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given real number is positive.

(positive? num) -> boolean?

* num : real? - The real number to check for positivity.

# Examples
```scheme
> (positive? 0) ;; => #f
> (positive? 1) ;; => #t
> (positive? -1) ;; => #f
```


### `(rational? ...)`

- **Rust 底层绑定**: `rationalp`
- **函数签名参数**: `(value: &SteelVal)`

Returns #t if obj is a rational number, #f otherwise.
Rational numbers are numbers that can be expressed as the quotient of two numbers.
For example, 3/4, -5/2, 0.25, and 0 are rational numbers.

(rational? value) -> bool?

* value : any - The value to check

Examples:
```scheme
> (rational? (/ 0.0)) ;; => #f
> (rational? 3.5) ;; => #t
> (rational? 6/10) ;; => #t
> (rational? +nan.0) ;; => #f
```


### `(real-part ...)`

- **Rust 底层绑定**: `real_part`
- **函数签名参数**: `(value: &SteelVal)`

Returns the real part of a number

(real-part number) -> number?

# Examples
```scheme
> (real-part 3+4i) ;; => 3
> (real-part 42) ;; => 42
```


### `(real? ...)`

- **Rust 底层绑定**: `realp`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given value is a real number

(real? value) -> boolean?

* value : any - The value to check

# Examples
```scheme
> (real? 42) ;; => #t
> (real? 3+4i) ;; => #f
> (real? "hello") ;; => #f
```


### `(round ...)`

- **Rust 底层绑定**: `round`
- **函数签名参数**: `(number: &SteelVal)`

Rounds the given number to the nearest integer, rounding half-way cases to
the even number.

(round number) -> number?

* number : real? - The number to round.

# Examples
```scheme
> (round 3.14) ;; => 3
> (round 4.6) ;; => 5
> (round 2.5) ;; => 2
> (round 3.5) ;; => 4
> (round -2.5) ;; => -2
```


### `(sin ...)`

- **Rust 底层绑定**: `sin`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the sine value of the input angle, measured in radians.

(sin n) -> number?

* n : number? - The input angle, in radians.

# Examples
```scheme
> (sin 0) ;; => 0
> (sin 1) ;; => 0.8414709848078965
> (sin 2.0) ;; => 0.9092974268256817
> (sin 3.14) ;; => 0.0015926529164868282
```


### `(sqrt ...)`

- **Rust 底层绑定**: `sqrt`
- **函数签名参数**: `(number: &SteelVal)`

Computes the square root of the given number.

(sqrt number) -> number?

* number : number? - The number to compute the square root for.

# Examples
```scheme
> (sqrt 4) ;; => 2
> (sqrt 2) ;; => 1.4142135623730951
> (sqrt -1) ;; => 0+1i
```


### `(square ...)`

- **Rust 底层绑定**: `square`
- **函数签名参数**: `(number: &SteelVal)`

Computes the square of the given number.

(square number) -> number?

* number : number? - The number to square.

# Examples
```scheme
> (square 5) ;; => 25
> (square -3) ;; => 9
> (square 2.5) ;; => 6.25
```


### `(tan ...)`

- **Rust 底层绑定**: `tan`
- **函数签名参数**: `(arg: &SteelVal)`

Returns the tangent value of the input angle, measured in radians.

(tan n) -> number?

* n : number? - The input angle, in radians.

# Examples
```scheme
> (tan 0) ;; => 0
> (tan 1) ;; => 1.557407724654902
> (tan 2.0) ;; => -2.185039863261519
> (tan 3.14) ;; => -0.0015926549364072232
```


### `(truncate ...)`

- **Rust 底层绑定**: `truncate`
- **函数签名参数**: `(arg: &SteelVal)`

Rounds the given number to the nearest integer, whose absolute value is not
larger than it.

(truncate number) -> integer?

* number : real? - The number to truncate.

# Examples

```scheme
> (truncate 42) ;; => 42
> (truncate 42.1) ;; => 42
> (truncate -42.1) ;; => -42
```


### `(zero? ...)`

- **Rust 底层绑定**: `zerop`
- **函数签名参数**: `(value: &SteelVal)`

Checks if the given real number is zero.

(zero? num) -> boolean?

* num : real? - The number to check for zero.

# Examples
```scheme
> (zero? 0) ;; => #t
> (zero? 0.0) ;; => #t
> (zero? 0.1) ;; => #f
```


## 字符串与字符库 (Strings & Chars)

提供高效 UTF-8 字符串、子串切片、大小写转换、格式化、正则表达式辅助与字符操作。在 Scheme 中默认全局可用。

### `(char->integer ...)`

- **Rust 底层绑定**: `char_to_integer`
- **函数签名参数**: `(ch: char)`

Returns the Unicode codepoint of a given character.

(char->integer char?) -> integer?

# Examples

```scheme
> (char->integer #\a) ;; => 97
> (char->integer #\λ) ;; => 955
```


### `(char->number ...)`

- **Rust 底层绑定**: `char_to_number`
- **函数签名参数**: `(c: char)`

Attemps to convert the character into an ascii decimal digit,
and returns `#f` on failure.

(char->number char?) -> (or/c number? bool?)

# Examples

```scheme
> (char->number #\4) ;; => 4
> (char->number #\a) ;; => #f
> (char->number #\٣) ;; => #f
```


### `(char-ci=? ...)`

- **Rust 底层绑定**: `char_ci_equals`
- **函数签名参数**: `(rest: RestArgsIter<char>)`

Checks if all characters are equal, in a case-insensitive fashion
(i.e. as if char-foldcase was applied to the arguments).

Requires that all inputs are characters, and will otherwise raise an error.

(char-ci=? char1 char2 ...) -> bool?

* char1 : char?
* char2 : char?

# Examples

```scheme
> (char-ci=? #\s #\S) ;; => #t
> (char-ci=? #\ß #\ẞ) ;; => #t
> (char-ci=? #\σ #\Σ #\ς) ;; => #t
```


### `(char-digit? ...)`

- **Rust 底层绑定**: `char_is_digit`
- **函数签名参数**: `(c: char)`

Returns `#t` if the character is an ascii decimal digit.

(char-digit? char?) -> bool?

# Examples

```scheme
> (char-digit? #\4) ;; => #t
> (char-digit? #\a) ;; => #f
> (char-digit? #\٣) ;; => #f
> (char-digit? #\①) ;; => #f
```


### `(char-downcase ...)`

- **Rust 底层绑定**: `char_downcase`
- **函数签名参数**: `(c: char)`

Returns the lower case version of a character, if defined by Unicode,
or the same character otherwise.

(char-downcase char?) -> char?

# Examples

```scheme
> (char-downcase #\U) ;; => #\u
> (char-downcase #\d) ;; => #\d
> (char-downcase #\ẞ) ;; => #\ß
```


### `(char-foldcase ...)`

- **Rust 底层绑定**: `char_foldcase`
- **函数签名参数**: `(c: char)`

Apply simple unicode case-folding to a char

(char-foldcase char?) -> char?

# Examples

```scheme
> (char-foldcase #\A) ;; => #\a
> (char-foldcase #\c) ;; => #\c
> (char-foldcase #\ς) ;; => #\σ
```


### `(char-upcase ...)`

- **Rust 底层绑定**: `char_upcase`
- **函数签名参数**: `(c: char)`

Returns the upper case version of a character, if defined by Unicode,
or the same character otherwise.

(char-upcase char?) -> char?

# Examples

```scheme
> (char-upcase #\d) ;; => #\D
> (char-upcase #\U) ;; => #\U
> (char-upcase #\ß) ;; => #\ß
```


### `(char-whitespace? ...)`

- **Rust 底层绑定**: `char_is_whitespace`
- **函数签名参数**: `(c: char)`

Returns `#t` if the character is a whitespace character.

# Example

```scheme
> (char-whitespace? #\space) ;; => #t
> (char-whitespace? #\newline) ;; => #t
; nbsp character
> (char-whitespace? #\xA0) ;; => #t
> (char-whitespace? #\越) ;; => #f
```


### `(char=? ...)`

- **Rust 底层绑定**: `char_equals`
- **函数签名参数**: `(rest: RestArgsIter<char>)`

Checks if all characters are equal.

Requires that all inputs are characters, and will otherwise raise an error.

(char=? char1 char2 ...) -> bool?

* char1 : char?
* char2 : char?

# Examples

```scheme
> (char=? #\a #\a) ;; => #t
> (char=? #\a #\b) ;; => #f
> (char=? #\a #\A) ;; => #f
```


### `(ends-with? ...)`

- **Rust 底层绑定**: `ends_with`
- **函数签名参数**: `(value: &SteelString, suffix: &SteelString)`

Checks if the input string ends with a given suffix

(ends-with? input pattern) -> bool?

* input : string?
* pattern: string?

# Examples

```scheme
> (ends-with? "foobar" "foo") ;; => #false
> (ends-with? "foobar" "bar") ;; => #true
```


### `(int->string ...)`

- **Rust 底层绑定**: `int_to_string`
- **函数签名参数**: `(value: isize)`

Converts an integer into a string.

Use of this procedure is discouraged in favor of the more powerful and more
portable `(number->string)` procedure.

(int->string int?) -> string?

# Examples

```scheme
> (int->string 10) ;; => "10"
```


### `(integer->char ...)`

- **Rust 底层绑定**: `integer_to_char`
- **函数签名参数**: `(int: u32)`

Returns the character corresponding to a given Unicode codepoint.

(integer->char integer?) -> char?

# Examples

```scheme
> (integer->char #x61) ;; => #\a
> (integer->char 955) ;; => #\λ
```


### `(make-string ...)`

- **Rust 底层绑定**: `make_string`
- **函数签名参数**: `(k: usize, mut c: RestArgsIter<'_, char>)`

Creates a string of a given length, filled with an optional character
(which defaults to `#\0`).

(make-string len [char]) -> string?

* len : int?
* char : char? = #\0

# Examples

```scheme
> (make-string 5 #\a) ;; => "aaaaa"
> (make-string 5) ;; => "\0\0\0\0\0"
```


### `(number->string ...)`

- **Rust 底层绑定**: `number_to_string`
- **函数签名参数**: `(value: &SteelVal, mut rest: RestArgsIter<'_, isize>)`

Converts the given number to a string, with an optional radix.

Returns an error, if the value given is not a number.

(number->string number? [radix]) -> string?

* radix: number?

```scheme
> (number->string 10) ;; => "10"
> (number->string 1.0) ;; => "1.0"
> (number->string 1/2) ;; => "1.0"
> (number->string 1+2i) ;; => "1+2i"
> (number->string 255 16) ;; => "ff"
> (number->string 1/2 2) ;; => "1/10"
```


### `(split-many ...)`

- **Rust 底层绑定**: `split_many`
- **函数签名参数**: `(value: &SteelString, pat: &SteelString)`

Splits a string given a separator pattern into a list of strings.

(split-many str pat) -> (listof string?)

* str : string?
* pat : string?

# Examples
```scheme
(split-many "foo,bar,baz" ",") ;; => '("foo" "bar" "baz")
(split-many "foo|bar|" "|") ;; => '("foo" "bar" "")
(split-many "" "&") ;; => '("")
```


### `(split-once ...)`

- **Rust 底层绑定**: `split_once`
- **函数签名参数**: `(value: &SteelString, pat: &SteelString)`

Splits a string given a separator at most once, yielding
a list with at most 2 elements.

(split-once str pat) -> string?

* str : string?
* pat : string?

# Examples
```scheme
(split-once "foo,bar,baz" ",") ;; => '("foo" "bar,baz")
(split-once "foo|bar|" "|") ;; => '("foo" "bar|")
(split-once "" "&") ;; => '("")
```


### `(split-whitespace ...)`

- **Rust 底层绑定**: `split_whitespace`
- **函数签名参数**: `(value: &SteelString)`

Returns a list of strings from the original string split on the whitespace

(split-whitespace string?) -> (listof string?)

# Examples

```scheme
(split-whitespace "apples bananas fruits veggies") ;; '("apples" "bananas" "fruits" "veggies")
(split-whitespace "one\t \ttwo\nthree") ;; '("one" "two" "three")
```


### `(starts-with? ...)`

- **Rust 底层绑定**: `starts_with`
- **函数签名参数**: `(value: &SteelString, prefix: &SteelString)`

Checks if the input string starts with a prefix

(starts-with? input pattern) -> bool?

* input : string?
* pattern: string?

# Examples

```scheme
> (starts-with? "foobar" "foo") ;; => #true
> (starts-with? "foobar" "bar") ;; => #false
```


### `(string ...)`

- **Rust 底层绑定**: `string_constructor`
- **函数签名参数**: `(rest: RestArgsIter<'_, char>)`

Constructs a string from the given characters

(string . char?) -> string?

# Examples

```scheme
> (string #\h #\e #\l #\l #\o) ;; => "hello"
> (string #\λ) ;; => "λ"
> (string) ;; => ""
```


### `(string->bytes ...)`

- **Rust 底层绑定**: `string_to_bytes`
- **函数签名参数**: `(value: &SteelString, mut rest: RestArgsIter<isize>)`

Encodes a string as UTF-8 into a bytevector.

(string->bytes str [start] [end]) -> bytes?

* str : string?
* start : int? = 0
* end : int? = (string-length str)

# Examples

```scheme
(string->bytes "Apple") ;; => #u8(#x41 #x70 #x70 #x6C #x65)
(string->bytes "αβγ") ;; => #u8(#xCE #xB1 #xCE #xB2 #xCE #xB3)
(string->bytes "one two three" 4 7) ;; => #u8(#x74 #x77 #x6F)
```


### `(string->int ...)`

- **Rust 底层绑定**: `string_to_int`
- **函数签名参数**: `(value: &SteelString)`

Converts a string into an int. Raises an error if the string cannot be converted to an integer.

Use of this procedure is discouraged in favor of the more powerful and more
portable `(string->number)` procedure.

(string->int string?) -> int?

# Examples

```scheme
> (string->int "100") ;; => 10
> (string->int "not-an-int") ;; error
```


### `(string->list ...)`

- **Rust 底层绑定**: `string_to_list`
- **函数签名参数**: `(value: &SteelString, mut rest: RestArgsIter<isize>)`

Converts a string into a list of characters.

(string->list str [start] [end]) -> (listof char?)

* str : string?
* start : int? = 0
* end : int? = (string-length str)

# Examples

```scheme
> (string->list "hello") ;; => '(#\h #\e #\l #\l #\o)
> (string->list "one two three" 4 7) ;; => '(#\t #\w #\o)
```


### `(string->number ...)`

- **Rust 底层绑定**: `string_to_number`
- **函数签名参数**: `(value: &SteelString, mut rest: RestArgsIter<'_, isize>,)`

Converts the given string to a number, with an optional radix.
On failure, it returns `#f`

(string->number digits [radix]) -> (or/c number? boolean?)

* digits : string?
* radix : number?

# Examples

```scheme
> (string->number "10") ;; => 10
> (string->number "1.0") ;; => 1.0
> (string->number "1/2") ;; => 1/2
> (string->number "1+2i") ;; => 1+2i
> (string->number "ff") ;; => #f
> (string->number "ff" 16) ;; => 255
> (string->number "1/10" 2) ;; => 1/2
> (string->number "not-a-number") ;; => #f
```


### `(string->uninterned-symbol ...)`

- **Rust 底层绑定**: `string_to_uninterned_symbol`
- **函数签名参数**: `(value: SteelString)`

Return an uninterned symbol from the given string.

(string->uninterned-symbol string?) -> symbol?

# Examples

```scheme
(string->uninterned-symbol "abc") ;; => 'abc
(string->uninterned-symbol "pea pod") ;; => '|pea pod|
```


### `(string->vector ...)`

- **Rust 底层绑定**: `string_to_vector`
- **函数签名参数**: `(value: &SteelString, mut rest: RestArgsIter<isize>)`

Returns a vector containing the characters of a given string

(string->vector s [start] [end]) -> vector?

* str : string?
* start : int? = 0
* end : int? = (string-length str)

# Examples

```scheme
(string->vector "hello") ;; => '#(#\h #\e #\l #\l #\o)
(string->vector "one two three" 4 7) ;; => '#(#\t #\w #\o)
```


### `(string-append ...)`

- **Rust 底层绑定**: `string_append`
- **函数签名参数**: `(mut rest: RestArgsIter<'_, &SteelString>)`

Concatenates all of the given strings into one

(string-append strs...) -> string?

* strs ... : string?

# Examples
```scheme
> (string-append) ;; => ""
> (string-append "foo" "bar") ;; => "foobar"
```


### `(string-ci=? ...)`

- **Rust 底层绑定**: `string_ci_equals`
- **函数签名参数**: `(rest: RestArgsIter<&SteelString>)`

Compares strings for equality, in a case insensitive fashion.

(string-ci=? string? string? ...) -> bool?

# Examples

```scheme
> (string-ci=? "hEllO WorLd" "HELLO worlD") ;; => #t
> (string-ci=? "Straße" "STRASSE" "strasse" "STRAẞE") ;; => #t
> (string-ci=? "ὈΔΥΣΣΕΎΣ" "ὀδυσσεύς" "ὀδυσσεύσ") ;; => #t
```


### `(string-contains? ...)`

- **Rust 底层绑定**: `string_contains`
- **函数签名参数**: `(s: &SteelString, contained: &SteelString)`

Searches a string to check if it contains the second argument.

(string-contains? string? string?) -> bool?

# Examples
```scheme
(string-contains? "hello" "lo") ;;=> #t
(string-contains? "hello" "world") ;;=> #f
```


### `(string-downcase ...)`

- **Rust 底层绑定**: `string_downcase`
- **函数签名参数**: `(value: &SteelString)`

Creates a new lowercased version of the input string

(string-downcase string?) -> string?

# Examples

```scheme
> (string-downcase "sPonGeBoB tExT") ;; => "spongebob text"
> (string-downcase "ὈΔΥΣΣΕΎΣ") ;; => "ὀδυσσεύς"
> (string-downcase "STRAẞE") ;; => "straße"
```


### `(string-foldcase ...)`

- **Rust 底层绑定**: `string_foldcase`
- **函数签名参数**: `(value: &SteelString)`

Applies full unicode case-folding to the input string

(string-foldcase string?) -> string?

# Examples

```scheme
> (string-foldcase "Straße") ;; => "strasse"
```


### `(string-join ...)`

- **Rust 底层绑定**: `string_join`
- **函数签名参数**: `(strings: SteelList<SteelVal>, mut rest: RestArgsIter<'_, &SteelString>,)`

Joins the given list of strings, with an optional separator.

(string-join strings [sep]) -> string?

* strings : (listof string?)
* sep : string? = ""

# Examples
```scheme
(string-join '("a" "b" "c")) ;; => "abc"
(string-join '("one" "two" "three") ", ") ;; => "one, two, three"
```


### `(string-length ...)`

- **Rust 底层绑定**: `string_length`
- **函数签名参数**: `(value: &SteelString)`

Get the number of characters in the string.

(string-length string?) -> int?

# Examples

```scheme
> (string-length "apples") ;; => 6
> (string-length "αβγ") ;; => 3
> (string-length "✅") ;; => 1
```


### `(string-push ...)`

- **Rust 底层绑定**: `string_push`
- **函数签名参数**: `(value: &mut SteelVal, character_or_string: SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(string-ref ...)`

- **Rust 底层绑定**: `string_ref`
- **函数签名参数**: `(value: &SteelString, index: usize)`

Extracts the nth character out of a given string, starting at 0.

(string-ref str n) -> char?

* str : string?
* n : int?

```scheme
(string-ref "one" 1) ;; => #\n
(string-ref "αβγ" 1) ;; => #\β
```


### `(string-replace ...)`

- **Rust 底层绑定**: `replace`
- **函数签名参数**: `(value: &SteelString, from: &SteelString, to: &SteelString)`

Replaces all occurrences of a pattern into the given string

(string-replace str from to) -> string?

* str : string?
* from : string?
* to : string?

# Examples
```scheme
(string-replace "hello world" "o" "@") ;; => "hell@ w@rld"
```


### `(string-upcase ...)`

- **Rust 底层绑定**: `string_upcase`
- **函数签名参数**: `(value: &SteelString)`

Creates a new uppercased version of the input string

(string-upcase string?) -> string?

# Examples

```scheme
> (string-upcase "lower") ;; => "LOWER"
> (string-upcase "straße") ;; => "STRASSE"
```


### `(string=? ...)`

- **Rust 底层绑定**: `string_equals`
- **函数签名参数**: `(rest: RestArgsIter<&SteelString>)`

Compares strings for equality.

(string=? string1 string2 ...) -> bool?

* string1 : string?
* string2 : string?

# Examples

```scheme
> (string=? "hello" "hello") ;; => #t
> (string=? "hello" "HELLO") ;; => #f
```


### `(substring ...)`

- **Rust 底层绑定**: `substring`
- **函数签名参数**: `(value: &SteelString, i: usize, mut rest: RestArgsIter<'_, isize>,)`

Creates a substring slicing the characters between two indices.

(substring str start end) -> string?

* str: string?
* start : int?
* end : int?

# Examples
```scheme
(substring "hello" 1 4) ;; => "ell"
(substring "hello" 10 15) ;; => error
```


### `(trim ...)`

- **Rust 底层绑定**: `trim`
- **函数签名参数**: `(value: &SteelString)`

Returns a new string with the leading and trailing whitespace removed.

(trim string?) -> string?

# Examples

```scheme
> (trim "   foo     ") ;; => "foo"
```


### `(trim-end ...)`

- **Rust 底层绑定**: `trim_end`
- **函数签名参数**: `(value: &SteelString)`

Returns a new string with the trailing whitespace removed.

(trim string?) -> string?

# Examples

```scheme
> (trim "   foo     ") ;; => "   foo"
```


### `(trim-end-matches ...)`

- **Rust 底层绑定**: `trim_end_matches`
- **函数签名参数**: `(value: &SteelString, pat: &SteelString)`

Returns a new string with the given `pat` repeatedly removed from the end
of the string

```scheme
(trim-end-matches string? string?) -> string?
```

# Examples
```scheme
> (trim-end-matches "123foo1bar123123" "123") ;; => "123foo1bar"
```


### `(trim-start ...)`

- **Rust 底层绑定**: `trim_start`
- **函数签名参数**: `(value: &SteelString)`

Returns a new string with the leading whitespace removed.

(trim string?) -> string?

# Examples

```scheme
> (trim "   foo     ") ;; => "foo     "
```


### `(trim-start-matches ...)`

- **Rust 底层绑定**: `trim_start_matches`
- **函数签名参数**: `(value: &SteelString, pat: &SteelString)`

Returns a new string with the given `pat` repeatedly removed from the start
of the string

```scheme
(trim-start-matches string? string?) -> string?
```

# Examples
```scheme
> (trim-start-matches "123foo1bar123123" "123") ;; => "foo1bar123123"
```


### `(utf8-length ...)`

- **Rust 底层绑定**: `utf8_length`
- **函数签名参数**: `(value: &SteelString)`

Get the length of the string in UTF-8 bytes.

(utf8-length string?) -> int?

# Examples

```scheme
> (utf8-length "apples") ;; => 6
> (utf8-length "αβγ") ;; => 6
> (utf8-length "✅") ;; => 3
```


## 列表与对子库 (Lists & Pairs)

Steel 采用展开链表（Unrolled Linked List）实现，提供 cons、car、cdr、append、reverse、take 等经典表操作与 O(1) 短表索引。

### `(#%list-sort ...)`

- **Rust 底层绑定**: `list_sort`
- **函数签名参数**: `(value: &List<SteelVal>, func: SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(empty? ...)`

- **Rust 底层绑定**: `is_empty`
- **函数签名参数**: `(list: &SteelVal)`

Checks if the list is empty

(empty? lst) -> bool?

* lst: list?

# Examples

```scheme
> (empty? (list 1 2 3 4 5)) ;; => #false
> (empty? '()) ;; => #true
```


### `(first ...)`

- **Rust 底层绑定**: `first`
- **函数签名参数**: `(list: &List<SteelVal>)`

Returns the first element of the list l.

(first l) -> any/c

* l : list?

# Examples

```scheme
> (first '(1 2)) ;; => 1
> (first (cons 2 3)) ;; => 2
```


### `(last ...)`

- **Rust 底层绑定**: `last`
- **函数签名参数**: `(list: &List<SteelVal>)`

Returns the last element in the list. Takes time proportional to the length of the list.

(last l) -> any/c

* l : list?

# Examples
```scheme
> (list (list 1 2 3 4)) ;; => 4
```


### `(length ...)`

- **Rust 底层绑定**: `length`
- **函数签名参数**: `(list: &List<SteelVal>)`

Returns the length of the list.

(length l) -> int?

* l : list?

# Examples

```scheme
> (length (list 10 20 30)) ;; => 3
```


### `(list->string ...)`

- **Rust 底层绑定**: `list_to_string`
- **函数签名参数**: `(list: &List<SteelVal>)`

Convert a list of charecters to a string.

(list->string lst) -> string?

* lst : (listof char?)

# Examples
```scheme
(list->string '(#\a #\b #\c)) ;; => "abc"
```


### `(list->vector ...)`

- **Rust 底层绑定**: `list_to_vector`
- **函数签名参数**: `(list: &List<SteelVal>)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(list-chunks ...)`

- **Rust 底层绑定**: `chunks`
- **函数签名参数**: `(list: &List<SteelVal>)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(list-contains ...)`

- **Rust 底层绑定**: `list_contains`
- **函数签名参数**: `(value: &SteelVal, list: &List<SteelVal>)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(list-drop ...)`

- **Rust 底层绑定**: `drop_start`
- **函数签名参数**: `(list: &List<SteelVal>, skip: usize)`

Remove a certain number of elements (`n`) from the front of `lst`.

(list-drop lst n) -> any/c

* lst : list?
* n : int?

# Examples
```scheme
> (list-drop '(1 2 3 4 5) 2) ;; => '(3 4 5)
> (list-drop '() 3) ;; => '()
```


### `(list-ref ...)`

- **Rust 底层绑定**: `list_ref`
- **函数签名参数**: `(list: &List<SteelVal>, index: isize)`

Returns the value located at the given index. Will raise an error if you try to index out of bounds.

Note: Runs in time proportional to the length of the list, however lists in Steel are implemented in such a fashion that the
time complexity is O(n/64). Meaning, for small lists this can be constant.

(list-ref lst index) -> list?

* lst : list?
* index : (and/c int? positive?)

# Examples
```scheme
> (list-ref (list 1 2 3 4) 2) ;; => 3
> (list-ref (range 0 100) 42) ;; => 42
> (list-ref (list 1 2 3 4) 10)
error[E11]: Generic
┌─ :1:2
│
1 │ (list-ref (list 1 2 3 4) 10)
│  ^^^^^^^^ out of bounds index in list-ref - list length: 4, index: 10
```


### `(list-tail ...)`

- **Rust 底层绑定**: `list_tail`
- **函数签名参数**: `(list_or_pair: &SteelVal, pos: usize)`

Same as `list-drop`, except raise an error if `n` is greater than the length of `lst`.

(list-tail lst n) -> any/c

* lst : list?
* n : int?

# Examples
```scheme
> (list-tail '(1 2 3 4 5) 2) ;; => '(3 4 5)
> (list-tail '() 3)
error[E11]: Generic
┌─ :1:2
│
1 │ (list-tail '() 3)
│  ^^^^^^^^^ list-tail expects at least 3
elements in the list, found: 0
```


### `(member ...)`

- **Rust 底层绑定**: `member`
- **函数签名参数**: `(value: &SteelVal, list: &List<SteelVal>)`

Return the first tail of the list, where the car is `equal?` to the given obj.
Returns `#f`, if no element is found.

(member obj lst) -> (or/c list? #f)

* obj : any/c
* lst : list?

```scheme
(member #\c '(#\a #\b #\c #\d #\e)) ;; => '(#\c #\d #\e)
(member 5 '(0 1 2 3 4)) ;; => #f
```


### `(memq ...)`

- **Rust 底层绑定**: `memq`
- **函数签名参数**: `(value: &SteelVal, list: &List<SteelVal>)`

Return the first tail of the list, where the car is `eq?` to the given obj.
Returns `#f`, if no element is found.

This procedure is equivalent to `member`, but using `eq?` instead of `equal?`.

(memq obj lst) -> (or/c list? #f)

* obj : any/c
* lst : list?

```scheme
(memq #\c '(#\a #\b #\c #\d #\e)) ;; => '(#\c #\d #\e)
(memq 5 '(0 1 2 3 4)) ;; => #f
```


### `(pair? ...)`

- **Rust 底层绑定**: `pair`
- **函数签名参数**: `(list: &SteelVal)`

Checks if the given value can be treated as a pair.

(pair? any/c) -> bool?

# Examples

```scheme
> (pair? '(10 20)) ;; => #true
> (pair? '(10)) ;; => #true
> (pair? '()) ;; => #false
```


### `(plist-get ...)`

- **Rust 底层绑定**: `plist_get`
- **函数签名参数**: `(list: &List<SteelVal>, key: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-get-kwarg ...)`

- **Rust 底层绑定**: `plist_get_context`
- **函数签名参数**: `(list: &List<SteelVal>, key: &SteelVal, func: &SteelVal,)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-get-positional-arg ...)`

- **Rust 底层绑定**: `plist_get_positional`
- **函数签名参数**: `(list: &List<SteelVal>, index: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-get-positional-arg-list ...)`

- **Rust 底层绑定**: `plist_get_positional_list`
- **函数签名参数**: `(list: &List<SteelVal>, index: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-try-get ...)`

- **Rust 底层绑定**: `plist_try_get`
- **函数签名参数**: `(list: &List<SteelVal>, key: &SteelVal, default_value: SteelVal,)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-try-get-positional-arg ...)`

- **Rust 底层绑定**: `plist_try_get_positional`
- **函数签名参数**: `(list: &List<SteelVal>, index: usize, default_value: SteelVal,)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(plist-validate-args ...)`

- **Rust 底层绑定**: `plist_validate_args`
- **函数签名参数**: `(list: &List<SteelVal>, required_keyword_arg_count: usize, required_positional_arg_count: usize, optional_keyword_arg_count: usize, optional_positional_arg_count: usize, is_rest: bool,)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(reverse ...)`

- **Rust 底层绑定**: `reverse`
- **函数签名参数**: `(arg: &mut SteelVal)`

Returns a list that has the same elements as `lst`, but in reverse order.
This function takes time proportional to the length of `lst`.

(reverse lst) -> list?

* lst : list?

# Examples
```scheme
> (reverse (list 1 2 3 4)) ;; '(4 3 2 1)
```


### `(second ...)`

- **Rust 底层绑定**: `second`
- **函数签名参数**: `(list: &List<SteelVal>)`

Get the second element of the list. Raises an error if the list does not have an element in the second position.

(second l) -> any/c

* l : list?

# Examples

```scheme
> (second '(1 2 3)) ;; => 2
error[E11]: Generic
┌─ :1:2
│
1 │ (second '())
│  ^^^^^^ second: index out of bounds - list did not have an element in the second position: []
```


### `(take ...)`

- **Rust 底层绑定**: `take`
- **函数签名参数**: `(list: &List<SteelVal>, n: isize)`

Returns the first n elements of the list l as a new list.

(take l n) -> list?

* l : list?
* n : (and/c positive? int?)

# Examples

```scheme
> (take '(1 2 3 4) 2) ;; => '(0 1)
> (take (range 0 10) 4) ;; => '(0 1 2 3)
```


### `(try-list-ref ...)`

- **Rust 底层绑定**: `try_list_ref`
- **函数签名参数**: `(list: &List<SteelVal>, index: isize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


## 向量库 (Vectors & Immutable Vectors)

提供紧凑内存连续存储的高效向量，支持 vector、vector-ref、vector-set!、vector-copy、不可变向量等操作。

### `(immutable-vector->list ...)`

- **Rust 底层绑定**: `immutable_vector_to_list`
- **函数签名参数**: `(vector: &SteelVector, rest: RestArgsIter<'_, isize>,)`

Converts an immutable vector into a list.

(immutable-vector->list vec) -> list?

* vec : immutable-vector?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (immutable-vector->list A) ;; => '(1 2 3)
```


### `(immutable-vector->string ...)`

- **Rust 底层绑定**: `immutable_vector_to_string`
- **函数签名参数**: `(vector: &SteelVector, rest: RestArgsIter<'_, isize>,)`

Converts an immutable vector of characters into a string.

(immutable-vector->string vec) -> string?

* vec : immutable-vector?

# Examples
```scheme
> (define A (immutable-vector #\a #\b #\c)) ;;
> (immutable-vector->string A) ;; => "abc"
```


### `(immutable-vector-append ...)`

- **Rust 底层绑定**: `immutable_vector_append`
- **函数签名参数**: `(mut rest: RestArgsIter<'_, &SteelVector>)`

Returns the combination of the given immutable vectors.

(immutable-vector-append . vecs) -> immutable-vector?

* vecs : immutable-vector? - The vectors to combine.

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (define B (immutable-vector 4 5 6)) ;;
> (immutable-vector-append A B) ;; => '#(1 2 3 4 5 6)
```


### `(immutable-vector-copy ...)`

- **Rust 底层绑定**: `immutable_vector_copy`
- **函数签名参数**: `(vector: &SteelVector, rest: RestArgsIter<'_, isize>)`

Returns a new copy of the given immutable vector.

(immutable-vector-copy vec) -> immutable-vector?

* vec : immutable-vector?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (immutable-vector-copy A) ;; => '#(1 2 3)
```


### `(immutable-vector-drop ...)`

- **Rust 底层绑定**: `immutable_vector_drop`
- **函数签名参数**: `(vector: &mut SteelVal, count: usize)`

Returns a new vector with the first `n` elements removed from the original vector.

(immutable-vector-drop vec n) -> immutable-vector?

* vec : immutable-vector?
* n : integer?

# Examples
```scheme
> (define A (immutable-vector 1 2 3 4)) ;;
> (immutable-vector-drop A 2) ;; => '#(3 4)
```


### `(immutable-vector-pop-back ...)`

- **Rust 底层绑定**: `immutable_vector_pop_back`
- **函数签名参数**: `(vector: &mut SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(immutable-vector-push ...)`

- **Rust 底层绑定**: `immutable_vector_push`
- **函数签名参数**: `(vector: &mut SteelVal, value: SteelVal)`

Pushes a value to the back of the vector, returning a new vector.

(immutable-vector-push vec val) -> immutable-vector?

* vec : immutable-vector?
* val : any?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (immutable-vector-push A 5) ;; => '#(1 2 3 5)
```


### `(immutable-vector-rest ...)`

- **Rust 底层绑定**: `immutable_vector_rest`
- **函数签名参数**: `(vector: &mut SteelVal)`

Returns the vector with the first value removed.

(immutable-vector-rest vec) -> immutable-vector?

* vec : immutable-vector?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (immutable-vector-rest A) ;; => '#(2 3)
```


### `(immutable-vector-set ...)`

- **Rust 底层绑定**: `immutable_vector_set`
- **函数签名参数**: `(vector: &mut SteelVal, index: usize, value: SteelVal)`

Returns a new vector with the specified index updated to the given value.

(immutable-vector-set vec index val) -> immutable-vector?

* vec : immutable-vector?
* index : integer?
* val : any?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (immutable-vector-set A 0 5) ;; => '#(5 2 3)
```


### `(immutable-vector-take ...)`

- **Rust 底层绑定**: `immutable_vector_take`
- **函数签名参数**: `(vector: &mut SteelVal, count: usize)`

Returns a new vector containing only the first `n` elements of the original vector.

(immutable-vector-take vec n) -> immutable-vector?

* vec : immutable-vector?
* n : integer?

# Examples
```scheme
> (define A (immutable-vector 1 2 3 4)) ;;
> (immutable-vector-take A 2) ;; => '#(1 2)
```


### `(make-immutable-vector ...)`

- **Rust 底层绑定**: `make_immutable_vector`
- **函数签名参数**: `(mut rest: RestArgsIter<'_, &SteelVal>)`

Creates an immutable vector of a given length, filled with an optional value.
(which defaults to '#<void>')

(make-immutable-vector len [val]) -> immutable-vector?

* len : integer?
* val : any? - defaults to #<void>

# Examples
```scheme
> (make-immutable-vector 3 5) ;; => '#(5 5 5)
> (make-immutable-vector 2) ;; => '#(#<void> #<void>)
```


### `(mut-vec-len ...)`

- **Rust 底层绑定**: `mut_vec_length`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>)`

Returns the length of a mutable vector.

(mut-vec-len vec) -> integer?

* vec : vector? - The mutable vector to retrieve the length of.

# Examples
```scheme
> (define A (mutable-vector 1 2 3 4 5)) ;;
> (mut-vec-len A) ;; => 5
```


### `(mutable-vector->clear ...)`

- **Rust 底层绑定**: `mutable_vector_clear`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>)`

Removes all elements from a mutable vector.

(mutable-vector->clear vec) -> void?

* vec : vector?

# Examples
```scheme
> (define A (vector 1 2 3 4)) ;;
> (mutable-vector->clear A) ;;
> A ;; => '#()
```


### `(mutable-vector->list ...)`

- **Rust 底层绑定**: `mut_vec_to_list`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>, rest: RestArgsIter<'_, isize>,)`

Converts a mutable vector into a list, optionally over a specified range.

(mutable-vector->list vec [start end]) -> list?

* vec : vector? - The mutable vector to convert.
* start : integer? - The starting index of the range (defaults to `0`).
* end : integer? - The exclusive ending index of the range (defaults to the length of `vec`).

# Examples
```scheme
> (define A (mutable-vector 1 2 3 4 5)) ;;
> (mutable-vector->list A) ;; => '(1 2 3 4 5)
> (mutable-vector->list A 1 4) ;; => '(2 3 4)
```


### `(mutable-vector->string ...)`

- **Rust 底层绑定**: `mutable_vector_to_string`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>)`

Converts a vector of characters into a string.

(mutable-vector->string vec) -> string?

* vec : vector? - (must contain only characters)

# Examples
```scheme
> (define A (vector #\H #\e #\l #\l #\o))
> (mutable-vector->string A) ;; => "Hello"
```


### `(mutable-vector-pop! ...)`

- **Rust 底层绑定**: `mutable_vector_pop`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>)`

Removes and returns the last element of the vector.

(mutable-vector-pop! vec) -> any?

* vec : vector? - the vector to modify

# Examples
```scheme
> (define A (vector 1 2 3))
> (mutable-vector-pop! A) ;; => 3
> A ;; => '#(1 2)
```


### `(vector->string ...)`

- **Rust 底层绑定**: `vector_to_string`
- **函数签名参数**: `(vector: Either<&SteelVector, &HeapRef<Vec<SteelVal>>>, rest: RestArgsIter<'_, isize>,)`

Converts a vector of characters into a string.

(vector->string vec) -> string?

* vec : vector?

# Examples
```scheme
> (define A (vector #\a #\b #\c)) ;;
> (vector->string A) ;; => "abc"
```


### `(vector-copy! ...)`

- **Rust 底层绑定**: `mut_vector_copy`
- **函数签名参数**: `(dest: &HeapRef<Vec<SteelVal>>, dest_start: usize, src: Either<&HeapRef<Vec<SteelVal>>, &SteelVector>, rest: RestArgsIter<'_, isize>,)`

Copies a range of elements from a source vector into a destination mutable vector.
Overwrites elements in `dest`, starting at `dest-start`, with elements from `src`
within the range `[src-start, src-end)`.

(vector-copy! dest dest-start src [src-start src-end]) -> void?

* dest : vector? - The destination mutable vector.
* dest-start : integer? - The starting index in the destination vector.
* src : vector? - The source vector.
* src-start : integer? - The starting index in the source vector (defaults to `0`).
* src-end : integer? - The exclusive ending index in the source vector (defaults to the length of `src`).


# Examples
```scheme
> (define A (mutable-vector 1 2 3 4 5)) ;;
> (define B (mutable-vector 10 20 30 40 50)) ;;
> (vector-copy! B 1 A 2 4) ;;
> B ;; => '#(10 3 4 40 50)
```


### `(vector-fill! ...)`

- **Rust 底层绑定**: `vector_fill`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>, element: SteelVal, rest: RestArgsIter<'_, isize>,)`

Fills a mutable vector with a specified value over a given range.

(vector-fill! vec value [start end]) -> void?

* vec : vector? - The mutable vector to modify.
* value : any? - The value to fill the vector with.
* start : integer? - The starting index of the fill range (defaults to `0`).
* end : integer? - The exclusive ending index of the fill range (defaults to the length of `vec`).

# Examples
```scheme
> (define A (mutable-vector 1 2 3 4 5)) ;;
> (vector-fill! A 9 1 4) ;;
> A ;; => '#(1 9 9 9 5)
```


### `(vector-length ...)`

- **Rust 底层绑定**: `vec_length`
- **函数签名参数**: `(v: Either<&SteelVector, &HeapRef<Vec<SteelVal>>>)`

Returns the length of the given vector.

(vector-length vec) -> integer?

* vec : vector? - The vector whose length is to be determined.

# Examples
```scheme
> (define V (immutable-vector 1 2 3 4)) ;;
> (vector-length V) ;; => 4
```


### `(vector-push ...)`

- **Rust 底层绑定**: `vector_push`
- **函数签名参数**: `(vector: &mut SteelVal, value: SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(vector-push-front ...)`

- **Rust 底层绑定**: `immutable_vector_push_front`
- **函数签名参数**: `(vector: &mut SteelVal, value: SteelVal)`

Pushes a value to the front of the vector, returning a new vector.

(vector-push-front vec val) -> immutable-vector?

* vec : immutable-vector?
* val : any?

# Examples
```scheme
> (define A (immutable-vector 1 2 3)) ;;
> (vector-push-front A 5) ;; => '#(5 1 2 3)
```


### `(vector-ref ...)`

- **Rust 底层绑定**: `vec_ref`
- **函数签名参数**: `(vec: &SteelVal, idx: &SteelVal)`

Retrieves the value at a specified index in an immutable or mutable vector.

(vector-ref vec index) -> any?

* vec : vector? - The vector from which to retrieve a value.
* index : integer? - The position in `vec` to access (must be within bounds).

# Examples
```scheme
> (define A (immutable-vector 10 20 30)) ;;
> (vector-ref A 1) ;; => 20
> (define B (mutable-vector 5 15 25)) ;;
> (vector-ref B 2) ;; => 25
```


### `(vector-set! ...)`

- **Rust 底层绑定**: `mut_vec_set`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>, i: usize, value: SteelVal)`

Sets the value at a specified index in a mutable vector.

(vector-set! vec index value) -> void?

* vec : vector? - The mutable vector to modify.
* index : integer? - The position in `vec` to update (must be within bounds).
* value : any? - The new value to store at `index`.

# Examples
```scheme
> (define A (mutable-vector 1 2 3)) ;;
> (vector-set! A 1 42) ;;
> A ;; => '#(1 42 3)
```


### `(vector-swap! ...)`

- **Rust 底层绑定**: `mut_vec_swap`
- **函数签名参数**: `(vec: &HeapRef<Vec<SteelVal>>, i: usize, j: usize)`

Swaps the value of the specified indices in a mutable vector.

(vector-swap! vec a b) -> void?

* vec : vector? - The mutable vector to modify.
* a : integer? - The first index of `vec` to swap with `b` (must be within bounds).
* b : integer? - The first index of `vec` to swap with `a` (must be within bounds).

# Examples
```scheme
> (define A (mutable-vector 1 2 3)) ;;
> (vector-swap! A 0 1) ;;
> A ;; => '#(2 1 3)
```


## 哈希表库 (Hash Maps)

提供不可变持久化哈希字典，支持 hash、hash-insert、hash-get、hash-remove、hash-contains?、hash-keys 等。在 TextCraft 中它是脚本层构造状态值的常用数据形状；状态的权威存储是 Flecs ECS（实体组件与 WorldState 单例），经 `state:*` / `ecs:*` 门面读写（见 `scheme_developer_guide.md` 2.12）。

### `(hash->list ...)`

- **Rust 底层绑定**: `hash_to_list`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns a list of the key-value pairs of a given hash map.

(hash->list map) -> (listof (cons/c any/c any/c))

* map : hash?

# Examples

```scheme
> (hash->list (hash 'a 10 'b 20)) ;; => '((a . 10) (b . 20))
```


### `(hash->vector ...)`

- **Rust 底层绑定**: `hash_to_vector`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns a list of the key-value pairs of a given hash map.

(hash->vector map) -> (vectorof (cons/c any/c any/c))

* map : hash?

# Examples

```scheme
> (hash->vector (hash 'a 10 'b 20)) ;; => '#((a . 10) (b . 20))
```


### `(hash-clear ...)`

- **Rust 底层绑定**: `clear`
- **函数签名参数**: `(hashmap: &mut SteelVal)`

Clears the entries out of the existing hashmap.
Will attempt to reuse the existing memory if there are no other references
to the hashmap.

(hash-clear map) -> hash?

* map : hash?

# Examples
```scheme
> (hash-clear (hash 'a 10 'b 20))
=> '#hash()
```


### `(hash-code ...)`

- **Rust 底层绑定**: `hash_code`
- **函数签名参数**: `(arg: &SteelVal)`

Gets the hash code for the given value;

(hash-code v) -> integer?

* v : any/c

# Examples
```scheme
(hash-code 10) ;; => 16689870864682149525
(hash-code "hello world") ;; => 12361891819228967546
```


### `(hash-contains? ...)`

- **Rust 底层绑定**: `hash_contains`
- **函数签名参数**: `(map: &Gc<HashMap<SteelVal, SteelVal>>, key: &SteelVal)`

Checks whether the given map contains the given key. Key must be hashable.

(hash-contains? map key) -> bool?

* map : hash?
* key : any/c

# Example

```scheme
> (hash-contains? (hash 'a 10 'b 20) 'a) ;; => #true
> (hash-contains? (hash 'a 10 'b 20) 'not-there) ;; => #false
```


### `(hash-empty? ...)`

- **Rust 底层绑定**: `hm_empty`
- **函数签名参数**: `(hm: &Gc<HashMap<SteelVal, SteelVal>>)`

Checks whether the hash map is empty

(hash-empty? map) -> bool?

* map : hash?

# Examples
```scheme
> (hash-empty? (hash 'a 10)) ;; => #f
> (hash-emptY? (hash)) ;; => #true
```


### `(hash-insert ...)`

- **Rust 底层绑定**: `hash_insert`
- **函数签名参数**: `(map: &mut SteelVal, key: &mut SteelVal, value: &mut SteelVal,)`

Returns a new hashmap with the additional key value pair added. Performs a functional update,
so the old hash map is still accessible.

(hash-insert map key val) -> hash?

* map : hash?
* key : any/c
* val : any/c

# Examples

```scheme
> (hash-insert (hash 'a 10 'b 20) 'c 30)
=> '#hash((a . 10) (b . 20) (c . 30))
```


### `(hash-keys->list ...)`

- **Rust 底层绑定**: `keys_to_list`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns the keys of the given hash map as a list.

(hash-keys->list map) -> (listof any/c)

* map : hash?

# Examples

```scheme
> (hash-keys->list (hash 'a 10 'b 20))
=> '(a b)
```


### `(hash-keys->vector ...)`

- **Rust 底层绑定**: `keys_to_vector`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns the keys of the given hash map as an immutable vector

(hash-keys->vector map) -> (vectorof any/c)

* map: hash?

# Examples
```scheme
> (hash-keys->vector (hash 'a 10 'b 20))
=> '#(a b)
```


### `(hash-length ...)`

- **Rust 底层绑定**: `hash_length`
- **函数签名参数**: `(map: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns the number of key value pairs in the map

(hash-length map) -> (and/c positive? int?)

* map : hash?

# Examples

```scheme
> (hash-length (hash 'a 10 'b 20)) ;; => 2
```


### `(hash-ref ...)`

- **Rust 底层绑定**: `hash_ref`
- **函数签名参数**: `(map: &Gc<HashMap<SteelVal, SteelVal>>, key: &SteelVal)`

Gets the `key` from the given `map`. Raises an error if the key does not exist. `hash-get` is an alias for this.

(hash-ref map key) -> any/c

* map : hash?
* key : any/c

# Examples

```scheme
> (hash-ref (hash 'a 10 'b 20) 'b) ;; => 20
```


### `(hash-remove ...)`

- **Rust 底层绑定**: `hash_remove`
- **函数签名参数**: `(map: &mut SteelVal, key: SteelVal)`

Returns a new hashmap with the given key removed. Performs a functional
update, so the old hash map is still available with the original key value pair.

(hash-remove map key) -> hash?

* map : hash?
* key : any/c

# Examples

```scheme
> (hash-remove (hash 'a 10 'b 20) 'a)
=> '#hash(('b . 20))
```


### `(hash-try-get ...)`

- **Rust 底层绑定**: `hash_try_get`
- **函数签名参数**: `(map: &Gc<HashMap<SteelVal, SteelVal>>, key: &SteelVal)`

Gets the `key` from the given `map`. Returns #false if the key does not exist.

(hash-try-get map key) -> (or any/c #false)

* map : hash?
* key : any/c

# Examples

```scheme
> (hash-try-get (hash 'a 10 'b 20) 'b) ;; => 20
> (hash-try-get (hash 'a 10 'b 20) 'does-not-exist) ;; => #false
```


### `(hash-union ...)`

- **Rust 底层绑定**: `hm_union`
- **函数签名参数**: `(mut hml: &mut SteelVal, mut hmr: &mut SteelVal)`

Constructs the union of two hashmaps, keeping the values
in the left map when the keys exist in both maps.

Will reuse memory where possible.

(hash-union l r) -> hash?

# Examples
```scheme
> (hash-union (hash 'a 10) (hash 'b 20)) ;; => '#hash((a . 10) (b . 20))
```


### `(hash-values->list ...)`

- **Rust 底层绑定**: `values_to_list`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns the values of the given hash map as a list

(hash-values->list map) -> (listof any/c)

* map : hash?

# Examples

```scheme
> (hash-values->list (hash 'a 10 'b 20))
=> '(10 20)
```


### `(hash-values->vector ...)`

- **Rust 底层绑定**: `values_to_vector`
- **函数签名参数**: `(hashmap: &Gc<HashMap<SteelVal, SteelVal>>)`

Returns the values of the given hash map as an immutable vector

(hash-values->vector map) -> (vectorof any/c)?

* map : hash?

# Examples

```scheme
> (hash-values->vector (hash 'a 10 'b 20))
=> '#(10 20)
```


## 哈希集合库 (Hash Sets)

提供哈希集合数据结构，支持 hashset、hashset-insert、hashset-contains?、集合并交差运算等。

### `(hashset->immutable-vector ...)`

- **Rust 底层绑定**: `hashset_to_immutable_vector`
- **函数签名参数**: `(hashset: &SteelHashSet)`

Creates an immutable vector from this hashset. The order of the vector is not guaranteed.

# Examples
```scheme
(hashset->immutable-vector (hashset 10 20 30)) ;; => '#(10 20 30)
(hashset->immutable-vector (hashset 10 20 30)) ;; => '#(20 10 30)
```


### `(hashset->list ...)`

- **Rust 底层绑定**: `hashset_to_list`
- **函数签名参数**: `(hashset: &SteelHashSet)`

Creates a list from this hashset. The order of the list is not guaranteed.

# Examples
```scheme
(hashset->list (hashset 10 20 30)) ;; => '(10 20 30)
(hashset->list (hashset 10 20 30)) ;; => '(20 10 30)
```


### `(hashset-clear ...)`

- **Rust 底层绑定**: `hashset_clear`
- **函数签名参数**: `(hashset: &mut SteelVal)`

Clears the hashset and returns the passed in hashset.
This first checks if there are no other references to this hashset,
and if there aren't, clears that allocation. Given that there are
functional updates, this is only relevant if there are no more
references to a given hashset, and you want to reuse its allocation.


### `(hashset-contains? ...)`

- **Rust 底层绑定**: `hashset_contains`
- **函数签名参数**: `(hashset: &SteelHashSet, key: &SteelVal)`

Test if the hashset contains a given element.

# Examples
```scheme
(hashset-contains? (hashset 10 20) 10) ;; => #true
(hashset-contains? (hashset 10 20) "foo") ;; => #false
```


### `(hashset-difference ...)`

- **Rust 底层绑定**: `hashset_difference`
- **函数签名参数**: `(l: &SteelHashSet, r: &SteelHashSet)`

Finds the difference between the two hash sets.

# Examples
```scheme
(hashset-difference (hashset 10 20 30) (hashset 20 30 40)) ;; => (hashset 40 10)
```


### `(hashset-insert ...)`

- **Rust 底层绑定**: `hs_insert`
- **函数签名参数**: `(hashset: &mut SteelVal, value: SteelVal)`

Insert a new element into the hashset. Returns a hashset.

# Examples
```scheme
(define hs (hashset 10 20 30))
(define updated (hashset-insert hs 40))
(equal? hs (hashset 10 20 30)) ;; => #true
(equal? updated (hashset 10 20 30 40)) ;; => #true
```


### `(hashset-intersection ...)`

- **Rust 底层绑定**: `hashset_intersection`
- **函数签名参数**: `(l: &SteelHashSet, r: &SteelHashSet)`

Finds the intersection between the two hash sets.

# Examples
```scheme
(hashset-intersection (hashset 10 20) (hashset 20)) ;; => (hashset 10)
```


### `(hashset-length ...)`

- **Rust 底层绑定**: `hashset_length`
- **函数签名参数**: `(hashset: &SteelHashSet)`

Get the number of elements in the hashset

# Examples
```scheme
(hashset-length (hashset 10 20 30)) ;; => 3
```


### `(hashset-subset? ...)`

- **Rust 底层绑定**: `hashset_is_subset`
- **函数签名参数**: `(left: &SteelHashSet, right: &SteelHashSet)`

Check if the left set is a subset of the right set

# Examples
```scheme
(hashset-subset? (hash 10) (hashset 10 20)) ;; => #true
(hashset-subset? (hash 100) (hashset 30)) ;; => #false
```


### `(hashset-union ...)`

- **Rust 底层绑定**: `hashset_union`
- **函数签名参数**: `(l: &SteelHashSet, r: &SteelHashSet)`

Finds the union between the two hash sets.

# Examples
```scheme
(hashset-union (hashset 10) (hashset 20)) ;; => (hashset 10 20)
```


### `(list->hashset ...)`

- **Rust 底层绑定**: `list_to_hashset`
- **函数签名参数**: `(l: &List<SteelVal>)`

Convert the given list into a hashset.

# Examples
```scheme
(list 10 20 30) ;; => (hashset 10 20 30)
```


## 字节向量库 (Bytevectors)

提供二进制原始字节序列存储与操作，支持 bytevector、bytes->string、string->bytes 等网络与序列化底层支持。

### `(byte? ...)`

- **Rust 底层绑定**: `is_byte`
- **函数签名参数**: `(value: &SteelVal)`

Returns `#t` if the given value is a byte, meaning an exact
integer between 0 and 255 inclusive, `#f` otherwise.

# Examples
```scheme
(byte? 65) ;; => #t
(byte? 0) ;; => #t
(byte? 256) ;; => #f
(byte? 100000) ;; => #f
(byte? -1) ;; => #f
```


### `(bytes->list ...)`

- **Rust 底层绑定**: `bytes_to_list`
- **函数签名参数**: `(value: &SteelByteVector)`

Converts the bytevector to the equivalent list representation.

# Examples
```scheme
(bytes->list (bytes 0 1 2 3 4 5)) ;; => '(0 1 2 3 4 5)
```


### `(bytes->string/utf8 ...)`

- **Rust 底层绑定**: `bytes_to_string`
- **函数签名参数**: `(value: &SteelByteVector, mut rest: RestArgsIter<'_, isize>,)`

Decodes a string from a bytevector containing valid UTF-8.

(bytes->string/utf8 buf [start] [end]) -> string?

* buf : bytes?
* start: int? = 0
* end: int? = (bytes-length buf)

# Examples
```scheme
(bytes->string/utf8 (bytes #xe5 #x8d #x83 #xe8 #x91 #x89)) ;; => "千葉"
```


### `(bytes-append ...)`

- **Rust 底层绑定**: `bytes_append`
- **函数签名参数**: `(mut rest: RestArgsIter<'_, &SteelByteVector>)`

Append multiple byte vectors into a new bytevector.

# Examples
```scheme
(bytes-append #u8(0 1 2) #u8(3 4 5)) ;; => #u8(#x00 #x01 #x02 #x03 #x04 #x05)

(bytes-append #u8(0) #u8(1) #u8() #u8(2)) ;; => #u8(#x00 #x01 #x02)
```


### `(bytes-clear! ...)`

- **Rust 底层绑定**: `bytes_clear`
- **函数签名参数**: `(value: &mut SteelByteVector)`

Removes all bytes from the given bytevector, leaving it empty.

(bytes-clear! vector)

* vector : bytes?

# Examples
```scheme
(define my-bytes (bytes 0 1 2 3))
(bytes-clear! my-bytes)
my-bytes ;; => #u8()
```


### `(bytes-copy ...)`

- **Rust 底层绑定**: `bytevector_copy_new`
- **函数签名参数**: `(bytevector: &SteelByteVector, mut rest: RestArgsIter<'_, isize>,)`

Creates a copy of a bytevector.

(bytevector-copy vector [start end]) -> bytes?

* vector : bytes?
* start: int? = 0
* end: int? = (bytes-length vector)

# Examples

```scheme
(define vec (bytes 1 2 3 4 5))

(bytes-copy vec) ;; => (bytes 1 2 3 4 5)
(bytes-copy vec 1 3) ;; => (bytes 2 3)
```


### `(bytes-length ...)`

- **Rust 底层绑定**: `bytes_length`
- **函数签名参数**: `(value: &SteelByteVector)`

Returns the length of the given byte vector

# Examples
```scheme
(bytes-length (bytes 1 2 3 4 5)) ;; => 5
```


### `(bytes-push! ...)`

- **Rust 底层绑定**: `bytes_push`
- **函数签名参数**: `(value: &mut SteelByteVector, byte: u8)`

Appends a byte to the end of the given bytevector, growing it in place.

(bytes-push! vector byte)

* vector : bytes?
* byte : byte?

# Examples
```scheme
(define my-bytes (bytes 0 1 2))
(bytes-push! my-bytes 3)
my-bytes ;; =>  #u8(#x00 #x01 #x02 #x03)
```


### `(bytes-ref ...)`

- **Rust 底层绑定**: `bytes_ref`
- **函数签名参数**: `(value: &SteelByteVector, index: usize)`

Fetches the byte at the given index within the bytevector.
If the index is out of bounds, this will error.

(bytes-ref vector index)

* vector : bytes?
* index: (and exact? int?)

# Examples
```scheme
(bytes-ref (bytes 0 1 2 3 4 5) 3) ;; => 4
(bytes-ref (bytes) 10) ;; error
```


### `(bytes-set! ...)`

- **Rust 底层绑定**: `bytes_set`
- **函数签名参数**: `(value: &mut SteelByteVector, index: usize, byte: u8)`

Sets the byte at the given index to the given byte. Will error
if the index is out of bounds.

(bytes-set! vector index byte)

* vector : bytes?
* index: (and exact? int?)
* byte: byte?

# Examples
```scheme
(define my-bytes (bytes 0 1 2 3 4 5))
(bytes-set! my-bytes 0 100)
(bytes-ref my-bytes 0) ;; => 100
```


### `(bytes? ...)`

- **Rust 底层绑定**: `is_bytes`
- **函数签名参数**: `(arg: &SteelVal)`

Returns `#t` if this value is a bytevector

# Examples
```scheme
(bytes? (bytes 0 1 2)) ;; => #t
(bytes? (list 10 20 30)) ;; => #f
```


### `(list->bytes ...)`

- **Rust 底层绑定**: `list_to_bytes`
- **函数签名参数**: `(value: Vec<u8>)`

Converts the list of bytes to the equivalent bytevector representation.
The list must contain _only_ values which satisfy the `byte?` predicate,
otherwise this function will error.

# Examples
```scheme
(list->bytes (list 0 1 2 3 4 5)) ;; => (bytes 0 1 2 3 4 5)
```


### `(make-bytes ...)`

- **Rust 底层绑定**: `make_bytes`
- **函数签名参数**: `(k: usize, mut c: RestArgsIter<'_, isize>)`

Creates a bytevector given a length and a default value.

(make-bytes len default) -> bytes?

* len : int?
* default : byte?

# Examples
```scheme
(make-bytes 6 42) ;; => (bytes 42 42 42 42 42)
```


## 符号库 (Symbols)

提供 Scheme 驻留符号（Interned Symbols）与非驻留符号的操作与断言。

### `(symbol=? ...)`

- **Rust 底层绑定**: `symbol_equals`
- **函数签名参数**: `(mut iter: RestArgsIter<SteelSymbol<'_>>)`

Compares one or more symbols for pointer‐identity equality.

(symbol=? sym1 sym2 …) -> bool?

* `sym1` : symbol? — the first symbol to compare
* `sym2` : symbol? — the next symbol to compare, and so on

Returns `#t` if all provided symbols share the same memory pointer,
`#f` otherwise. At least one argument is required.

# Examples
```scheme
> (define a 'foo)
> (define b 'foo)
> (symbol=? a b)
=> #t
> (symbol=? 'a 'b)
=> #f
> (symbol=? 'x 'x 'x)
=> #t
```


## 端口与流式 I/O (Ports & I/O)

提供标准输入输出（stdin, stdout, stderr）、字符串端口（open-input-string, open-output-string）、文件流与格式化读取器。

### `(#%default-error-port ...)`

- **Rust 底层绑定**: `default_error_port`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%default-input-port ...)`

- **Rust 底层绑定**: `default_input_port`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%default-output-port ...)`

- **Rust 底层绑定**: `default_output_port`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%file-input-port? ...)`

- **Rust 底层绑定**: `is_file_input`
- **函数签名参数**: `(maybe_port: &SteelVal)`

Checks if a given value is a file input port

(file-input-port? any/c) -> bool?

# Examples

```scheme
> (file-input-port? (stdin)) ;; => #false
> (file-input-port? (open-input-file "foo.scm")) ;; => #true
```


### `(#%peek-byte ...)`

- **Rust 底层绑定**: `peek_byte`
- **函数签名参数**: `(port: &SteelPort)`

Peeks the next byte from an input port.

(peek-byte [port]) -> byte?

* port : input-port? = (current-input-port)


### `(#%peek-char ...)`

- **Rust 底层绑定**: `peek_char`
- **函数签名参数**: `(port: &SteelPort)`

Peeks the next character from an input port.

(peek-char [port]) -> char?

* port : input-port? = (current-input-port)


### `(#%raw-display ...)`

- **Rust 底层绑定**: `display`
- **函数签名参数**: `(line: &SteelVal, port: &SteelPort)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%raw-write ...)`

- **Rust 底层绑定**: `write`
- **函数签名参数**: `(line: &SteelVal, port: &SteelPort)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%raw-write-char ...)`

- **Rust 底层绑定**: `write_char`
- **函数签名参数**: `(character: char, port: &SteelPort)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%raw-write-string ...)`

- **Rust 底层绑定**: `write_string`
- **函数签名参数**: `(line: &SteelVal, port: &SteelPort)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%read-byte ...)`

- **Rust 底层绑定**: `read_byte`
- **函数签名参数**: `(port: &SteelPort)`

Reads a single byte from an input port.

(read-byte [port]) -> byte?

* port : input-port? = (current-input-port)


### `(#%read-bytes ...)`

- **Rust 底层绑定**: `read_bytes`
- **函数签名参数**: `(amt: usize, port: &SteelPort)`

Reads bytes from an input port.

(read-bytes amt [port]) -> bytes?

* amt : (and positive? int?)
* port : input-port? = (current-input-port)


### `(#%read-bytes! ...)`

- **Rust 底层绑定**: `read_bytes_into`
- **函数签名参数**: `(buf: &SteelByteVector, port: &SteelPort, start: usize, end: usize,)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%read-char ...)`

- **Rust 底层绑定**: `read_char`
- **函数签名参数**: `(port: &SteelPort)`

Reads the next character from an input port.

(read-char [port]) -> char?

* port : input-port? = (current-input-port)


### `(#%read-line ...)`

- **Rust 底层绑定**: `read_line`
- **函数签名参数**: `(port: &SteelPort)`

Reads a line from an input port.

(read-line [port]) -> string?

* port : input-port? = (current-input-port)


### `(#%read-port-to-bytes ...)`

- **Rust 底层绑定**: `read_port_to_bytes`
- **函数签名参数**: `(port: &SteelPort)`

Reads the entire content of an input port into a byte vector.

(read-port-to-bytes [port]) -> string?

* [port] : input-port? = (current-input-port)


### `(#%read-port-to-string ...)`

- **Rust 底层绑定**: `read_port_to_string`
- **函数签名参数**: `(port: &SteelPort)`

Reads the entire content of an input port into a string.

(read-port-to-string [port]) -> string?

* [port] : input-port? = (current-input-port)


### `(#%string-input-port? ...)`

- **Rust 底层绑定**: `is_string_input`
- **函数签名参数**: `(maybe_port: &SteelVal)`

Checks if a given value is a string input port

(string-input-port? any/c) -> bool?

# Examples

```scheme
> (string-input-port? (stdin)) ;; => #false
> (string-input-port? (open-input-string "foo")) ;; => #true
```


### `(#%write-byte ...)`

- **Rust 底层绑定**: `write_byte`
- **函数签名参数**: `(byte: u8, port: &SteelPort)`

Writes a single byte to an output port.

(write-byte b [port])

* b : byte?
* port : output-port? = (current-output-port)


### `(#%write-bytes ...)`

- **Rust 底层绑定**: `write_bytes`
- **函数签名参数**: `(bytes: &SteelByteVector, port: &SteelPort)`

Writes the contents of a bytevector into an output port.

(write-bytes buf [port])

* buf : bytes?
* port : output-port? = (current-output-port)


### `(close-input-port ...)`

- **Rust 底层绑定**: `close_input_port`
- **函数签名参数**: `(port: &SteelPort)`

Close an input port. If the port is a file, the file will be closed.

(close-input-port input-port?) -> void


### `(close-output-port ...)`

- **Rust 底层绑定**: `close_output_port`
- **函数签名参数**: `(port: &SteelPort)`

Close an output port. If the port is a file, the file will be closed.

(close-output-port output-port?) -> void


### `(close-port ...)`

- **Rust 底层绑定**: `close_port`
- **函数签名参数**: `(port: &SteelPort)`

Close a port. If the port is a file, the file will be closed.

(close-port port?) -> void


### `(eof-object ...)`

- **Rust 底层绑定**: `eof_object`

Returns an EOF object.

(eof-object) -> eof-object?


### `(eof-object? ...)`

- **Rust 底层绑定**: `eof_objectp`
- **函数签名参数**: `(value: &SteelVal)`

Returns `#t` if the value is an EOF object.

(eof-object? any/c) -> bool?


### `(flush-output-port ...)`

- **Rust 底层绑定**: `flush_output_port`
- **函数签名参数**: `(port: &SteelPort)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(get-output-bytevector ...)`

- **Rust 底层绑定**: `get_output_bytevector`
- **函数签名参数**: `(port: &SteelPort)`

Extracts the contents from a port created with `open-output-bytevector`.

(get-output-bytevector port?) -> bytes?


### `(get-output-string ...)`

- **Rust 底层绑定**: `get_output_string`
- **函数签名参数**: `(port: &SteelPort)`

Extracts the string contents from a port created with `open-output-string`.

(get-output-string port?) -> string?


### `(input-port? ...)`

- **Rust 底层绑定**: `is_input`
- **函数签名参数**: `(maybe_port: &SteelVal)`

Checks if a given value is an input port

(input-port? any/c) -> bool?

# Examples

```scheme
> (input-port? (stdin)) ;; => #true
> (input-port? "foo") ;; => #false
```


### `(open-input-bytevector ...)`

- **Rust 底层绑定**: `open_input_bytevector`
- **函数签名参数**: `(bytes: &SteelByteVector)`

Creates an input port from a bytevector, that will return the bytevector contents.

(open-input-bytevector bytes?) -> input-port?


### `(open-input-file ...)`

- **Rust 底层绑定**: `open_input_file`
- **函数签名参数**: `(path: &SteelString)`

Takes a filename `path` referring to an existing file and returns an input port. Raises an error
if the file does not exist

(open-input-file string?) -> input-port?

# Examples
```scheme
> (open-input-file "foo-bar.txt") ;; => #<input-port:foo-bar.txt>
> (open-input-file "file-does-not-exist.txt")
error[E08]: Io
┌─ :1:2
│
1 │ (open-input-file "foo-bar.txt")
│  ^^^^^^^^^^^^^^^ No such file or directory (os error 2)
```


### `(open-input-file ...)`

- **Rust 底层绑定**: `open_input_file_sandboxed`
- **函数签名参数**: `(_: &SteelString)`

Takes a filename `path` referring to an existing file and returns an input port. Raises an error
if the file does not exist

(open-input-file string?) -> input-port?

# Examples
```scheme
> (open-input-file "foo-bar.txt") ;; => #<input-port:foo-bar.txt>
> (open-input-file "file-does-not-exist.txt")
error[E08]: Io
┌─ :1:2
│
1 │ (open-input-file "foo-bar.txt")
│  ^^^^^^^^^^^^^^^ No such file or directory (os error 2)
```


### `(open-input-string ...)`

- **Rust 底层绑定**: `open_input_string`
- **函数签名参数**: `(s: &SteelString)`

Creates an input port from a string, that will return the string contents.

(open-input-string string?) -> input-port?


### `(open-output-bytevector ...)`

- **Rust 底层绑定**: `open_output_bytevector`

Creates an output port that accumulates what is written into a bytevector.
These bytes can be recovered calling `get-output-bytevector`.

(open-output-bytevector) -> output-port?

# Examples
```scheme
(define out (open-output-bytevector))


(write-byte 30 out)
(write-byte 250 out)

(get-output-bytevector out) ;; => (bytes 30 250)
```


### `(open-output-file ...)`

- **Rust 底层绑定**: `open_output_file_sandboxed`
- **函数签名参数**: `(_: &SteelString)`

Takes a filename `path` referring to a file to be created and returns an output port.

(open-output-file string?) -> output-port?

# Examples
```scheme
> (open-output-file "foo-bar.txt") ;; => #<output-port:foo-bar.txt>
```


### `(open-output-string ...)`

- **Rust 底层绑定**: `open_output_string`

Creates an output port that accumulates what is written into a string.
This string can be recovered calling `get-output-string`.

(open-output-string) -> output-port?

# Examples
```scheme
(define out (open-output-string))


(write-char "α" out)
(write-char "ω" out)

(get-output-string out) ;; => "αω"
```


### `(output-port? ...)`

- **Rust 底层绑定**: `is_output`
- **函数签名参数**: `(maybe_port: &SteelVal)`

Checks if a given value is an output port

(output-port? any/c) -> bool?

# Examples

```scheme
> (define output (open-output-file "foo.txt"))
> (output-port? output) ;; => #true
```


### `(read-line-from-port ...)`

- **Rust 底层绑定**: `read_line_from_port`
- **函数签名参数**: `(port: &SteelPort)`

Reads a line from the given port, including the '\n' at the end.

Use of this procedure is discouraged in favor of the (read-line) procedure,
which is included in the scheme spec and therefore more portable.

(read-line-from-port port?) -> string?


### `(stdin ...)`

- **Rust 底层绑定**: `open_stdin`

Gets the port handle to stdin

(stdin) -> input-port?

# Examples

```scheme
> (stdin) ;; => #<input-port:stdin>
```


### `(stdout ...)`

- **Rust 底层绑定**: `open_stdout`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(would-block ...)`

- **Rust 底层绑定**: `would_block_object`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(would-block-object? ...)`

- **Rust 底层绑定**: `would_block_objectp`
- **函数签名参数**: `(value: &SteelVal)`

Returns `#t` if the value is an EOF object.

(eof-object? any/c) -> bool?


### `(write-line! ...)`

- **Rust 底层绑定**: `write_line`
- **函数签名参数**: `(port: &SteelPort, line: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


## 文件系统与路径 (Filesystem & Path)

提供跨平台文件读写、路径合并、目录遍历、文件元数据获取（如 canonicalize, glob, path-exists?）。引入模块：`(require-builtin steel/filesystem)`。

### `(canonicalize-path ...)`

- **Rust 底层绑定**: `canonicalize_path`
- **函数签名参数**: `(path: &SteelString)`

Returns canonical path with all components normalized.

(canonicalize-path path) -> string?

* path : string? - The path to canonicalize.

# Examples
```scheme
> (canonicalize-path "logs") ;; => "/Users/me/Desktop/programming/logs"
> (canonicalize-path "logs/today.json") ;; => "/Users/me/Desktop/programming/logs/today.json"
```


### `(change-current-directory! ...)`

- **Rust 底层绑定**: `change_current_directory`
- **函数签名参数**: `(path: &SteelString)`

Change the current working directory

(change-current-directory! path) -> void?

* path : (string?) - The directory to switch to

# Examples
```scheme
> (change-current-directory! "logs") ;;
> (change-current-directory! "..") ;;
```


### `(copy-directory-recursively! ...)`

- **Rust 底层绑定**: `copy_directory_recursively`
- **函数签名参数**: `(source: &SteelString, destination: &SteelString,)`

Recursively copies the contents of the source directory to the destination.

(copy-directory-recursively! source destination) -> void?

* source : string? - The directory to copy.
* destination : string? - The destination directory into which to copy.

# Examples
```scheme
> (copy-directory-recursively! "logs" "backup") ;;
```


### `(create-directory! ...)`

- **Rust 底层绑定**: `create_directory`
- **函数签名参数**: `(directory: &SteelString)`

Creates the directory

(create-directory! dir) -> void?

* dir : (string?) - The directory name to create.

# Examples
```scheme
> (create-directory! "logs") ;;
```


### `(current-directory ...)`

- **Rust 底层绑定**: `current_directory`

Outputs the current working directory as a string

(current-directory) -> string?

# Examples
```scheme
> (current-directory) ;; => "/Users/me/Desktop/programming"
```


### `(delete-directory! ...)`

- **Rust 底层绑定**: `delete_directory`
- **函数签名参数**: `(directory: &SteelString)`

Deletes the directory

(delete-directory! dir) -> void?

* dir : (string?) - The directory name to delete.

# Examples
```scheme
> (delete-directory! "logs") ;;
```


### `(delete-file! ...)`

- **Rust 底层绑定**: `delete_file`
- **函数签名参数**: `(file: &SteelString)`

Deletes the file

(delete-file! path) -> void?

* path : (string?) - The file to delete

# Examples
```scheme
> (delete-file! "logs/today.json") ;;
```


### `(file-metadata ...)`

- **Rust 底层绑定**: `file_metadata`
- **函数签名参数**: `(path: &SteelString)`

Access the file metadata for a given path


### `(file-name ...)`

- **Rust 底层绑定**: `file_name`
- **函数签名参数**: `(path: &SteelString)`

Gets the filename for a given path.

(file-name path) -> string?

* path : string? - The path from which to get the file name.

# Examples
```scheme
> (file-name "logs") ;; => "logs"
> (file-name "logs/today.json") ;; => "today.json"
```


### `(fs-metadata-accessed ...)`

- **Rust 底层绑定**: `fs_metadata_accessed`
- **函数签名参数**: `(value: &SteelVal)`

Get the last accessed time from the file metadata


### `(fs-metadata-created ...)`

- **Rust 底层绑定**: `fs_metadata_created`
- **函数签名参数**: `(value: &SteelVal)`

Get the created time from the file metadata


### `(fs-metadata-is-dir? ...)`

- **Rust 底层绑定**: `fs_metadata_is_dir`
- **函数签名参数**: `(value: &SteelVal)`

Check if this metadata is from a directory


### `(fs-metadata-is-file? ...)`

- **Rust 底层绑定**: `fs_metadata_is_file`
- **函数签名参数**: `(value: &SteelVal)`

Check if this metadata is from a file


### `(fs-metadata-is-symlink? ...)`

- **Rust 底层绑定**: `fs_metadata_is_symlink`
- **函数签名参数**: `(value: &SteelVal)`

Check if this metadata is from a symlink


### `(fs-metadata-len ...)`

- **Rust 底层绑定**: `fs_metadata_len`
- **函数签名参数**: `(value: &SteelVal)`

Get the length of the file in bytes


### `(fs-metadata-modified ...)`

- **Rust 底层绑定**: `fs_metadata_modified`
- **函数签名参数**: `(value: &SteelVal)`

Get the last modified time from the file metadata


### `(fs-metadata? ...)`

- **Rust 底层绑定**: `is_fs_metadata`
- **函数签名参数**: `(value: &SteelVal)`

Checks if this value is a #<Metadata>


### `(glob ...)`

- **Rust 底层绑定**: `glob`
- **函数签名参数**: `(pattern: SteelString, mut rest: RestArgsIter<'_, &SteelVal>)`

Returns an iterator over the paths matching the given glob pattern. An
optional second argument supplies match options. Use `glob-iter-next!` to
advance the resulting iterator.

(glob pattern [options]) -> glob-iterator?

* pattern : string?
* options : match-options? - optional matching options

```scheme
> (define paths (glob "src/*.rs"))
> (glob-iter-next! paths) ;; => the first matching path
```


### `(glob-iter-next! ...)`

- **Rust 底层绑定**: `glob_paths_next`
- **函数签名参数**: `(paths: &SteelVal)`

Advances a glob iterator created by `glob`, returning the next matching
path, or `#false` once the iterator has been exhausted.

(glob-iter-next! paths) -> (or path? #false)

* paths : glob-iterator?


### `(is-dir? ...)`

- **Rust 底层绑定**: `is_dir`
- **函数签名参数**: `(path: &SteelString)`

Checks if a path is a directory.

(is-dir? path) -> bool?

* path : string? - The path to check.

# Examples
```scheme
> (is-dir? "logs") ;; => #true
> (is-dir? "logs/today.json") ;; => #false
```


### `(is-file? ...)`

- **Rust 底层绑定**: `is_file`
- **函数签名参数**: `(path: &SteelString)`

Checks if a path is a file.

(is-file? path) -> bool?

* path : string? - The path to check.

# Examples
```scheme
> (is-file? "logs") ;; => #false
> (is-file? "logs/today.json") ;; => #true
```


### `(parent-name ...)`

- **Rust 底层绑定**: `parent_name`
- **函数签名参数**: `(path: &SteelString)`

Gets the parent directory name for a given path.

(parent-name path) -> string?

* path : string? - The path from which to get the parent.

# Examples
```scheme
> (parent-name "logs") ;; => ""
> (parent-name "logs/today.json") ;; => "logs"
```


### `(path->extension ...)`

- **Rust 底层绑定**: `get_extension`
- **函数签名参数**: `(path: &SteelString)`

Gets the extension from a path.

(path->extension path) -> (or/c string? void?)

* path : string? - The path from which to get the extension.

# Examples
```scheme
> (path->extension "logs") ;; => void
> (path->extension "logs/today.json") ;; => ".json"
```


### `(path->string ...)`

- **Rust 底层绑定**: `path_to_string`
- **函数签名参数**: `(path: &SteelVal)`

Converts a path into its string representation.

(path->string path) -> string?

* path : path?


### `(path-exists? ...)`

- **Rust 底层绑定**: `path_exists`
- **函数签名参数**: `(path: &SteelString)`

Checks if a path exists.

(path-exists? path) -> bool?

* path : (string?) - The path to check

# Examples
```scheme
> (path-exists? "logs") ;; => #true
> (path-exists? "backup/logs") ;; => #false
```


### `(read-dir ...)`

- **Rust 底层绑定**: `read_dir`
- **函数签名参数**: `(path: &SteelString)`

Returns the contents of the directory as a list

(read-dir path) -> list?

* path : string? - The path to the directory.

# Examples
```scheme
> (read-dir "logs") ;; => '("logs/today.json" "logs/yesterday.json")
> (read-dir "empty_dir") ;; => '()
```


### `(read-dir-entry-file-name ...)`

- **Rust 底层绑定**: `read_dir_entry_file_name`
- **函数签名参数**: `(value: &SteelVal)`

Returns the file name from a given read-dir-entry.


### `(read-dir-entry-is-dir? ...)`

- **Rust 底层绑定**: `read_dir_entry_is_dir`
- **函数签名参数**: `(value: &SteelVal)`

Checks whether the read dir entry is a directory.

(read-dir-entry-is-dir? value) -> bool?

* value : read-dir-iter-entry?

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(define next (read-dir-iter-next! my-iter))

(read-dir-entry-path) ;; => "src/lib.rs"
(read-dir-entry-is-dir? next) ;; #false - because this is a file

```


### `(read-dir-entry-is-file? ...)`

- **Rust 底层绑定**: `read_dir_entry_is_file`
- **函数签名参数**: `(value: &SteelVal)`

Checks whether the read dir entry is a file.

(read-dir-entry-is-dir? value) -> bool?

* value : read-dir-iter-entry?

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(define next (read-dir-iter-next! my-iter))

(read-dir-entry-path) ;; => "src/lib.rs"
(read-dir-entry-is-dir? next) ;; #true - because this is a file

```


### `(read-dir-entry-is-symlink? ...)`

- **Rust 底层绑定**: `read_dir_entry_is_symlink`
- **函数签名参数**: `(value: &SteelVal)`

Checks whether the read dir entry is a symlink.


### `(read-dir-entry-metadata ...)`

- **Rust 底层绑定**: `read_dir_entry_metadata`
- **函数签名参数**: `(value: &SteelVal)`

Extract the file metadata from the #<DirEntry>


### `(read-dir-entry-path ...)`

- **Rust 底层绑定**: `read_dir_entry_path`
- **函数签名参数**: `(value: &SteelVal)`

Returns the path from a given read-dir-entry.


### `(read-dir-iter ...)`

- **Rust 底层绑定**: `read_dir_iter`
- **函数签名参数**: `(directory: &SteelString)`

Creates an iterator over the contents of the given directory.
The given path must be a directory.

(read-dir-iter dir) -> #<ReadDir>

* dir : (is-dir?) - the directory to iterate over

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(read-dir-iter-next! my-iter) ;; => #<DirEntry> src/lib.rs
(read-dir-iter-next! my-iter) ;; => #<DirEntry> src/main.rs
(read-dir-iter-next! my-iter) ;; => #false
```


### `(read-dir-iter-entry? ...)`

- **Rust 底层绑定**: `is_read_dir_iter_entry`
- **函数签名参数**: `(value: &SteelVal)`

Checks whether the given value is a #<DirEntry>

(read-dir-iter-entry? value) -> bool?

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(define next (read-dir-iter-next! my-iter))
(read-dir-iter-entry? next) ;; => #true
```


### `(read-dir-iter-next! ...)`

- **Rust 底层绑定**: `read_dir_iter_next`
- **函数签名参数**: `(iter: &SteelVal)`

Reads one entry from the iterator. Reads a `ReadDir` struct.

(read-dir-iter-next! read-dir-iter) -> #<DirEntry>

* dir : (read-dir-iter?) - the directory to iterate over

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(define nex-entry (read-dir-iter-next! my-iter)) ;; => #<DirEntry> src/lib.rs
(read-dir-entry-is-dir? next-entry) ;; => #false
(read-dir-entry-is-file? next-entry) ;; => #true
(read-dir-entry-file-name) ;; => "lib.rs"
```


### `(read-dir-iter? ...)`

- **Rust 底层绑定**: `is_read_dir`
- **函数签名参数**: `(value: &SteelVal)`

Checks whether the given value is a #<ReadDir>

(read-dir-iter? value) -> bool?

# Examples
```scheme
(define my-iter (read-dir-iter "src"))
(read-dir-iter? my-iter) ;; => #true
(read-dir-iter "not an iter") ;; => #false
```


### `(rename-file-or-directory! ...)`

- **Rust 底层绑定**: `rename_file_or_directory`
- **函数签名参数**: `(source: &SteelString, destination: &SteelString,)`

Renames a file or directory, replacing any data at the destination.

(rename-file-or-directory! source destination) -> void?

* source : (string?) - The file or directory to rename.
* destination : (string?) - The destination to which to move the file or directory.

# Examples
```scheme
> (rename-file-or-directory! "logs/today.json" "logs/tomorrow.json") ;;
> (rename-file-or-directory! "logs" "backup") ;;
```


## 时间与基准 (Time & Duration)

提供高精度时间测量、持续时间、格式化等。引入模块：`(require-builtin steel/time)`。

### `(current-inexact-milliseconds ...)`

- **Rust 底层绑定**: `current_inexact_milliseconds`

Returns the number of milliseconds since the Unix epoch as an inexact number.

(current-inexact-milliseconds) -> inexact?


### `(current-milliseconds ...)`

- **Rust 底层绑定**: `current_milliseconds`

Returns the number of milliseconds since the Unix epoch as an integer.

(current-milliseconds) -> int?


### `(current-second ...)`

- **Rust 底层绑定**: `current_seconds`

Returns the number of seconds since the Unix epoch as an integer.

(current-second) -> int?


### `(duration->micros ...)`

- **Rust 底层绑定**: `duration_to_micros`
- **函数签名参数**: `(duration: Duration)`

Returns the total number of whole microseconds contained in the given duration.

(duration->micros dur) -> int?

* dur : duration?


### `(duration->millis ...)`

- **Rust 底层绑定**: `duration_to_millis`
- **函数签名参数**: `(duration: Duration)`

Returns the total number of whole milliseconds contained in the given duration.

(duration->millis dur) -> int?

* dur : duration?


### `(duration->nanos ...)`

- **Rust 底层绑定**: `duration_to_nanos`
- **函数签名参数**: `(duration: Duration)`

Returns the total number of nanoseconds contained in the given duration.

(duration->nanos dur) -> int?

* dur : duration?


### `(duration->seconds ...)`

- **Rust 底层绑定**: `duration_to_seconds`
- **函数签名参数**: `(duration: Duration)`

Returns the number of whole seconds contained in the given duration.

(duration->seconds dur) -> int?

* dur : duration?


### `(duration->string ...)`

- **Rust 底层绑定**: `duration_to_string`
- **函数签名参数**: `(duration: Duration)`

Returns a string representation of a duration

(duration->string dur)

* dur : duration?


### `(duration-since ...)`

- **Rust 底层绑定**: `duration_since`
- **函数签名参数**: `(instant: Instant, earlier: Instant)`

Returns the duration of time that elapsed from `earlier` to `instant`.

(duration-since instant earlier) -> duration?

* instant : instant?
* earlier : instant? - an instant created no later than `instant`


### `(instant/elapsed ...)`

- **Rust 底层绑定**: `instant_elapsed`
- **函数签名参数**: `(instant: Instant)`

Returns the duration that has elapsed since the given instant was created.

(instant/elapsed instant) -> duration?

* instant : instant?


### `(instant/now ...)`

- **Rust 底层绑定**: `instant_now`

Returns the current instant from a monotonic clock. Pair with
`instant/elapsed` or `duration-since` to measure how much time has passed.

(instant/now) -> instant?


### `(local-time/now! ...)`

- **Rust 底层绑定**: `current_time_formatted`
- **函数签名参数**: `(format_string: String)`

Returns the local time in the format given by the input string (using `chrono::Local::format`).

(local-time/now! fmt) -> string?

* fmt : string?


### `(naive-current-date-local ...)`

- **Rust 底层绑定**: `naive_current_date`

Returns the current date in the local time zone as a naive date (a date
without any time zone information attached).

(naive-current-date-local) -> naive-date?


### `(naive-date-and-hms ...)`

- **Rust 底层绑定**: `with_time`
- **函数签名参数**: `(date: NaiveDate, hour: u32, minute: u32, second: u32)`

Combines a naive date with an hour, minute, and second to produce a naive
date-time. Returns `#false` if the time values are out of range.

(naive-date-and-hms date hour minute second) -> naive-date-time?

* date : naive-date?
* hour : int? - the hour, from 0 to 23
* minute : int? - the minute, from 0 to 59
* second : int? - the second, from 0 to 59


### `(naive-date-day ...)`

- **Rust 底层绑定**: `date_day`
- **函数签名参数**: `(date: NaiveDate)`

Returns the day of the month of the given naive date, from 1 to 31.

(naive-date-day date) -> int?

* date : naive-date?


### `(naive-date-month ...)`

- **Rust 底层绑定**: `date_month`
- **函数签名参数**: `(date: NaiveDate)`

Returns the month of the given naive date, from 1 to 12.

(naive-date-month date) -> int?

* date : naive-date?


### `(naive-date-year ...)`

- **Rust 底层绑定**: `date_year`
- **函数签名参数**: `(date: NaiveDate)`

Returns the year of the given naive date.

(naive-date-year date) -> int?

* date : naive-date?


### `(naive-date-ymd ...)`

- **Rust 底层绑定**: `naive_date`
- **函数签名参数**: `(year: i32, month: u32, day: u32)`

Constructs a naive date from a year, month, and day. Returns `#false` if the
given values do not form a valid calendar date.

(naive-date-ymd year month day) -> naive-date?

* year : int?
* month : int? - the month, from 1 to 12
* day : int? - the day of the month, from 1 to 31


### `(system-time-duration-since ...)`

- **Rust 底层绑定**: `system_time_duration_since`
- **函数签名参数**: `(left: SystemTime, right: SystemTime,)`

Gets the duration between two system times.

(system-time-duration-since time earlier)


### `(system-time/now ...)`

- **Rust 底层绑定**: `system_time_now`

Returns the current `SystemTime`.

(system-time/now) -> SystemTime?


### `(system-time<= ...)`

- **Rust 底层绑定**: `system_time_lte`
- **函数签名参数**: `(left: SystemTime, right: SystemTime)`

Returns `#true` if the first system time is earlier than or equal to the second.

(system-time<= left right) -> bool?

* left : system-time?
* right : system-time?


### `(system-time<? ...)`

- **Rust 底层绑定**: `system_time_lt`
- **函数签名参数**: `(left: SystemTime, right: SystemTime)`

Returns `#true` if the first system time is strictly earlier than the second.

(system-time<? left right) -> bool?

* left : system-time?
* right : system-time?


### `(system-time>= ...)`

- **Rust 底层绑定**: `system_time_gte`
- **函数签名参数**: `(left: SystemTime, right: SystemTime)`

Returns `#true` if the first system time is later than or equal to the second.

(system-time>= left right) -> bool?

* left : system-time?
* right : system-time?


### `(system-time>? ...)`

- **Rust 底层绑定**: `system_time_gt`
- **函数签名参数**: `(left: SystemTime, right: SystemTime)`

Returns `#true` if the first system time is strictly later than the second.

(system-time>? left right) -> bool?

* left : system-time?
* right : system-time?


### `(time/sleep-ms ...)`

- **Rust 底层绑定**: `sleep_millis`
- **函数签名参数**: `(millis: usize)`

Sleeps the thread for a given number of milliseconds.

(time/sleep-ms ms)

* ms : int?


## 传感器与流式处理 (Transducers & Streams)

类似 Clojure 的高效零内存分配流式管道变换器（mapping, filtering, taking, flattening）。引入模块：`(require-builtin steel/transducers)`。

### `(#%reducer->int ...)`

- **Rust 底层绑定**: `reducer_to_int`
- **函数签名参数**: `(arg: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(#%transducers->funcs ...)`

- **Rust 底层绑定**: `transducers_func`
- **函数签名参数**: `(arg: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(dropping ...)`

- **Rust 底层绑定**: `dropping`
- **函数签名参数**: `(amt: &SteelVal)`

Creates a taking iterator combinator

(dropping integer?) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3 4 5) (dropping 3) (into-list)) ;; => '(4 5)
```


### `(enumerating ...)`

- **Rust 底层绑定**: `enumerating`

Create an enumerating iterator

(enumerating) -> iterator?

# Examples
```scheme
(transduce (list 1 3 5) (enumerating) (into-list)) ;; => '((0 1) (1 3) (2 5))
```


### `(extending ...)`

- **Rust 底层绑定**: `extending`
- **函数签名参数**: `(iterable: &SteelVal)`

Create an extending iterator

(extending iterable) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3) (extending (list 4 5 6 7)) (into-list)) ;; => '(1 2 3 4 5 6 7)
```


### `(filtering ...)`

- **Rust 底层绑定**: `filtering`
- **函数签名参数**: `(func: &SteelVal)`

Creates a filtering iterator

(filtering proc?) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3 4) (filtering even?) (into-list)) ;; => '(2 4)
```


### `(flat-mapping ...)`

- **Rust 底层绑定**: `flat_mapping`
- **函数签名参数**: `(func: &SteelVal)`

Creates a flat-mapping iterator

(flat-mapping proc?) -> iterator

# Examples
```scheme
(transduce (list 1 2 3) (flat-mapping (λ (x) (range 0 x))) (into-list)) ;; => '(0 0 1 0 1 2)
```


### `(flattening ...)`

- **Rust 底层绑定**: `flattening`

Creates a flattening iterator that etc

(flattening) -> iterator?

# Examples
```scheme
(transduce (list '(1 2) '(3 4) '(5 6)) (flattening) (into-list)) ;; => '(1 2 3 4 5 6)
```


### `(interleaving ...)`

- **Rust 底层绑定**: `interleaving`
- **函数签名参数**: `(iterable: &SteelVal)`

Create an interleaving iterator

(interleaving any/c) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3) (interleaving (list 4 5 6)) (into-list)) ;; => '(1 4 2 5 3 6)
```


### `(mapping ...)`

- **Rust 底层绑定**: `mapping`
- **函数签名参数**: `(func: &SteelVal)`

Create a mapping iterator

(mapping proc?) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3) (mapping (λ (x) (+ x 1))) (into-list)) ;; => '(2 3 4)
```


### `(taking ...)`

- **Rust 底层绑定**: `taking`
- **函数签名参数**: `(amt: &SteelVal)`

Creates a taking iterator combinator

(taking number?) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3 4 5) (taking 3) (into-list)) ;; => '(1 2 3)
```


### `(zipping ...)`

- **Rust 底层绑定**: `zipping`
- **函数签名参数**: `(iterable: &SteelVal)`

Create a zipping iterator

(zipping any/c) -> iterator?

# Examples
```scheme
(transduce (list 1 2 3) (zipping (list 4 5 6 7)) (into-list)) ;; => '((1 4) (2 5) (3 6))
```


## 网络 TCP 套接字 (Networking TCP)

提供异步非阻塞 TCP 客户端连接、读取与写入支持。引入模块：`(require-builtin steel/tcp)`。

### `(tcp-accept ...)`

- **Rust 底层绑定**: `tcp_accept`
- **函数签名参数**: `(value: &SteelVal)`

Accept a new incoming connection from this listener.

This function will block the calling thread until a new TCP connection
is established. When established, the corresponding `TcpStream?` will be
returned

# Examples

```scheme
(define listener (tcp-listen "127.0.0.1:8080"))

(tcp-accept listener) ;; => TcpStream?
```


### `(tcp-accept-with-addr ...)`

- **Rust 底层绑定**: `tcp_accept_addr`
- **函数签名参数**: `(value: &SteelVal)`

Accept a new incoming connection from this listener.

This function will block the calling thread until a new TCP connection
is established. When established, the corresponding `TcpStream?` will be
returned with the remote address in a pair

# Examples

```scheme
(define listener (tcp-listen "127.0.0.1:8080"))

(tcp-accept listener) ;; => (cons TcpStream? string?)
```


### `(tcp-connect ...)`

- **Rust 底层绑定**: `tcp_connect`
- **函数签名参数**: `(addr: SteelString)`

Opens a TCP connection to a remote host.

`addr` is an address of the remote host as a string

If `addr` yields multiple addresses, `tcp-connect` will be attempted with
each of the addresses until a connection is successful. If none of
the addresses result in a successful connection, the error returned from
the last connection attempt (the last address) is returned.

# Examples

Open a TCP connection to `127.0.0.1:8080`:

```scheme
(tcp-connect "127.0.0.1:8080")
```


### `(tcp-listen ...)`

- **Rust 底层绑定**: `tcp_listen`
- **函数签名参数**: `(addr: SteelString)`

Creates a new `TcpListener?` which will be bound to the specified
address.

The returned listener is ready for accepting connections.

```scheme
(tcp-listen addr) -> TcpListener?
```

Binding with a port number of 0 will request that the OS assigns a port
to this listener. The port allocated can be queried via the
`tcp-listener-local-addr` method.

If `addr` yields multiple addresses, `bind` will be attempted with
each of the addresses until one succeeds and returns the listener. If
none of the addresses succeed in creating a listener, the error returned
from the last attempt (the last address) is returned.

# Examples

Creates a TCP listener bound to `127.0.0.1:80`:

```scheme
(tcp-listen "127.0.0.1:80")
```


### `(tcp-listener-local-addr ...)`

- **Rust 底层绑定**: `tcp_listener_local_addr`
- **函数签名参数**: `(value: &SteelVal)`

Returns the local socket address of this listener.

```scheme
(tcp-listener-local-addr listener) -> string?
```

listener : TcpListener?


### `(tcp-listener-set-non-blocking! ...)`

- **Rust 底层绑定**: `tcp_listener_set_non_blocking`
- **函数签名参数**: `(value: &SteelVal)`

Puts the given TCP listener into non-blocking mode. Once set, `tcp-accept`
will return immediately rather than waiting when no connection is pending.

(tcp-listener-set-non-blocking! listener) -> void?

* listener : tcp-listener?


### `(tcp-shutdown! ...)`

- **Rust 底层绑定**: `tcp_close`
- **函数签名参数**: `(stream: &SteelVal)`

Shuts down both halves of this tcp connection.

This function will cause all pending and future I/O on the specified
portions to return immediately with either an error or an okay result,
dependong on if it was the write or read half.

# Platform-specific behavior

Calling this function multiple times may result in different behavior,
depending on the operating system. On Linux, the second call will
return `Ok(())`, but on macOS, it will return an error wrapping the rust error
`ErrorKind::NotConnected`. This may change in the future.

```scheme
(tcp-shutdown! stream) ;; stream : TcpStream?
```


### `(tcp-stream-buffered-reader ...)`

- **Rust 底层绑定**: `tcp_buffered_output_port`
- **函数签名参数**: `(stream: &SteelVal)`

Get the reader half of this tcp stream as a buffered reader port.

```scheme
(define my-port (tcp-connect "127.0.0.1:8080"))
(tcp-stream-buffered-reader my-port) ;; => port?
```


### `(tcp-stream-reader ...)`

- **Rust 底层绑定**: `tcp_output_port`
- **函数签名参数**: `(stream: &SteelVal)`

Get the reader half of this tcp stream as a port.

```scheme
(define my-port (tcp-connect "127.0.0.1:8080"))
(tcp-stream-reader my-port) ;; => port?
```


### `(tcp-stream-set-non-blocking! ...)`

- **Rust 底层绑定**: `tcp_stream_set_non_blocking`
- **函数签名参数**: `(value: &SteelVal)`

Puts the given TCP stream into non-blocking mode. Once set, reads and writes
on the stream return immediately rather than waiting when they would block.

(tcp-stream-set-non-blocking! stream) -> void?

* stream : tcp-stream?


### `(tcp-stream-writer ...)`

- **Rust 底层绑定**: `tcp_input_port`
- **函数签名参数**: `(stream: &SteelVal)`

Get the writer half of this tcp stream as a port.

```scheme
(define my-port (tcp-connect "127.0.0.1:8080"))
(tcp-stream-writer my-port) ;; => port?
```


## HTTP 客户端与请求 (HTTP Client)

提供 HTTP 请求构造与响应解析（get, post, headers, body）。引入模块：`(require-builtin steel/http)`。

### `(download-file! ...)`

- **Rust 底层绑定**: `download_file`
- **函数签名参数**: `(_url: &SteelString, _file: &SteelString)`

Download file from a URL


### `(http-parse-request ...)`

- **Rust 底层绑定**: `parse_http_request`
- **函数签名参数**: `(vector: &SteelByteVector)`

Parses an HTTP request out of the given byte vector. Returns a request
object on success, or `#false` if the buffer does not yet contain a
complete request.

(http-parse-request buf) -> (or http-request? #false)

* buf : bytes?


### `(http-parse-response ...)`

- **Rust 底层绑定**: `parse_http_response`
- **函数签名参数**: `(vector: &SteelByteVector)`

Parses an HTTP response out of the given byte vector. Returns a response
object on success, or `#false` if the buffer does not yet contain a
complete response.

(http-parse-response buf) -> (or http-response? #false)

* buf : bytes?


### `(http-request-body-offset ...)`

- **Rust 底层绑定**: `body_offset`
- **函数签名参数**: `(value: &SteelVal)`

Returns the byte offset into the original buffer at which the body of a
parsed HTTP request begins.

(http-request-body-offset request) -> int?

* request : http-request?


### `(http-request-headers ...)`

- **Rust 底层绑定**: `headers`
- **函数签名参数**: `(value: &SteelVal)`

Returns the headers of a parsed HTTP request as a hashmap mapping header
names (strings) to their values (byte vectors).

(http-request-headers request) -> hash?

* request : http-request?


### `(http-request-method ...)`

- **Rust 底层绑定**: `method`
- **函数签名参数**: `(value: &SteelVal)`

Returns the HTTP method (for example `"GET"` or `"POST"`) of a parsed
HTTP request.

(http-request-method request) -> string?

* request : http-request?


### `(http-request-path ...)`

- **Rust 底层绑定**: `path`
- **函数签名参数**: `(value: &SteelVal)`

Returns the request path of a parsed HTTP request.

(http-request-path request) -> string?

* request : http-request?


### `(http-request-version ...)`

- **Rust 底层绑定**: `version`
- **函数签名参数**: `(value: &SteelVal)`

Returns the HTTP version of a parsed HTTP request.

(http-request-version request) -> string?

* request : http-request?


### `(http-response-headers ...)`

- **Rust 底层绑定**: `resp_headers`
- **函数签名参数**: `(value: &SteelVal)`

Returns the headers of a parsed HTTP response as a hashmap mapping header
names (strings) to their values (byte vectors).

(http-response-headers response) -> hash?

* response : http-response?


## 子进程与系统命令 (Process & Commands)

提供系统子进程拉起、环境变量设置、标准流重定向与退出码获取。引入模块：`(require-builtin steel/process)`。

### `(child-stderr ...)`

- **Rust 底层绑定**: `child_stderr`
- **函数签名参数**: `(child: &SteelVal)`

Get a handle to the stderr handle of the child process. The process
must have been started with the `with-stderr-piped` option for this
to be available, otherwise stderr will be inherited. This will return
false if the handle has already been consumed.

(child-stderr subprocess) -> (or input-port? #false)

subprocess : ChildProcess?


### `(child-stdin ...)`

- **Rust 底层绑定**: `child_stdin`
- **函数签名参数**: `(child: &SteelVal)`

Get a handle to the stdin handle of the child process. The process
must have been started with the `with-stdin-piped` option for this
to be available, otherwise stdin will be inherited. This will return
false if the handle has already been consumed.

(child-stdout subprocess) -> (or input-port? #false)

subprocess : ChildProcess?


### `(child-stdout ...)`

- **Rust 底层绑定**: `child_stdout`
- **函数签名参数**: `(child: &SteelVal)`

Get a handle to the stdout handle of the child process. The process
must have been started with the `with-stdout-piped` option for this
to be available, otherwise stdout will be inherited. This will return
false if the handle has already been consumed.

(child-stdout subprocess) -> (or output-port? #false)

subprocess : ChildProcess?

```scheme
(define handle (~> (command "/bin/ls" '())
with-stdout-piped
spawn-process
unwrap-ok))
(read-port-to-string (child-stdout handle)) ;; The resulting string
```


### `(command ...)`

- **Rust 底层绑定**: `command`
- **函数签名参数**: `(name: SteelString, args: &List<SteelVal>)`

Create a `CommandBuilder` from a command and a list of arguments. Used to spawn
a subprocess.

(command cmd args) -> CommandBuilder?

* cmd : string?
* args : (listof string?)

```scheme
> (spawn-process (command "echo" (list "hello" "world")))
```


### `(command-builder? ...)`

- **Rust 底层绑定**: `is_command_builder`
- **函数签名参数**: `(value: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(kill ...)`

- **Rust 底层绑定**: `child_kill`
- **函数签名参数**: `(child: &SteelVal)`

Terminate the subprocess.

(subprocess-kill subprocess)

subprocess : ChildProcess?


### `(set-piped-stdout! ...)`

- **Rust 底层绑定**: `stdout_piped_other`
- **函数签名参数**: `(builder: SteelVal)`

Equivalent to `set-stdout-piped!`: connects the spawned child's stdin,
stdout, and stderr to pipes. Returns the command builder.

(set-piped-stdout! process) -> CommandBuilder?

* process : CommandBuilder?


### `(set-stdout-piped! ...)`

- **Rust 底层绑定**: `stdout_piped`
- **函数签名参数**: `(builder: SteelVal)`

Configures the command builder so that the spawned child's stdin, stdout, and
stderr are all connected to pipes, letting the parent process read from and
write to them. Returns the command builder.

(set-stdout-piped! process) -> CommandBuilder?

* process : CommandBuilder?


### `(spawn-process ...)`

- **Rust 底层绑定**: `spawn_process`
- **函数签名参数**: `(builder: &SteelVal)`

Spawn the given process. Returns a result indicating whether the process was
able to be spawned.

(spawn-process process) -> (Result? ChildProcess?)

* process : CommandBuilder?

```scheme
> (require "steel/result")
> (define spawned (spawn-process (command "/bin/ls" '()))) ;; => (Ok #<steel::primitives::process::ChildProcess>)
> (define child (unwrap-ok spawned))
```


### `(subprocess? ...)`

- **Rust 底层绑定**: `is_subprocess`
- **函数签名参数**: `(value: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(wait ...)`

- **Rust 底层绑定**: `wait`
- **函数签名参数**: `(builder: &SteelVal)`

Wait for the subprocess to finish. Returns a result with the status code
of the awaited subprocess.

(wait process) -> (Result? int?)

* process : ChildProcess?

```scheme
> (~> (command "echo" (list "hello"))
spawn-process
unwrap-ok
wait)
```


### `(wait->stdout ...)`

- **Rust 底层绑定**: `wait_with_stdout`
- **函数签名参数**: `(builder: &SteelVal)`

Wait for the subprocess to finish, capturing its stdout. Returns a result
holding everything the process wrote to stdout, decoded as a UTF-8 string.
The process must have been started with stdout piped (for example via
`with-stdout-piped`) for this to capture any output.

(wait->stdout process) -> (Result? string?)

* process : ChildProcess?

```scheme
> (~> (command "echo" (list "hello"))
with-stdout-piped
spawn-process
unwrap-ok
wait->stdout)
```


### `(which ...)`

- **Rust 底层绑定**: `binary_exists_on_path`
- **函数签名参数**: `(binary: String)`

Searches the directories listed on the `PATH` environment variable for the
given executable, returning its absolute path as a string, or `#false` if it
cannot be found.

(which binary) -> (or string? #false)

* binary : string?

```scheme
> (which "ls") ;; => "/bin/ls"
> (which "some-nonexistent-binary") ;; => #false
```


### `(with-cleared-env-vars ...)`

- **Rust 底层绑定**: `clear_env_var`
- **函数签名参数**: `(builder: SteelVal)`

Removes all environment variables for the child.

(with-cleared-env-vars process) -> CommandBuilder?

* process - CommandBuilder?

```scheme
> (define pb (command "echo" (list "hello")))
> (~> (command "echo" (list "hello"))
(with-cleared-env-vars "FOO")
spawn-process
unwrap-ok
wait)
```


### `(with-current-dir ...)`

- **Rust 底层绑定**: `current_dir`
- **函数签名参数**: `(builder: SteelVal, directory: String)`

Sets the current directory for the child. `set-current-dir!` is an alias.

(with-current-dir process dir) -> CommandBuilder?

* process - CommandBuilder?
* dir - string?

```scheme
> (define pb (command "echo" (list "hello")))
> (with-current-dir pb "/home/foo")
> (~> (command "echo" (list "hello"))
(with-current-dir "/home/foo")
spawn-process
unwrap-ok
wait)
```


### `(with-env-var ...)`

- **Rust 底层绑定**: `set_env_var`
- **函数签名参数**: `(builder: SteelVal, key: String, value: String)`

Sets an environment variable for the child. `set-env-var!` is an alias.

(with-env-var process key value) -> CommandBuilder?

* process - CommandBuilder?
* key - string?
* value - string?

```scheme
> (define pb (command "echo" (list "hello")))
> (~> (command "echo" (list "hello"))
(with-env-var "FOO" "BAR")
spawn-process
unwrap-ok
wait)
```


### `(with-stderr ...)`

- **Rust 底层绑定**: `with_stderr`
- **函数签名参数**: `(builder: SteelVal, port: SteelVal)`

Redirect stderr from the process to the given port

(with-stderr process port) -> CommandBuilder?

* process : CommandBuilder?
* port : (and output-port? file-port?)

```scheme
> (define output (open-output-file "test.txt"))
> (~> (command "echo" (list "hello"))
(with-stderr output)
spawn-process
unwrap-ok
wait)
```


### `(with-stderr-piped ...)`

- **Rust 底层绑定**: `with_stderr_piped`
- **函数签名参数**: `(builder: SteelVal)`

Constructs a pipe to be arranged to connect to stderr.

(with-stderr-piped process) -> CommandBuilder?

* process : CommandBuilder?

```scheme
> (~> (command "echo" (list "hello"))
with-stderr-piped
spawn-process
unwrap-ok
wait)
```


### `(with-stdin ...)`

- **Rust 底层绑定**: `with_stdin`
- **函数签名参数**: `(builder: SteelVal, port: SteelVal)`

Redirect stdin from the process to the given port

(with-stdin process port) -> CommandBuilder?

* process : CommandBuilder?
* port : (and input-port? file-port?)

```scheme
> (define output (open-input-file "test.txt"))
> (~> (command "echo" (list "hello"))
(with-stdin output)
spawn-process
unwrap-ok
wait)
```


### `(with-stdin-piped ...)`

- **Rust 底层绑定**: `with_stdin_piped`
- **函数签名参数**: `(builder: SteelVal)`

Constructs a pipe to be arranged to connect to stdin.

(with-stdin-piped process) -> CommandBuilder?

* process : CommandBuilder?

```scheme
> (~> (command "echo" (list "hello"))
with-stdin-piped
spawn-process
unwrap-ok
wait)
```


### `(with-stdout ...)`

- **Rust 底层绑定**: `with_stdout`
- **函数签名参数**: `(builder: SteelVal, port: SteelVal)`

Redirect stdout from the process to the given port

(with-stdout process port) -> CommandBuilder?

* process : CommandBuilder?
* port : (and output-port? file-port?)

```scheme
> (define output (open-output-file "test.txt"))
> (~> (command "echo" (list "hello"))
(with-stdout output)
spawn-process
unwrap-ok
wait)
```


### `(with-stdout-piped ...)`

- **Rust 底层绑定**: `with_stdout_piped`
- **函数签名参数**: `(builder: SteelVal)`

Constructs a pipe to be arranged to connect to stdout.

(with-stdout-piped process) -> CommandBuilder?

* process : CommandBuilder?

```scheme
> (~> (command "echo" (list "hello"))
with-stdout-piped
spawn-process
unwrap-ok
wait)
```


### `(without-env-var ...)`

- **Rust 底层绑定**: `remove_env_var`
- **函数签名参数**: `(builder: SteelVal, key: String)`

Removes an environment variable for the child.

(with-env-var process key) -> CommandBuilder?

* process - CommandBuilder?
* key - string?

```scheme
> (define pb (command "echo" (list "hello")))
> (~> (command "echo" (list "hello"))
(without-env-var "FOO")
spawn-process
unwrap-ok
wait)
```


## 事件循环与异步轮询 (Async Polling)

提供 Steel 异步运行时任务调度与轮询机制。引入模块：`(require-builtin steel/polling)`。

### `(add-event-interest-all ...)`

- **Rust 底层绑定**: `add_all`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(add-event-interest-read ...)`

- **Rust 底层绑定**: `add_read`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(add-event-interest-write ...)`

- **Rust 底层绑定**: `add_write`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(events->list ...)`

- **Rust 底层绑定**: `events`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(events-clear! ...)`

- **Rust 底层绑定**: `clear_events`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(fresh-event-id ...)`

- **Rust 底层绑定**: `fresh_key`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(make-poller ...)`

- **Rust 底层绑定**: `new_poller`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(modify-event-interest-all! ...)`

- **Rust 底层绑定**: `modify_all`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(modify-event-interest-read! ...)`

- **Rust 底层绑定**: `modify_read`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(modify-event-interest-write! ...)`

- **Rust 底层绑定**: `modify_write`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal, key: usize)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(poller-delete! ...)`

- **Rust 底层绑定**: `delete_interest`
- **函数签名参数**: `(poller: &SteelVal, socket: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(poller-wait ...)`

- **Rust 底层绑定**: `poller_wait`
- **函数签名参数**: `(poller: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


## 加密与哈希算法 (Cryptographic Hashes)

提供 MD5、SHA-256、SHA-512 等摘要计算。引入模块：`(require-builtin steel/hashes)`。

### `(md5-hasher ...)`

- **Rust 底层绑定**: `md5_hasher`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(md5-hasher->bytes ...)`

- **Rust 底层绑定**: `md5_hasher_to_bytes`
- **函数签名参数**: `(value: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


### `(md5-hasher-update! ...)`

- **Rust 底层绑定**: `md5_hasher_update`
- **函数签名参数**: `(hasher: &SteelVal, value: &SteelVal)`

*内置原生函数，遵循 R7RS / Steel 标准语义。*


## 核心控制流与标准宏 (Syntax & Special Forms)

Steel 内置了现代 Scheme 与 Lisp 的核心宏和控制流结构，由 `stdlib.scm` 提供并在所有模块中默认生效：

### `when`

- **语法规范**: `(when condition expr ...)`

当 `condition` 为真时顺序求值后续表达式。

### `unless`

- **语法规范**: `(unless condition expr ...)`

当 `condition` 为假（`#f`）时顺序求值后续表达式。

### `cond`

- **语法规范**: `(cond [test-expr then-body ...] ... [else else-body ...])`

多分支条件求值宏，按序匹配首个为真的分支并返回。

### `case`

- **语法规范**: `(case val-expr [((datum ...) expr ...)] ... [else else-body])`

按键值匹配分发分支。

### `and`

- **语法规范**: `(and expr ...)`

短路逻辑与：自左向右求值，若遇到 `#f` 立即返回 `#f`，否则返回最后表达式结果。

### `or`

- **语法规范**: `(or expr ...)`

短路逻辑或：自左向右求值，返回首个非 `#f` 的值，全部为假则返回 `#f`。

### `let*`

- **语法规范**: `(let* ([var expr] ...) body ...)`

顺序作用域绑定，后声明的变量可以在初始化表达式中引用先前声明的变量。

### `letrec*`

- **语法规范**: `(letrec* ([var expr] ...) body ...)`

相互递归局部定义绑定，支持一组互相引用的函数或闭包。

### `while`

- **语法规范**: `(while condition expr ...)`

当 `condition` 为真时循环执行体，直到为假。

### `->`

- **语法规范**: `(-> val (func1 args...) (func2 args...))`

线程首宏（Thread-first）：将前一步的结果作为下一个函数调用的第一个参数传入。

### `->>`

- **语法规范**: `(->> val (func1 args...) (func2 args...))`

线程尾宏（Thread-last）：将前一步的结果作为下一个函数调用的最后一个参数传入。

### `with-handler`

- **语法规范**: `(with-handler (lambda (err) handle-err) body ...)`

异常捕获保护块：当 `body` 执行发生异常时触发 handler 恢复。

### `delay / force`

- **语法规范**: `(delay expr) -> promise?, (force p) -> value`

惰性求值支持。

## Steel 常用扩展模块 (Modules: Result, Option, Serde 等)

通过 `(require "steel/...")` 或 `(require-builtin ...)` 引入的高级开箱即用模块：

### 模块 `"steel/result"`

- **引入方式**: `(require "steel/result")`

Rust 风格 Result 类型：支持 `(Ok v)`、`(Err e)`、`(Ok? res)`、`(Err? res)`、`(unwrap-ok res)`、`(unwrap-err res)`。

### 模块 `"steel/option"`

- **引入方式**: `(require "steel/option")`

Rust 风格 Option 类型：支持 `(Some v)`、`(None)`、`(Some? opt)`、`(None? opt)`、`(unwrap-some opt)`。

### 模块 `"steel/serde"`

- **引入方式**: `(require "steel/serde")`

JSON/数据序列化模块：支持结构体与 Steel 原生值到 JSON/二进制的高性能转换。

### 模块 `"steel/sync"`

- **引入方式**: `(require "steel/sync")`

并发同步原语：提供线程间安全同步通道、Mutex 与原子引用。

### 模块 `"steel/async"`

- **引入方式**: `(require "steel/async")`

异步等待与 Promise 原语。

## 模块引用规范与最佳实践 (Require Syntax Guide)


在 TextCraft 游戏开发中，引用模块请遵循以下原则：

1. **内置 Rust 编译模块**：
   使用 `require-builtin`：
   ```scheme
   (require-builtin steel/time)
   (require-builtin steel/filesystem as fs.)
   (require-builtin steel/tcp as tcp.)
   ```

2. **内置 Scheme 标准库模块**：
   使用 `require` 字符串路径：
   ```scheme
   (require "steel/result")
   (require "steel/option")
   ```

3. **项目内部相对脚本模块**：
   使用相对文件名：
   ```scheme
   (require "prelude.scm")
   (require "task1_input.scm")
   ```

4. **评测确定性（TextCraft 项目约束）**：
   `steel/time`、`steel/random`、`steel/meta`、`steel/filesystem`、`steel/ports`、`steel/process` 在 `--eval` / `run_epoch` 期间被宿主门禁（见文件头提示），模型代码中使用这些模块的逻辑在评测时会直接报错；需要随机与时间语义时改用宿主原语 `(engine:random-int ...)` 与模型自维护的 `tick_count`。
