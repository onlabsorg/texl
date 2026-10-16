# Conditions and logic

Swan has no `if` statement. Conditions are written with two operators: `?`
picks a value when a condition holds, and `;` supplies an alternative when
there is no value.

## Conditional: ?

`cond ? value` returns `value` if `cond` is true, and `Nothing` otherwise.
`value` is evaluated only if `cond` is true. See
[Truth values](types.md#truth-values) for what counts as true.

```swan
TRUE ? "yes"         # => "yes"
FALSE ? "yes"        # => ()
"" ? "yes"           # => ()
```

## Alternative: ;

`value ; other` returns `value`, unless `value` is `Nothing`; then it
evaluates and returns `other`.

```swan
() ; "fallback"      # => "fallback"
"value" ; "fallback" # => "value"
```

`;` only reacts to `Nothing`. Other false values, and `Undefined` values, are
returned as they are:

```swan
FALSE ; 5            # => FALSE
0 ; 5                # => 0
"" ; "empty"         # => ""
```

## If-else

`cond ? a ; b` combines the two: if `cond` is true, `?` gives `a` and `;`
keeps it; otherwise `?` gives `Nothing` and `;` replaces it with `b`.

```swan
age = 20
age >= 18 ? "adult" ; "minor"  # => "adult"
```

Chain them for several cases. The conditions are tested from left to right:

```swan
grade = score -> score >= 90 ? "A" ; score >= 80 ? "B" ; "C"
grade 95             # => "A"
grade 85             # => "B"
grade 70             # => "C"
```

If the chosen value is itself `Nothing`, `;` treats it as missing and moves on
to the alternative:

```swan
TRUE ? () ; "fallback"         # => "fallback"
```

### Evaluation order

The right side of `?`, `;`, `&` and `|` is evaluated only when it is needed.
The other side is never evaluated, so an assignment there does not happen:

```swan
x = 1
FALSE ? (x = 2)      # => ()
x                    # => 1
```

This is also what stops a recursion. The recursive call can be on either side
of `;`:

```swan
fact = n -> n <= 1 ? 1 ; n * fact (n - 1)
fact 5               # => 120
fact2 = n -> n > 1 ? n * fact2 (n - 1) ; 1
fact2 5              # => 120
```

## And, or

`a & b` returns `a` if `a` is false, and `b` otherwise. `a | b` returns `a` if
`a` is true, and `b` otherwise. They return one of their operands, not
necessarily a boolean. `b` is evaluated only when it is the result.

```swan
TRUE & "hello"       # => "hello"
0 & "hello"          # => 0
0 | "world"          # => "world"
"name" | "world"     # => "name"
```

`|` is a convenient way to replace *any* false value, including `0`, `""` and
`Undefined`, with a default:

```swan
count = 0
count | "none"       # => "none"
("a" - 1) | "error"  # => "error"
```

`&` and `|` have the same precedence and associate to the left. When you mix
them, add parentheses.

## Not

`!x` returns `FALSE` if `x` is true, and `TRUE` if it is false.

```swan
!TRUE                # => FALSE
!0                   # => TRUE
!"hello"             # => FALSE
```

Like other operators, `!` works item by item on tuples, so `!()` is `()`, not
`TRUE`.

## Testing for Nothing

`x == ()` does not test for `Nothing`, because `==` works item by item and
`Nothing` has no items. Put the value in a list instead: the list is empty
exactly when the value is `Nothing`.

```swan
x = ()
[x] == []            # => TRUE
[0] == []            # => FALSE
```

To *replace* `Nothing` with a default, use `;`.

## Conditions on tuples

Unlike most operators, `?`, `&` and `|` do not work item by item on tuples.
They test their left side as a whole, and a tuple is true if at least one of
its items is true:

```swan
(FALSE, 0, 1) ? "yes"                    # => "yes"
(FALSE, 0) ? "yes"                       # => ()
(TRUE, FALSE, TRUE) ? (1, 2, 3)          # => (1, 2, 3)
```

To filter a tuple, map it with a function that returns `Nothing` for the items
to drop:

```swan
nums = (5, 12, 8, 30)
nums => (n -> n > 10 ? n)                # => (12, 30)
```
