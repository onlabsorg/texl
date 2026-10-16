# Common pitfalls

Each of these follows from the rules in the previous chapters, but they
surprise most people at least once.

## The comma binds loosest

`x = 1, 2` binds `1` to `x`, then makes a tuple of `Nothing` and `2`. Put a
tuple in parentheses to bind it:

```swan
x = 1, 2
x                    # => 1
x = (1, 2)
x                    # => (1, 2)
```

The same goes for function bodies: `f = x -> a, b` is `(f = x -> a), b`. Write
`f = x -> (a, b)`.

## Function bodies with = or :

A body that binds names must be in parentheses, because `=` binds looser than
`->`:

```swan
f = x -> y = x       # => Undefined, and nothing is bound
f = x -> (y = x, y * 2)
f 4                  # => 8
```

## Application is left-associative

`f g x` applies `f` to `g`, then applies the result to `x`. Wrap the inner call
in parentheses:

```swan
type promise.create ()         # => Undefined
type (promise.create ())       # => "Promise"
```

## Negative arguments

`f -1` subtracts `1` from `f`. Write `f (-1)`:

```swan
text.head -2 "swanky"          # => Undefined
text.head (-2) "swanky"        # => "swan"
```

## Prefix operators bind tightest

`-x.y` means `(-x).y`, and `!f x` means `(!f) x`. Add parentheses:

```swan
-(math.PI)           # => -3.141592653589793
!(bool.from 0)       # => TRUE
```

## A single value is not repeated over a tuple

`(1, 2, 3) * 2` multiplies only the first item. Map a function instead:

```swan
(1, 2, 3) => (x -> x * 2)      # => (2, 4, 6)
```

## Comparisons on tuples give tuples

`(1, 2) == (1, 2)` is `(TRUE, TRUE)`, not `TRUE`. Compare lists, or reduce
with `bool.all`:

```swan
[1, 2] == [1, 2]     # => TRUE
bool.all ((1, 2) == (1, 2))    # => TRUE
```

## ; only replaces Nothing

`FALSE ; 5` is `FALSE`, and an undefined value is not replaced either. Use `|`
to replace every false value:

```swan
FALSE ; 5            # => FALSE
FALSE | 5            # => 5
```

## Conditions do not work item by item

`?`, `&` and `|` test a tuple as a whole, so `?` does not filter a tuple:

```swan
(TRUE, FALSE) ? (1, 2)         # => (1, 2)
```

Map the tuple instead. See [Conditions on tuples](conditions.md#conditions-on-tuples).

## Assignments do not chain

`a = b = 1` and `a : b : 1` give `Undefined`. Write `a = 1, b = 1`.

## Text has no escapes

`"\n"` is a backslash and an `n`, and a text literal cannot contain `"`. Write
line breaks directly inside the quotes.

## Unbound names are Nothing

A misspelled name gives `Nothing`, which renders as empty text. If a value is
unexpectedly missing, check the spelling first.

## Mapping associates to the right

`x => f => g` means `x => (f => g)`. Add parentheses around each step, or
compose: `x => (f >> g)`.

## Joining requires text

`text.join` takes a tuple of text values. Convert numbers first, and turn lists
into tuples:

```swan
text.join ", " (1, 2)                    # => Undefined
text.join ", " ((1, 2) => text.from)     # => "1, 2"
text.join ", " (list.enum ["a", "b"])    # => "a, b"
```
