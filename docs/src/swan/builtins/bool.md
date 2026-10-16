# bool

Converting and combining truth values. See
[Truth values](../types.md#truth-values) for the rules.

## `bool.TRUE`, `bool.FALSE`

```text
bool.TRUE -> Bool
bool.FALSE -> Bool
```

The two boolean values, the same as the root names `TRUE` and `FALSE`.

```swan
bool.TRUE == TRUE    # => TRUE
```

## `bool.from`

```text
bool.from value -> Bool
```

Returns the truth value of `value`: `FALSE` for `FALSE`, `0`, `""`, `[]`, `{}`
and undefined values, `TRUE` for everything else.

```swan
bool.from 0          # => FALSE
bool.from "no"       # => TRUE
bool.from (0, 1)     # => (FALSE, TRUE)
```

## `bool.any`

```text
bool.any items -> Bool
```

Returns `TRUE` if at least one item of the tuple `items` is true. It reduces
the whole tuple to one value, so `bool.any ()` is `FALSE`.

```swan
bool.any (0, "", 3)  # => TRUE
bool.any (0, "")     # => FALSE
```

## `bool.all`

```text
bool.all items -> Bool
```

Returns `TRUE` if every item of the tuple `items` is true. It reduces the
whole tuple to one value, so `bool.all ()` is `TRUE`.

```swan
bool.all (1, "a")    # => TRUE
bool.all (1, 0)      # => FALSE
bool.all ([1, 2] == [1, 2], 3 > 2)       # => TRUE
```
