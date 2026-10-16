# undef

Creating and inspecting [undefined values](../undefined.md).

## `undef.from`

```text
undef.from value -> Undefined
```

Returns an undefined value that carries `value`, for example a message that
explains the error. The whole argument is wrapped, even a tuple.

```swan
err = undef.from "not found"
type err             # => "Undefined"
(undef.inspect err).val        # => "not found"
```

## `undef.inspect`

```text
undef.inspect u -> Namespace
```

Returns a namespace that describes the undefined value `u`. Its `type` member
names the kind of error, and the other members depend on it:

| `type`            | Members                       | Meaning                                    |
|-------------------|-------------------------------|--------------------------------------------|
| `UnaryOperation`  | `op`, `val`, `pos`            | operation `op` failed on the operand `val` |
| `BinaryOperation` | `op`, `lhv`, `rhv`, `pos`     | operation `op` failed on `lhv` and `rhv`   |
| `Termination`     | `expected`, `pos`             | the closing bracket `expected` is missing  |
| `Token`           | `token`, `pos`                | a token is in the wrong place              |
| `Name`            | `pos`                         | a name was expected                        |
| `Value`           | `val`                         | created by `undef.from`                    |

`pos` is the position of the error in the source, in bytes from the start of
the expression; built-in functions report 0. Operands that are themselves
undefined are described in the same way. If `u` is not undefined, the result is
`Undefined`.

```swan
e = undef.inspect ([1] - 1)
e.type               # => "BinaryOperation"
e.op                 # => "subtraction"
(e.lhv, e.rhv)       # => ([1], 1)
(undef.inspect (text.size 1)).(op, val)  # => ("text.size", 1)
```
