# Names and scopes

## Binding names

`=` binds a name to a value and returns `Nothing`. `:` does the same but
returns the value.

```swan
x = 10               # => ()
x                    # => 10
y : 20               # => 20
y                    # => 20
```

`=` is handy in a sequence of steps, where the `Nothing` disappears (see
[Sequencing](tuples.md#sequencing)). `:` is handy when you want to keep the
value, for example to show it in a template and remember it at the same time.

Binding a name again replaces its value in the current scope. The left side of
`=` and `:` must be a name, a tuple of names, or `()`. Anything else gives
`Undefined` and binds nothing.

```swan
1 = 2                # => Undefined
```

Assignments do not chain: `a = b = 1` and `a : b : 1` give `Undefined`. Write
`a = 1, b = 1` instead.

## Destructuring

A tuple of names binds each name to the matching item:

```swan
(a, b) = (1, 2)
a                    # => 1
b                    # => 2
(a, b) = (b, a)
a                    # => 2
```

When there are more names than items, the extra names are bound to `Nothing`.
When there are more items than names, the last name receives all the remaining
items:

```swan
(first, rest) = (1, 2, 3)
first                # => 1
rest                 # => (2, 3)
(p, q, r) = (1, 2)
r                    # => ()
```

The same rules apply to function parameters.

## Unbound names

A name that is not bound anywhere evaluates to `Nothing`. This is not an
error, so a misspelled name silently gives `Nothing`. The
[alternative operator](conditions.md#alternative-) turns that into a default
value:

```swan
title ; "Untitled"   # => "Untitled"
title = "Report"
title ; "Untitled"   # => "Report"
```

## Scopes

A scope is a table of names. Each scope (except the outermost) has a parent,
and looking up a name searches the scope itself, then its parent, and so on.

New scopes are created by:

- **function calls**: the body of a function runs in a new scope, whose
  parent is the scope where the function was *defined*;
- **namespaces**: the expression inside `{ }` runs in a new scope, whose
  parent is the current scope.

`ns.expr` evaluates `expr` in the scope of the namespace `ns`.

Binding a name always affects the current scope. Inside a function, `=`
creates a local name, even when an outer scope has the same name:

```swan
count = 0
increment = () -> (count = count + 1, count)
increment()          # => 1
count                # => 0
```

The outermost scopes hold the built-in names. Your code runs in a scope below
them, so you can reuse a built-in name without changing the built-in.

## Global names: bind and label

`bind` and `label` bind names in the global scope, the scope where top-level
code runs, wherever they are called from. They take the name as text, which
makes them useful inside functions and in promise callbacks:

```swan
remember = v -> bind "last" v
remember 42
last                 # => 42
label "answer" 42    # => 42
answer               # => 42
```

`bind name value` returns `Nothing`, like `=`. `label name value` returns the
value, like `:`. Both accept a tuple of names. See
[Root names](builtins/root.md#bind).

## Lifting: $name

`$name` reads `name` from the *outermost* scope that binds it, skipping the
nearer bindings that hide it:

```swan
x = 1
f = () -> (x = 2, (x, $x))
f()                  # => (2, 1)
```

The operand of `$` must be a name or a tuple of names.
