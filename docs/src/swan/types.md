# Values and types

Every Swan value has one of ten types. The `type` function returns the name of
a value's type as text.

| Type        | Example                | Description                                              |
|-------------|------------------------|----------------------------------------------------------|
| `Nothing`   | `()`                   | The absence of a value. It is also the empty tuple.      |
| `Bool`      | `TRUE`, `FALSE`        | A truth value.                                           |
| `Numb`      | `42`, `3.14`           | A 64-bit floating-point number.                          |
| `Text`      | `"hello"`              | A sequence of Unicode characters.                        |
| `Tuple`     | `1, 2, "three"`        | Two or more values separated by commas.                  |
| `List`      | `[1, 2, 3]`            | An ordered sequence of values, indexed from 0.           |
| `Namespace` | `{x: 1, y: 2}`         | A set of named values.                                   |
| `Func`      | `x -> x * 2`           | A function, defined in Swan or built in.                 |
| `Promise`   | `promise.create()`     | A value that may arrive later.                           |
| `Undefined` | `"a" - 1`              | The result of an invalid operation.                      |

```swan
type 42              # => "Numb"
type "hello"         # => "Text"
type [1, 2]          # => "List"
type (x -> x)        # => "Func"
type math.sqrt       # => "Func"
type ("a" - 1)       # => "Undefined"
```

`type` works item by item on tuples, like most functions. So `type (1, "a")`
is `("Numb", "Text")`, and `type ()` is `()`, not `"Nothing"`.

```swan
type (1, "a")        # => ("Numb", "Text")
type ()              # => ()
```

All values are immutable, except namespaces and promises: a namespace can gain
new names (see [Namespaces](collections.md#namespaces)), and a promise receives
its value once (see [Promises](promises.md)).

## Truth values

Conditions (`?`, `&`, `|`, `!`) convert their operand to a truth value. These
values are **false**:

- `FALSE`
- `0`
- `""` (empty text)
- `[]` (empty list)
- `{}` (empty namespace)
- `Nothing`
- every `Undefined` value
- a pending promise

Every other value is **true**, including every function. A tuple is true if at
least one of its items is true.

```swan
bool.from 0          # => FALSE
bool.from "0"        # => TRUE
bool.from []         # => FALSE
bool.from (x -> x)   # => TRUE
bool.any (0, "", 3)  # => TRUE
```

A [promise](promises.md) is false while it is pending and true once it is
resolved, whatever its value:

```swan
p = promise.create ()
bool.from p          # => FALSE
promise.resolve p 0
bool.from p          # => TRUE
```

## Equality

`==` and `!=` compare values by content. Values of different types are never
equal.

```swan
1 == 1               # => TRUE
"a" == "a"           # => TRUE
[1, [2]] == [1, [2]] # => TRUE
{x: 1} == {x: 1}     # => TRUE
1 == "1"             # => FALSE
```

- Two namespaces are equal if they have the same names with equal values, and
  were created in the same scope.
- A function is never equal to anything, not even to itself.
- Two promises are equal only if they are the same promise.

Equality between tuples works item by item and gives a tuple of truth values.
To compare two sequences as a whole, compare them as lists:

```swan
(1, 2) == (1, 3)     # => (TRUE, FALSE)
[1, 2] == [1, 3]     # => FALSE
```

## Order

`<`, `<=`, `>` and `>=` compare two values of the same type:

- numbers by value;
- text alphabetically, by Unicode code point (so `"Z" < "a"`);
- booleans with `FALSE < TRUE`;
- lists item by item; when one list is a prefix of the other, the shorter one
  comes first.

`Nothing` is less than every other value. Comparing values of different types,
or of an unordered type (functions, namespaces, promises), gives `Undefined`.

```swan
2 < 10               # => TRUE
"apple" < "banana"   # => TRUE
"Z" < "a"            # => TRUE
[1, 2] < [1, 2, 0]   # => TRUE
() < -1000           # => TRUE
1 < "2"              # => Undefined
```

## Numbers

Numbers follow the IEEE 754 rules. Dividing by zero does not fail: it gives an
infinity, or "not a number" (`NaN`) for `0 / 0`.

```swan
7 / 2                # => 3.5
1 / 0                # => inf
0.1 + 0.2            # => 0.30000000000000004
```

Use `math.round`, `math.floor` and the other [math](builtins/math.md)
functions to get whole numbers.

## Converting values

| To        | Use                                                           |
|-----------|---------------------------------------------------------------|
| `Text`    | [`text.from`](builtins/text.md#textfrom)                      |
| `Numb`    | [`numb.from`](builtins/numb.md#numbfrom)                      |
| `Bool`    | [`bool.from`](builtins/bool.md#boolfrom)                      |
| `List`    | `[value]` (a tuple becomes the items of the list)             |
| `Tuple`   | [`list.enum`](builtins/list.md#listenum), [`text.enum`](builtins/text.md#textenum), [`nspace.enum`](builtins/nspace.md#nspaceenum) |

```swan
text.from 42 + "!"   # => "42!"
numb.from "41" + 1   # => 42
[(1, 2, 3)]          # => [1, 2, 3]
list.enum [1, 2, 3]  # => (1, 2, 3)
```
