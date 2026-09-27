# swan-core

A zero-copy, hand-written Rust implementation of the **Swan** expression language.

Swan is a small, dynamically-typed, expression-oriented language designed to be
embedded inside template engines and similar host applications. This crate
provides the lexer, parser (AST), and evaluator as a reusable library with no
external parser-generator dependencies.

> **Status**: the lexer is complete and tested. The parser, AST, value types,
> and evaluator modules are stubs under active development.

---

## The Swan Language — Quick Reference

A Swan expression is a sequence of binary operations:

```
operand  operator  operand  operator  operand  ...
```

Operands can be grouped with `(…)`, `[…]`, or `{…}`. Everything from `#` to
the end of the line is a comment and is ignored.

### Operators (highest to lowest precedence)

| Precedence | Operator | Description |
|:---:|---|---|
| 30 | `F X`  ·  `x.y` | Function application / sub-contexting |
| 27 | `x ^ y` | Exponentiation |
| 26 | `x * y`  ·  `x / y`  ·  `x % y` | Multiplicative arithmetic |
| 25 | `x + y`  ·  `x - y` | Additive arithmetic |
| 24 | `x == y`  ·  `x != y`  ·  `x < y`  ·  `x <= y`  ·  `x > y`  ·  `x >= y` | Comparison |
| 23 | `x & y`  ·  `x \| y` | Logical AND / OR |
| 22 | `x ? y` | Conditional: returns `y` when `x` is truthy |
| 21 | `x ; y` | Alternative: returns `x` unless it is `Undefined`, then returns `y` |
| 15 | `params -> body` | Function definition *(right-assoc)* |
| 14 | `f >> g`  ·  `g << f` | Pipe / Compose |
| 13 | `x => f` | Mapping |
| 12 | `name = x`  ·  `name: x` | Assignment / Labelling |
| 10 | `x , y` | Pairing (tuple construction) |

Unary `+x` and `-x` are also supported.

### Types

| Type | Literal / Example | Notes |
|---|---|---|
| `Nothing` | `()` | The unit type / empty tuple |
| `Bool` | `TRUE`, `FALSE` | Returned by comparisons |
| `Numb` | `42`, `3.14`, `1.5e-3` | IEEE-754 double |
| `Text` | `"hello"`, `'hello'` | String; indexable via apply |
| `List` | `[1, 2, 3]` | Ordered sequence; indexable via apply |
| `Namespace` | `{x=1, y=2}` | Name → value mapping; prototype inheritance |
| `Func` | `x -> x * 2` | First-class function; `self` for recursion |
| `Undefined` | *(result of invalid op)* | Operations never panic — they return `Undefined` |
| `Tuple` | `1, 2, "abc"` | Flat product type; not an `Item` |

### A few examples

```
# Arithmetic
(12 + 2) * 3 - 4          # => 38

# Function definition and application
double = x -> x * 2
double 5                  # => 10
double(5)                 # => 10  (parentheses are optional)

# Recursive factorial using `self`
fact = n -> n == 0 ? 1 ; n * self(n-1)
fact 6                    # => 720

# Namespace (records)
point = {x=3, y=4}
point.x                   # => 3   (sub-contexting)
point "y"                 # => 4   (apply)

# Conditional and alternative
x = -1
x > 0 ? "positive" ; "non-positive"   # => "non-positive"

# Composition
add1 = x -> x + 1
mul2 = x -> x * 2
add1 >> mul2               # same as x -> mul2(add1(x))
(add1 >> mul2) 3           # => 8
```

---

## Crate Architecture

```
swan-core
├── token.rs   — Token<'a> enum; all token kinds produced by the lexer
├── lexer.rs   — Zero-copy character scanner; implements Iterator<Item=(usize, Token)>
├── ast.rs     — Expression enum (AST node types) + Anchored<T> position wrapper
├── parser.rs  — [stub] Recursive-descent parser: tokens → AST
├── value.rs   — [stub] Runtime value representation
└── eval.rs    — [stub] Expression evaluator and scope management
```

### `token.rs`

Defines [`Token<'a>`](src/token.rs) — a lifetime-parameterised enum whose
`Identifier`, `Text`, `Comment`, and `Undefined` variants borrow slices
directly from the source string, avoiding heap allocations.

### `lexer.rs`

[`Lexer<'a>`](src/lexer.rs) is a hand-written character state machine.  It
implements `Iterator<Item=(usize, Token<'a>)>` where the `usize` is the byte
offset of each token in the source.

Key properties:
- **Zero-copy** — no `String` allocations during tokenisation.
- **UTF-8 aware** — `position` tracks byte offsets, safe for `&str` slicing.
- **Greedy two-char operators** — `==`, `!=`, `<=`, `>=`, `->`, `<<`, `>>`,
  `=>` are recognised before falling through to their single-char counterparts.

### `ast.rs`

Defines `Expression`, the recursive AST node enum, and `Anchored<T>`, a
wrapper that attaches a source position to any value.

### `parser.rs` *(stub)*

Will implement a shunting-yard recursive-descent parser that converts the
token stream into an `Expression` tree, honouring Swan's operator precedence
and associativity rules.

### `value.rs` *(stub)*

Will define the runtime `Value` type (`Bool`, `Numb`, `Text`, `List`,
`Namespace`, `Func`, `Undefined`, …).

### `eval.rs` *(stub)*

Will implement the evaluator: walks the `Expression` tree and produces a
`Value`, threaded through a scope chain.

---

## Getting Started

Add the crate as a path dependency while it is under development:

```toml
[dependencies]
swan-core = { path = "../swan-core" }
```

### Using the lexer today

```rust
use swan_core::lexer::Lexer;
use swan_core::token::Token;

fn main() {
    let source = r#"x = "hello""#;

    for (pos, token) in Lexer::new(source) {
        println!("{pos:>3}  {token:?}");
    }
}
```

Output:

```
  0  Identifier("x")
  2  AssignmentOp
  4  Text("hello")
```

---

## Development

```bash
# Compile the library
cargo build

# Run the test suite (unit tests + doctests)
cargo test

# Compile without producing artefacts (fast feedback)
cargo check

# Generate and open API documentation
cargo doc --open
```

---

## Reference Implementation

The JS reference implementation lives in `swan-js-repomix.md` (a single-file
snapshot of the original [SwanJS](https://github.com/onlabsorg/swan-js)
repository). The key files are:

| JS file | Rust equivalent |
|---|---|
| `lib/lexer.js` | `src/lexer.rs` |
| `lib/parser.js` | `src/parser.rs` |
| `lib/types.js` | `src/value.rs` |
| `lib/interpreter.js` | `src/eval.rs` |

---

## License

[MIT](https://opensource.org/licenses/MIT)
