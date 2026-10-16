# text

Measuring, searching, slicing, splitting and joining [text](../collections.md#text).
Positions and lengths count Unicode characters, not bytes, and start at 0.

## `text.from`

```text
text.from value -> Text
```

Converts `value` to text, the way a template renders it:

| Value        | Text                         |
|--------------|------------------------------|
| a text       | the text itself              |
| a number     | the number, such as `42` or `1.5` |
| a boolean    | `TRUE` or `FALSE`            |
| a list       | `[[List of N]]`              |
| a namespace  | `[[Namespace of N]]`         |
| a function   | `[[Func]]`                   |
| undefined    | `[[Undefined]]`              |

A promise gives `Undefined`. Use `text.from` to build text out of numbers,
since `+` does not mix types.

```swan
text.from 42         # => "42"
text.from 42 + "!"   # => "42!"
text.from (1, TRUE)  # => ("1", "TRUE")
```

## `text.size`

```text
text.size string -> Numb
```

Returns the number of characters in `string`.

```swan
text.size "hello"    # => 5
text.size "héllo"    # => 5
text.size ""         # => 0
```

## `text.enum`

```text
text.enum string -> Tuple
```

Returns the characters of `string` as a tuple of one-character texts.

```swan
text.enum "abc"      # => ("a", "b", "c")
```

## `text.lower`

```text
text.lower string -> Text
```

Returns `string` with every letter in lower case.

```swan
text.lower "TeXL"    # => "texl"
```

## `text.upper`

```text
text.upper string -> Text
```

Returns `string` with every letter in upper case.

```swan
text.upper "TeXL"    # => "TEXL"
```

## `text.trim`

```text
text.trim string -> Text
```

Returns `string` without the white space (spaces, tabs, line breaks) at its
start and end.

```swan
text.trim "  padded  "         # => "padded"
```

## `text.trim_head`

```text
text.trim_head string -> Text
```

Returns `string` without the white space at its start.

```swan
text.trim_head "  padded  "    # => "padded  "
```

## `text.trim_tail`

```text
text.trim_tail string -> Text
```

Returns `string` without the white space at its end. It is handy to remove the
final line break of an imported document.

```swan
text.trim_tail "  padded  "    # => "  padded"
```

## `text.find`

```text
text.find needle haystack -> Numb
```

Returns the position of the first occurrence of the text `needle` in the text
`haystack`, or `-1` if there is none.

```swan
text.find "l" "hello"          # => 2
text.find "lo" "hello"         # => 3
text.find "z" "hello"          # => -1
```

## `text.rfind`

```text
text.rfind needle haystack -> Numb
```

Returns the position of the last occurrence of the text `needle` in the text
`haystack`, or `-1` if there is none.

```swan
text.rfind "l" "hello"         # => 3
```

## `text.head`

```text
text.head n string -> Text
```

Returns the first `n` characters of `string`. A negative `n` returns all the
characters except the last `-n`. A fractional `n` is truncated toward zero.

```swan
text.head 3 "swanky"           # => "swa"
text.head (-2) "swanky"        # => "swan"
text.head 10 "swan"            # => "swan"
```

## `text.tail`

```text
text.tail n string -> Text
```

Returns the characters of `string` from position `n` on, that is, all but the
first `n`. A negative `n` returns the last `-n` characters. A fractional `n`
is truncated toward zero.

```swan
text.tail 4 "swanky"           # => "ky"
text.tail (-3) "swanky"        # => "nky"
```

Combined with `text.rfind`, it extracts the end of a path:

```swan
path = "docs/guide/intro.texl"
text.tail (text.rfind "/" path + 1) path # => "intro.texl"
```

## `text.split`

```text
text.split separator string -> Tuple
```

Splits `string` at every occurrence of the text `separator` and returns the
parts as a tuple. If `separator` does not occur, the result is `string`
itself.

```swan
text.split "," "a,b,c"         # => ("a", "b", "c")
text.split ", " "red, green"   # => ("red", "green")
text.split "," "abc"           # => "abc"
```

## `text.join`

```text
text.join separator items -> Text
```

Joins the texts of the tuple `items`, putting `separator` between them. Every
item must be a text; otherwise the result is `Undefined`. `items` is taken as
a whole, so `text.join` returns a single text.

```swan
text.join "-" ("a", "b", "c")  # => "a-b-c"
text.join "" ("a", "b")        # => "ab"
text.join ", " ((1, 2) => text.from)     # => "1, 2"
text.join ", " (list.enum ["x", "y"])    # => "x, y"
```

To join with line breaks, put a real line break between the quotes, since
Swan text has no escapes:

```texl
<% text.join "
" ("first", "second") %>
```

```text
first
second
```
