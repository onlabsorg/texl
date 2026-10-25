# Lists, text and namespaces

## Lists

A list is an ordered sequence of values, written in square brackets.

```swan
nums = [10, 20, 30]
nums                 # => [10, 20, 30]
[]                   # => []
```

The brackets enclose an expression, usually a tuple, and the list contains its
items. Since tuples are flat, a tuple inside the brackets adds its items
directly, and `Nothing` adds nothing. A list inside a list stays a single item:

```swan
[1, (2, 3)]          # => [1, 2, 3]
[1, (), 2]           # => [1, 2]
[[1, 2], [3]]        # => [[1, 2], [3]]
```

### Getting items

Apply a list to a number to get the item at that position. Positions start
at 0. A position that is negative, fractional or past the end gives `Nothing`.

```swan
nums = [10, 20, 30]
nums 0               # => 10
nums 2               # => 30
nums 10              # => ()
nums (0, 2)          # => (10, 30)
```

### Combining and transforming

`+` concatenates lists. The [`list`](builtins/list.md) namespace has functions
to measure, search and slice them.

```swan
nums = [10, 20, 30]
nums + [40]          # => [10, 20, 30, 40]
list.size nums       # => 3
list.find 20 nums    # => 1
list.head 2 nums     # => [10, 20]
list.reverse nums    # => [30, 20, 10]
```

Lists are values: no operation changes a list. To process each item, turn the
list into a tuple with `list.enum`, map a function over it, and put the result
back in brackets:

```swan
prices = [5, 12, 20]
[list.enum prices => (p -> p * 2)]   # => [10, 24, 40]
```

## Text

Text values support `+` (concatenation) and comparison. Applying a text to a
number gives the character at that position, starting at 0. Positions count
Unicode characters, not bytes.

```swan
word = "swan"
word 0               # => "s"
word (1, 2)          # => ("w", "a")
word 10              # => ()
"sw" + "an" == word  # => TRUE
```

The [`text`](builtins/text.md) namespace has functions to measure, search,
slice, split and join text.

```swan
text.size "héllo"    # => 5
text.upper "texl"    # => "TEXL"
text.split "," "a,b,c"         # => ("a", "b", "c")
```

## Namespaces

A namespace is a set of named values, written in curly braces.

```swan
point = {x: 3, y: 4}
point                # => {x: 3, y: 4}
```

The braces enclose an expression, which Swan evaluates in a new scope. Every
name bound during that evaluation becomes a member of the namespace. The
members can use the members defined before them, and the names around the
namespace:

```swan
scale = 10
box = {width: 2, height: width * 3, area: width * height * scale}
box.area             # => 120
```

You can bind members with `:` or `=`. Inside the braces, the separators are
just the comma operator, so any expression works, including definitions of
functions:

```swan
counter = {start = 1, next = n -> n + 1}
counter.next counter.start     # => 2
```

### Reading members

There are two ways to read a member:

- `ns.name` evaluates `name` *inside* the namespace. The right side can be any
  expression, and it can use all the members.
- `ns "name"` looks up the member called `"name"`. The key is a text value, so
  it can be computed.

```swan
point = {x: 3, y: 4}
point.x              # => 3
point.(x * y)        # => 12
point "y"            # => 4
key = "x"
point key            # => 3
point "z"            # => ()
```

The two differ for names that are not members. `ns "name"` only sees the
members of the namespace, while `ns.name` also sees the names of the place
where it is evaluated, like any other expression there. Members hide those
names:

```swan
point = {x: 3, y: 4}
scale = 10
point "scale"        # => ()
point.(x * scale)    # => 30
type point.math      # => "Namespace"
x = 1
point.x              # => 3
```

A namespace is just its members. It does not remember where it was created,
so `ns.name` does not see the names around that place:

```swan
make = () -> (secret = 1, {x: 2})
p = make()
p.secret             # => ()
```

Reading a member of `Nothing` gives `Nothing`, so a chain of `.` stops safely
at the first missing link:

```swan
config = {}
config.server.port   # => ()
```

### Namespaces are immutable

A namespace cannot change after it is created: its members are the names bound
inside its braces, and nothing else. Binding a name inside a namespace with
`.` gives an undefined value and binds nothing. This also holds for the
built-in namespaces, such as `text` and `math`, so the built-ins work the
same everywhere:

```swan
point = {x: 3}
type (point.(y = 4))     # => "Undefined"
point                    # => {x: 3}
type (text.(size = 0))   # => "Undefined"
text.size "abc"          # => 3
```

To get a namespace with more members, create a new one. Inside its braces,
`.` gives access to the members of the old one:

```swan
point = {x: 3}
point3d = point.{x: x, y: 4}
point3d                  # => {x: 3, y: 4}
```

A function call inside `.` still works as usual: its parameters and the names
its body binds are in its own scope, not in the namespace.

### Inspecting namespaces

The [`nspace`](builtins/nspace.md) namespace lists and counts the members:

```swan
point = {x: 3, y: 4}
nspace.enum point    # => ("x", "y")
nspace.size point    # => 2
```

### Namespaces as modules

Namespaces group related functions, like the built-in `math` and `text`
namespaces:

```swan
geometry = {
    square = side -> side * side,
    circle = radius -> math.PI * radius ^ 2
}
geometry.square 3    # => 9
```
