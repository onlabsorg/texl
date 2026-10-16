# list

Measuring, searching and slicing [lists](../collections.md#lists). All the
functions return new values; none changes a list.

## `list.size`

```text
list.size items -> Numb
```

Returns the number of items in the list `items`.

```swan
list.size [10, 20, 30]         # => 3
list.size []                   # => 0
```

## `list.enum`

```text
list.enum items -> Tuple
```

Returns the items of the list `items` as a tuple. An empty list gives
`Nothing`, and a list of one item gives that item. Use it to map a function
over the items of a list.

```swan
list.enum [10, 20, 30]         # => (10, 20, 30)
list.enum [5]                  # => 5
list.enum []                   # => ()
[list.enum [1, 2, 3] => (x -> x * 2)]    # => [2, 4, 6]
```

## `list.reverse`

```text
list.reverse items -> List
```

Returns a list with the items of the list `items` in reverse order.

```swan
list.reverse [1, 2, 3]         # => [3, 2, 1]
```

## `list.find`

```text
list.find item items -> Numb
```

Returns the position of the first item of the list `items` that is equal to
`item`, or `-1` if there is none. Positions start at 0.

```swan
list.find 20 [10, 20, 30, 20]  # => 1
list.find 99 [10, 20, 30]      # => -1
list.find [1] [[1], [2]]       # => 0
```

## `list.rfind`

```text
list.rfind item items -> Numb
```

Returns the position of the last item of the list `items` that is equal to
`item`, or `-1` if there is none.

```swan
list.rfind 20 [10, 20, 30, 20] # => 3
```

## `list.head`

```text
list.head n items -> List
```

Returns the first `n` items of the list `items`. A negative `n` returns all
the items except the last `-n`. A fractional `n` is truncated toward zero.

```swan
list.head 2 [1, 2, 3, 4]       # => [1, 2]
list.head (-1) [1, 2, 3, 4]    # => [1, 2, 3]
list.head 10 [1, 2]            # => [1, 2]
```

## `list.tail`

```text
list.tail n items -> List
```

Returns the items of the list `items` from position `n` on, that is, all but
the first `n`. A negative `n` returns the last `-n` items. A fractional `n` is
truncated toward zero.

```swan
list.tail 1 [1, 2, 3, 4]       # => [2, 3, 4]
list.tail (-2) [1, 2, 3, 4]    # => [3, 4]
```
