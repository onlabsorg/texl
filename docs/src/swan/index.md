# The Swan language

Swan is the language you write inside Texl tags. It is small, dynamically
typed and expression-oriented: a Swan program is a single expression, and
evaluating it always produces a value. There are no statements, no keywords
and no exceptions. An invalid operation does not stop the program; it produces
an `Undefined` value instead.

This part of the manual describes the language. To learn how Swan fits into a
template, start with [Templates](../templates/index.md).

## A quick tour

```swan
name = "Swan"                        # bind a name
"Hello, " + name + "!"               # => "Hello, Swan!"

double = x -> x * 2                  # define a function
double 21                            # => 42

(1, 2, 3) => double                  # => (2, 4, 6)
[10, 20, 30] 1                       # => 20

point = {x: 3, y: 4}                 # a namespace
point.x                              # => 3
math.sqrt (point.x ^ 2 + point.y ^ 2)  # => 5

age = 20
age >= 18 ? "adult" ; "minor"        # => "adult"

"hello" - 1                          # => Undefined
```

## Reading the examples

Each example block is a short session. The lines run in order, and names bound
on one line are available on the next. When a line ends with `# =>`, the text
after the arrow is the value of that line, written in Swan notation:

| Value                  | Written as             |
|------------------------|------------------------|
| numbers                | `42`, `3.14`, `inf`    |
| text                   | `"hello"`              |
| booleans               | `TRUE`, `FALSE`        |
| nothing                | `()`                   |
| tuples                 | `(1, 2, 3)`            |
| lists                  | `[1, 2, 3]`            |
| namespaces             | `{x: 3, y: 4}`         |
| functions              | `Func`                 |
| promises               | `Promise(42)`          |
| undefined values       | `Undefined`            |

A template shows some of these values differently. For example, a list renders
as `[[List of 3]]`. See [Tags and output](../templates/tags.md).

## Trying Swan

The quickest way to evaluate Swan expressions is a template. Save this as
`try.texl`:

```texl
<% double = x -> x * 2 %><% double 21 %>
```

and render it with the [command-line tool](../hosts/cli.md):

```sh
texl try.texl
```

## Contents

- [Syntax](syntax.md): comments, names and literals.
- [Values and types](types.md): the ten types, truth values and comparison.
- [Operators](operators.md): every operator, its precedence and its meaning.
- [Tuples](tuples.md): comma-separated values, and how operators work on them.
- [Lists, text and namespaces](collections.md): the collection types.
- [Names and scopes](names.md): binding names, destructuring, scopes.
- [Conditions and logic](conditions.md): `?`, `;`, `&`, `|` and `!`.
- [Functions](functions.md): definition, application, closures, composition.
- [Promises](promises.md): values that arrive later.
- [Undefined values](undefined.md): how Swan reports errors.
- [Common pitfalls](pitfalls.md): surprises worth knowing in advance.
- [Built-in library](builtins/index.md): every built-in name.
