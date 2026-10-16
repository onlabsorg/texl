# Root names

These names are bound directly in the global scope.

## `TRUE`, `FALSE`

```text
TRUE -> Bool
FALSE -> Bool
```

The two boolean values. They are also available as `bool.TRUE` and
`bool.FALSE`.

```swan
TRUE & FALSE         # => FALSE
```

## `type`

```text
type value -> Text
```

Returns the name of the type of `value`: `"Bool"`, `"Numb"`, `"Text"`,
`"List"`, `"Namespace"`, `"Func"`, `"Promise"` or `"Undefined"`.

Like other functions, `type` is applied to each item of a tuple, so it never
returns `"Tuple"` or `"Nothing"`: `type (1, "a")` is `("Numb", "Text")`, and
`type ()` is `()`.

```swan
type 42              # => "Numb"
type [1, 2]          # => "List"
type text.size       # => "Func"
type (1, "a")        # => ("Numb", "Text")
```

## `parse`

```text
parse source -> Func
```

Compiles the Swan expression in the text `source` and returns a function. The
function takes a namespace and evaluates the expression inside it, so the
expression can use the members of the namespace, and the names visible where
the namespace was created.

The source is parsed once, when `parse` is called. A syntax error does not
make `parse` fail: the function returns `Undefined` when it runs. A `source`
that is not text gives `Undefined`, and so does applying the function to
something other than a namespace.

```swan
area = parse "width * height"
area {width: 3, height: 4}     # => 12
area 42                        # => Undefined
```

## `bind`

```text
bind name value -> Nothing
```

Binds `name` to `value` in the global scope, wherever it is called from, and
returns `Nothing`. `name` is a text, or a tuple of texts to bind several names
at once, following the rules of [destructuring](../names.md#destructuring).

`bind` is how a function, or a callback waiting on a promise, makes a value
available to the rest of the program. In a template, the global scope is the
engine's global scope, shared by every document the engine renders.

```swan
f = () -> bind "seen" TRUE
f()
seen                           # => TRUE
bind ("a", "b") (1, 2)
a + b                          # => 3
```

## `label`

```text
label name value -> value
```

Like [`bind`](#bind), but returns `value` instead of `Nothing`, the way `:`
differs from `=`.

```swan
label "answer" 42              # => 42
answer                         # => 42
```
