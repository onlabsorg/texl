# Functions

## Defining functions

`params -> body` creates a function. Functions are values: you bind them to
names, pass them to other functions and return them from functions.

```swan
double = x -> x * 2
double 21            # => 42
```

The parameters can be:

- a name: `x -> x * 2`;
- a tuple of names: `(a, b) -> a + b`;
- `()`, for a function that ignores its argument: `() -> "hello"`.

The body is any expression. It is evaluated each time the function is called,
not when it is defined.

`->` binds looser than most operators, but tighter than `,`, `=` and `:`.
Wrap the body in parentheses when it contains one of them:

```swan
f = x -> (y = x * 2, y + 1)
f 3                  # => 7
```

## Calling functions

Write the function, then its argument. Parentheses around the argument are only
needed when it is more than a name or a literal:

```swan
double = x -> x * 2
double 5             # => 10
double(5)            # => 10
double (2 + 3)       # => 10
double 2 + 3         # => 7
```

Pass several arguments as a tuple, and no argument as `()`:

```swan
add = (a, b) -> a + b
add (3, 4)           # => 7
hello = () -> "hello"
hello()              # => "hello"
```

Arguments bind to parameters like a [destructuring
assignment](names.md#destructuring): missing arguments are `Nothing`, and the
last parameter collects any extra arguments.

```swan
first = (head, tail) -> head
first (1, 2, 3)      # => 1
rest = (head, tail) -> tail
rest (1, 2, 3)       # => (2, 3)
```

Application is left-associative: `f g x` means `(f g) x`, which applies `f` to
`g`. To apply `f` to the result of `g x`, write `f (g x)`:

```swan
text.upper text.trim " hi "    # => Undefined
text.upper (text.trim " hi ")  # => "HI"
```

The same applies to a negative argument: `f -1` is a subtraction. Write
`f (-1)`.

## Closures

A function remembers the scope where it was defined, and its body can use the
names of that scope. It sees their current values when it runs:

```swan
rate = 2
convert = amount -> amount * rate
convert 10           # => 20
rate = 3
convert 10           # => 30
```

A function that returns a function makes a *closure* over its own parameters:

```swan
multiplier = factor -> (x -> x * factor)
triple = multiplier 3
triple 5             # => 15
```

The names a function binds while running are local to that call.

## Recursion

A function can call itself through the name it is bound to:

```swan
fact = n -> n <= 1 ? 1 ; n * fact (n - 1)
fact 10              # => 3628800
```

The recursion stops because the right side of `?` and `;` is evaluated only
when it is needed. See [Evaluation order](conditions.md#evaluation-order).

## Currying

A function with several parameters can also be written as a function that
takes the first parameter and returns a function that takes the next one. This
lets you supply the arguments one at a time:

```swan
add = a -> b -> a + b
add 3 4              # => 7
increment = add 1
increment 10         # => 11
```

Most built-in functions with two parameters are curried this way, for example
`text.find needle haystack` and `math.pow exponent base`.

## Mapping

`items => f` applies `f` to each item of the tuple `items`:

```swan
double = x -> x * 2
(1, 2, 3) => double            # => (2, 4, 6)
(4, 9, 16) => math.sqrt        # => (2, 3, 4)
```

With a tuple of functions, each function is applied to each item:

```swan
(1, 2) => (x -> x + 1, x -> x * 10)    # => (2, 10, 3, 20)
```

Results that are `Nothing` are dropped, so `=>` also filters:

```swan
(1, 5, 12, 3) => (n -> n > 4 ? n)      # => (5, 12)
```

`=>` is right-associative: `x => f => g` means `x => (f => g)`. To map twice,
add parentheses or compose the functions:

```swan
double = x -> x * 2
inc = x -> x + 1
((1, 2) => double) => inc      # => (3, 5)
(1, 2) => (double >> inc)      # => (3, 5)
```

To map over a list, turn it into a tuple first, and put the result back in a
list if you need one:

```swan
[list.enum [1, 2, 3] => (x -> x * x)]  # => [1, 4, 9]
```

## Composition

`f >> g` is a function that applies `f`, then `g`. `g << f` is the same
function, written in the other direction ("`g` after `f`"):

```swan
inc = x -> x + 1
double = x -> x * 2
(inc >> double) 3    # => 8
(double << inc) 3    # => 8
(inc << double) 3    # => 7
```

## Other callable values

Lists, text and namespaces can also be applied to an argument. This is how you
read their items:

| Value       | Argument        | Result                                     |
|-------------|-----------------|--------------------------------------------|
| a list      | a number `n`    | the item at position `n`                   |
| a text      | a number `n`    | the character at position `n`              |
| a namespace | a text `k`      | the member named `k`                       |

```swan
[10, 20, 30] 1       # => 20
"swan" 0             # => "s"
{a: 1} "a"           # => 1
```

Applying a tuple of functions applies each of them to the argument:

```swan
(math.floor, math.round, math.ceil) 2.5          # => (2, 3, 3)
```

Applying any other value, such as a number, gives `Undefined`.

## Evaluating code at run time

[`parse`](builtins/root.md#parse) compiles a text into a function, which
evaluates the code in the namespace you apply it to:

```swan
area = parse "width * height"
area {width: 3, height: 4}     # => 12
```
