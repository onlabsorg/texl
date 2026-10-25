# nspace

Inspecting [namespaces](../collections.md#namespaces).

## `nspace.enum`

```text
nspace.enum ns -> Tuple
```

Returns the names of the members of the namespace `ns`, as a tuple of texts
sorted by Unicode code point (so capital letters come first). An empty
namespace gives `Nothing`.

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

Returns the number of members of the namespace `ns`.

```swan
nspace.size {x: 1, y: 2}       # => 2
nspace.size {}                 # => 0
```
