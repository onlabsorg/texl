# Built-in library

Every Swan program starts with these names already bound:

| Name                         | Contents                                          |
|------------------------------|---------------------------------------------------|
| [Root names](root.md)        | `TRUE`, `FALSE`, `type`, `parse`, `bind`, `label` |
| [`bool`](bool.md)            | Converting and combining truth values             |
| [`numb`](numb.md)            | Converting values to numbers                      |
| [`text`](text.md)            | Measuring, searching, slicing, splitting text     |
| [`list`](list.md)            | Measuring, searching and slicing lists            |
| [`nspace`](nspace.md)        | Inspecting namespaces                             |
| [`math`](math.md)            | Mathematical constants and functions              |
| [`time`](time.md)            | Timestamps and calendar dates                     |
| [`promise`](promise.md)      | Creating and resolving promises                   |
| [`undef`](undef.md)          | Creating and inspecting undefined values          |

In a template, the engine also binds [`texl`](texl.md), which imports other
documents, and the host can bind its own names (see
[Names and data](../../templates/names.md)).

## Reading the reference

Each entry starts with a signature that shows how to call the function and
what it returns:

```text
text.find needle haystack -> Numb
```

This reads: apply `text.find` to `needle`, then apply the result to
`haystack`, and you get a number. The description says what types the
arguments must have.

### Curried functions

Functions with two parameters are *curried*: they take the first argument and
return a function that takes the second. You can call them in one go, or keep
the intermediate function:

```swan
text.find "l" "hello"          # => 2
find_l = text.find "l"
find_l "world"                 # => 3
```

The first argument is the one you are most likely to fix in advance, such as
the separator of `text.split` or the exponent of `math.pow`. Note that
`text.find ("l", "hello")` passes a single tuple, not two arguments, and does
not work.

### Tuples

Unless an entry says otherwise, functions are applied to each item of a tuple
argument and return a tuple of results. A `Nothing` argument gives `Nothing`.

```swan
text.upper ("a", "b")          # => ("A", "B")
text.find ("l", "o") "hello"   # => (2, 4)
text.upper ()                  # => ()
```

Functions that reduce a tuple to one value, such as `bool.all` and `math.max`,
say so in their entry.

### Errors

A function given an argument of the wrong type returns `Undefined`. The
[undefined value](../undefined.md) names the function in its `op` member.

```swan
text.size 42                   # => Undefined
(undef.inspect (text.size 42)).op        # => "text.size"
```
