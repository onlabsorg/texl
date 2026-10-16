# Conditions and logic

Swan has no `if` statement. Conditions are written with two operators: `?`
picks a value when a condition holds, and `;` supplies an alternative when
there is no value.

## Conditional: ?

`cond ? value` returns `value` if `cond` is true, and `Nothing` otherwise. See
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

Both sides of `?` are always evaluated; only the right side of `;` is
evaluated on demand. This matters for recursion: put the recursive call in the
`;` branch, and the case that stops the recursion in the `?` branch.

```swan
fact = n -> n <= 1 ? 1 ; n * fact (n - 1)
fact 5               # => 120
```

Written the other way round, `n > 1 ? n * fact (n - 1) ; 1` calls `fact`
forever, and the program crashes.

## And, or

`a & b` returns `a` if `a` is false, and `b` otherwise. `a | b` returns `a` if
`a` is true, and `b` otherwise. They return one of their operands, not
necessarily a boolean. Both operands are always evaluated.

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

`?`, `&` and `|` work item by item on tuples, like other operators. Combined
with the fact that `Nothing` disappears from tuples, `?` filters a tuple:

```swan
(TRUE, FALSE, TRUE) ? (1, 2, 3)          # => (1, 3)
nums = (5, 12, 8, 30)
(nums => (n -> n > 10)) ? nums           # => (12, 30)
```
