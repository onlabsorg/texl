# Tuples

A tuple is a sequence of values separated by commas.

```swan
1, 2, "three"        # => (1, 2, "three")
(1, "b")             # => (1, "b")
```

Tuples are how Swan passes several arguments to a function, returns several
results, and applies an operation to many values at once.

## Tuples are flat

A tuple never contains another tuple or `Nothing`. When you build one, nested
tuples are flattened and `Nothing` items are dropped:

```swan
(1, (2, 3))          # => (1, 2, 3)
(1, (), 2)           # => (1, 2)
```

There are no tuples of one item and no empty tuples: a single value *is* a
tuple of one item, and `Nothing` *is* the empty tuple.

```swan
(5)                  # => 5
((), ())             # => ()
```

A trailing comma is a syntax error:

```swan
(1, 2,)              # => Undefined
```

To nest sequences, use [lists](collections.md#lists), which keep their
structure: `[[1, 2], [3]]` is a list of two lists.

## Sequencing

Because `Nothing` items disappear, a tuple whose items are assignments acts
like a sequence of steps. The assignments return `Nothing`, so only the last
value remains:

```swan
x = 10, y = 5, x + y # => 15
```

This is the usual way to write a multi-step computation in one expression.

## Operations on tuples

Most operators work item by item on tuples:

```swan
(10, 20) + (1, 2)    # => (11, 22)
(1, 2) == (1, 3)     # => (TRUE, FALSE)
!(0, 1)              # => (TRUE, FALSE)
```

When the two tuples have different lengths, the shorter one is padded with
`Nothing`. Most operations between a value and `Nothing` are undefined:

```swan
(1, 2) + (10, 20, 30)          # => (11, 22, Undefined)
```

A single value is a tuple of one item, so it is combined with the first item
only:

```swan
(1, 2, 3) * 10                 # => (10, Undefined, Undefined)
```

To apply the same operation to every item, map a function over the tuple with
`=>`:

```swan
(1, 2, 3) => (x -> x * 10)     # => (10, 20, 30)
```

## Tuples and functions

A tuple passed to a function becomes its arguments:

```swan
add = (a, b) -> a + b
add (3, 4)           # => 7
```

Built-in functions that take one value, such as `math.sqrt` or `text.upper`,
are applied to each item of a tuple:

```swan
math.sqrt (4, 9, 16) # => (2, 3, 4)
text.upper ("a", "b")          # => ("A", "B")
```

Applying a tuple of functions applies each of them:

```swan
(math.floor, math.ceil) 2.5    # => (2, 3)
```

## Working with items

Tuples have no indexing of their own. Put the tuple in a list to count or pick
items, and use [`list.enum`](builtins/list.md#listenum) to turn a list back
into a tuple:

```swan
t = (10, 20, 30)
[t] 1                # => 20
list.size [t]        # => 3
list.enum (list.reverse [t])   # => (30, 20, 10)
```

## How tuples render

In a template, a tuple renders as its items joined by `", "`:

```texl
<% 1, 2, "three" %>
```

```text
1, 2, three
```

To join items with another separator, use
[`text.join`](builtins/text.md#textjoin).
