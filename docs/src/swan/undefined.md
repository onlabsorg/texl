# Undefined values

When an operation makes no sense, such as subtracting a number from a text,
Swan does not stop. The operation returns an `Undefined` value, which records
what went wrong, and the evaluation continues.

```swan
"hello" - 1          # => Undefined
type ("hello" - 1)   # => "Undefined"
```

In a template, an undefined value renders as `[[Undefined]]`.

## Where they come from

- Operators applied to the wrong types: `"a" + 1`, `-"a"`, `TRUE < 1`.
- Applying a value that cannot be applied: `5 3`.
- Built-in functions given the wrong arguments: `text.size 42`.
- Math results that are not numbers: `math.sqrt (-1)`.
- Syntax errors: `(1 + 2`, `1 + * 2`.
- Binding something that is not a name: `1 = 2`.
- Binding a name inside a namespace: `math.(pi = 3)`.
- `undef.from`, which creates one on purpose.

An unbound name is *not* undefined: it is `Nothing`.

## How they spread

Most operations with an undefined operand give an undefined result, so an
error early in an expression usually shows in its final value:

```swan
("hello" - 1) + 5    # => Undefined
text.upper ("hello" - 1)       # => Undefined
```

A few operations handle them like other values:

- An undefined value is false, so `?`, `&`, `|` and `!` treat it as such.
- `;` does not replace it, because it is not `Nothing`.
- A tuple keeps it as one of its items.

```swan
!("hello" - 1)       # => TRUE
("hello" - 1) | 0    # => 0
("hello" - 1) ; 0    # => Undefined
(1, "a" - 1, 3)      # => (1, Undefined, 3)
```

So `|` is the way to replace an undefined value with a default.

## Finding out what went wrong

[`undef.inspect`](builtins/undef.md#undefinspect) turns an undefined value into
a namespace that describes it:

```swan
err = undef.inspect ("hello" - 1)
err.type             # => "BinaryOperation"
err.op               # => "subtraction"
err.lhv              # => "hello"
err.rhv              # => 1
```

| `type`            | Cause                                   | Other members                  |
|-------------------|-----------------------------------------|--------------------------------|
| `UnaryOperation`  | a prefix operator or a built-in function | `op`, `val`, `pos`            |
| `BinaryOperation` | a binary operator or an application     | `op`, `lhv`, `rhv`, `pos`      |
| `Termination`     | a missing closing bracket               | `expected`, `pos`              |
| `Token`           | a character or token in the wrong place | `token`, `pos`                 |
| `Name`            | a name was expected                     | `pos`                          |
| `Value`           | created with `undef.from`               | `val`                          |

`op` names the operation, such as `"sum"`, `"application"` or `"text.size"`.
`pos` is the position in the expression where the error happened, counted in
bytes from the start of the expression (in a template, from the start of the
tag's content). Built-in functions report position 0. When an operand is itself
undefined, `undef.inspect` describes it too, so you can follow the chain back
to the original error.

## Creating undefined values

A function can return an undefined value to signal that its input was invalid.
[`undef.from`](builtins/undef.md#undeffrom) wraps any value, such as a message,
into one:

```swan
safe_div = (a, b) -> b == 0 ? undef.from "division by zero" ; a / b
safe_div (1, 0)      # => Undefined
(undef.inspect (safe_div (1, 0))).val    # => "division by zero"
```
