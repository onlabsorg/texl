# Syntax

A Swan program is one expression, made of operands (names, literals and
bracketed groups) joined by operators. This chapter describes the operands;
[Operators](operators.md) describes the rest.

## Whitespace and comments

Spaces, tabs and line breaks separate tokens and are otherwise ignored, so an
expression can span several lines.

A comment starts with `#` and runs to the end of the line:

```swan
x = 10     # bind 10 to x
x + 1      # => 11
```

## Names

A name starts with a letter or `_` and continues with letters, digits and `_`.
Names are case-sensitive.

```swan
total = 1
_tmp = 2
item2 = 3
Total = 4
total                # => 1
```

Swan has no keywords. `TRUE`, `FALSE` and the built-in namespaces are ordinary
names bound in the global scope, so you can shadow them (but you rarely want
to). A name that is not bound evaluates to `Nothing`, not to an error:

```swan
not_bound_anywhere   # => ()
```

## Numbers

A number literal is a sequence of digits, with an optional fraction and an
optional exponent. All numbers are 64-bit floating-point values.

```swan
42                   # => 42
3.14                 # => 3.14
1.5e3                # => 1500
2E-2                 # => 0.02
```

A number cannot start or end with a dot, and cannot contain `_`. A negative
number is the `-` operator applied to a positive literal.

```swan
.5                   # => Undefined
1_000                # => Undefined
-0.5                 # => -0.5
```

## Text

A text literal is enclosed in double quotes. It can span several lines.

```swan
"hello"              # => "hello"
greeting = "Dear reader,
  welcome."
text.size greeting   # => 23
```

Text literals have **no escape sequences**: a backslash is an ordinary
character, and a literal cannot contain a double quote. Single quotes do not
delimit text.

```swan
"a\nb"               # this is the four characters a, \, n, b
text.size "a\nb"     # => 4
'single'             # => Undefined
```

In a template, write quotes and other special characters in the text around
the tags instead.

## Brackets

| Brackets  | Meaning                                                             |
|-----------|---------------------------------------------------------------------|
| `( ... )` | Grouping. `()` alone is `Nothing`, the empty value.                 |
| `[ ... ]` | A [list](collections.md#lists) of the values inside.                |
| `{ ... }` | A [namespace](collections.md#namespaces) of the names bound inside. |

```swan
(1 + 2) * 3          # => 9
()                   # => ()
[1, 2, 3]            # => [1, 2, 3]
{a: 1, b: 2}         # => {a: 1, b: 2}
```

## Syntax errors

Swan never rejects a program. A syntax error, such as a missing closing bracket
or a misplaced operator, produces an `Undefined` value in the place where the
error is, and the rest of the expression is evaluated as usual.

```swan
(1 + 2               # => Undefined
1 + * 2              # => Undefined
```

See [Undefined values](undefined.md) to find out what went wrong.
