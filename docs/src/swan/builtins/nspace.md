# nspace

Inspecting [namespaces](../collections.md#namespaces).

## `nspace.enum`

```text
nspace.enum ns -> Tuple
```

Returns the names of the members of the namespace `ns`, as a tuple of texts
sorted by Unicode code point (so capital letters come first). Names inherited
from enclosing scopes are not included. An empty namespace gives `Nothing`.

```swan
nspace.enum {b: 2, a: 1, C: 3} # => ("C", "a", "b")
nspace.enum {}                 # => ()
```

Combine it with namespace application to list the values:

```swan
ns = {x: 1, y: 2}
ns (nspace.enum ns)            # => (1, 2)
```

## `nspace.size`

```text
nspace.size ns -> Numb
```

Returns the number of members of the namespace `ns`. Names inherited from
enclosing scopes are not counted.

```swan
nspace.size {x: 1, y: 2}       # => 2
nspace.size {}                 # => 0
```

## `nspace.parent`

```text
nspace.parent ns -> Namespace
```

Returns the scope in which the namespace `ns` was created, as a namespace, or
`Nothing` for an outermost scope. This is the scope that `ns.name` falls back
to for names that are not members of `ns`.

```swan
outer = {inner = {}}
nspace.parent outer.inner == outer       # => TRUE
```
