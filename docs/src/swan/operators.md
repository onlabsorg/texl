# Operators

## Precedence

Operators with a higher precedence bind tighter. Use parentheses whenever the
order is not obvious.

| Precedence | Operators                       | Meaning                              | Associativity |
|-----------:|---------------------------------|--------------------------------------|---------------|
|    highest | `+x` `-x` `!x` `$x`             | prefix operators                     |               |
|         30 | `x.y`                           | subcontexting                        | left          |
|         28 | `f x`                           | function application                 | left          |
|         27 | `x ^ y`                         | power                                | right         |
|         26 | `x * y` `x / y` `x % y`         | product, division, modulo            | left          |
|         25 | `x + y` `x - y`                 | sum, subtraction                     | left          |
|         24 | `==` `!=` `<` `<=` `>` `>=`     | comparison                           | left          |
|         23 | `x & y` `x \| y`                | logical and, or                      | left          |
|         22 | `x ? y`                         | conditional                          | left          |
|         21 | `x ; y`                         | alternative                          | left          |
|         15 | `x -> y`                        | function definition                  | right         |
|         14 | `f >> g` `g << f`               | composition                          | left          |
|         13 | `x => f` `p ~> f`               | mapping, then                        | right         |
|         12 | `x = y` `x : y`                 | assignment, labelling                | left          |
|     lowest | `x , y`                         | pairing                              | left          |

Some consequences:

```swan
double = x -> x * 2
double 2 + 3         # => 7, because application binds tighter than +
double (2 + 3)       # => 10
2 ^ 3 ^ 2            # => 512, because ^ is right-associative
-2 ^ 2               # => 4, because prefix - binds tighter than ^
-(2 ^ 2)             # => -4
x = 1, 2             # => 2, because , binds looser than =
x                    # => 1
```

Prefix operators bind tighter than everything, even `.` and application. Wrap
the operand in parentheses when it is more than a name or a literal:

```swan
-math.PI             # => Undefined, because it means (-math).PI
-(math.PI)           # => -3.141592653589793
```

## Arithmetic

| Operator | Numbers          | Text          | Lists         | Booleans     |
|----------|------------------|---------------|---------------|--------------|
| `x + y`  | sum              | concatenation | concatenation | logical or   |
| `x - y`  | subtraction      |               |               |              |
| `x * y`  | product          |               |               | logical and  |
| `x / y`  | division         |               |               |              |
| `x % y`  | remainder        |               |               |              |
| `x ^ y`  | power            |               |               |              |

Both operands must have the same type. Any other combination gives
`Undefined`. Swan never converts types implicitly.

```swan
12 + 2 * 3           # => 18
7 % 3                # => 1
-7 % 3               # => -1, the sign of the left operand
"Te" + "xl"          # => "Texl"
[1, 2] + [3]         # => [1, 2, 3]
"Total: " + 10       # => Undefined
"Total: " + text.from 10   # => "Total: 10"
```

Prefix `-x` negates a number. Prefix `+x` returns `x` unchanged, whatever its
type.

## Comparison

`==`, `!=`, `<`, `<=`, `>` and `>=` compare two values and return `TRUE` or
`FALSE`. See [Equality](types.md#equality) and [Order](types.md#order) for the
rules. Chained comparisons do not work: `a < b < c` compares a boolean with
`c`. Write `a < b & b < c` instead.

## Logic and conditions

| Operator | Result                                                                  |
|----------|-------------------------------------------------------------------------|
| `!x`     | `FALSE` if `x` is true, `TRUE` if it is false.                          |
| `x & y`  | `x` if `x` is false, otherwise `y`.                                     |
| `x \| y` | `x` if `x` is true, otherwise `y`.                                      |
| `x ? y`  | `y` if `x` is true, otherwise `Nothing`.                                |
| `x ; y`  | `x` unless `x` is `Nothing`; then `y`. `y` is evaluated only if needed. |

Together, `?` and `;` make an if-else: `cond ? then ; else`. See
[Conditions and logic](conditions.md).

## Functions

| Operator  | Result                                                               |
|-----------|----------------------------------------------------------------------|
| `x -> y`  | A function with parameters `x` and body `y`.                          |
| `f x`     | `f` applied to `x`. Lists, text and namespaces can be applied too.    |
| `f >> g`  | A function that applies `f`, then `g` to the result.                  |
| `g << f`  | The same function: `g` after `f`.                                     |
| `x => f`  | `f` applied to each item of the tuple `x`.                            |

See [Functions](functions.md).

## Names and namespaces

| Operator  | Result                                                                 |
|-----------|------------------------------------------------------------------------|
| `x = y`   | Binds the name(s) `x` to `y`, and returns `Nothing`.                    |
| `x : y`   | Binds the name(s) `x` to `y`, and returns `y`.                          |
| `ns.x`    | Evaluates the expression `x` inside the namespace `ns`.                 |
| `$x`      | The value of the name `x` in the outermost scope that binds it.         |

See [Names and scopes](names.md) and [Namespaces](collections.md#namespaces).

## Tuples and promises

| Operator  | Result                                                                 |
|-----------|------------------------------------------------------------------------|
| `x , y`   | A tuple of the items of `x` followed by the items of `y`.               |
| `p ~> f`  | A promise of `f` applied to the value of the promise `p`.               |

See [Tuples](tuples.md) and [Promises](promises.md).

## Operators on tuples

All binary operators except `,`, `;`, `=`, `:`, `->`, `=>` and `~>` work item by
item when an operand is a tuple. The prefix operators `-` and `!` do too.

```swan
(1, 2) + (10, 20)    # => (11, 22)
-(1, 2)              # => (-1, -2)
```

A single value counts as a tuple of one item. It is **not** repeated to match
the other operand, so use [mapping](functions.md#mapping) to apply the same
operation to every item:

```swan
(1, 2, 3) * 10                 # => (10, Undefined, Undefined)
(1, 2, 3) => (x -> x * 10)     # => (10, 20, 30)
```

See [Operations on tuples](tuples.md#operations-on-tuples) for the details.
