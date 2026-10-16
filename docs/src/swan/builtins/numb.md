# numb

Converting values to numbers.

## `numb.from`

```text
numb.from value -> Numb
```

Converts `value` to a number:

- a number is returned unchanged;
- `TRUE` is `1` and `FALSE` is `0`;
- a text is read as a number literal, which may have a sign, a fraction and an
  exponent. The text must contain nothing else, not even spaces. `"inf"` and
  `"NaN"` are accepted too.

Any other value, and a text that is not a number, gives `Undefined`.

```swan
numb.from "42"       # => 42
numb.from "-1.5e3"   # => -1500
numb.from TRUE       # => 1
numb.from " 42"      # => Undefined
numb.from (text.trim " 42")    # => 42
numb.from ("1", "2") # => (1, 2)
```
