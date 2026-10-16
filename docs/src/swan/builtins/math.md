# math

Mathematical constants and functions. They take and return numbers. A result
that is not a number, such as the square root of a negative number, is
`Undefined`.

## Constants

```text
math.PI -> Numb
math.E -> Numb
math.INF -> Numb
math.INFINITY -> Numb
```

`math.PI` is π, `math.E` is Euler's number *e*, and `math.INF` and
`math.INFINITY` are positive infinity. For negative infinity, write
`-(math.INF)`.

```swan
math.PI              # => 3.141592653589793
math.E               # => 2.718281828459045
-(math.INF)          # => -inf
```

## Rounding

```text
math.round x -> Numb
math.floor x -> Numb
math.ceil x -> Numb
math.trunc x -> Numb
math.abs x -> Numb
```

`math.round` rounds to the nearest whole number, and halfway cases away from
zero. `math.floor` rounds down, `math.ceil` rounds up, and `math.trunc` drops
the fraction. `math.abs` returns the absolute value.

```swan
math.round 2.5       # => 3
math.round (-2.5)    # => -3
math.floor (-1.5)    # => -2
math.ceil 1.2        # => 2
math.trunc (-1.7)    # => -1
math.abs (-3)        # => 3
```

To round to a number of decimals, scale first:

```swan
math.round (math.PI * 100) / 100         # => 3.14
```

## Powers and logarithms

```text
math.sqrt x -> Numb
math.exp x -> Numb
math.ln x -> Numb
math.log2 x -> Numb
math.log10 x -> Numb
math.pow exponent base -> Numb
math.log base x -> Numb
```

`math.sqrt` is the square root, `math.exp x` is *e* to the power `x`, and
`math.ln`, `math.log2` and `math.log10` are the logarithms in base *e*, 2 and
10.

`math.pow` and `math.log` are curried, and take the fixed part first:
`math.pow exponent base` is `base ^ exponent`, and `math.log base x` is the
logarithm of `x` in base `base`.

```swan
math.sqrt 16         # => 4
math.sqrt (-1)       # => Undefined
math.log10 1000      # => 3
math.pow 2 5         # => 25
square = math.pow 2
(1, 2, 3) => square  # => (1, 4, 9)
math.log 2 8         # => 3
```

## Trigonometry

```text
math.sin x -> Numb
math.cos x -> Numb
math.tan x -> Numb
math.asin x -> Numb
math.acos x -> Numb
math.atan x -> Numb
math.atan2 x y -> Numb
math.hypot x y -> Numb
```

The trigonometric functions and their inverses work in radians.
`math.atan2 x y` is the angle of the point (`x`, `y`) from the positive *x*
axis, between -π and π. `math.hypot x y` is the distance of the point
(`x`, `y`) from the origin.

```swan
math.sin 0           # => 0
math.cos math.PI     # => -1
math.atan2 1 1       # => 0.7853981633974483
math.hypot 3 4       # => 5
```

## Hyperbolic functions

```text
math.sinh x -> Numb
math.cosh x -> Numb
math.tanh x -> Numb
math.asinh x -> Numb
math.acosh x -> Numb
math.atanh x -> Numb
```

The hyperbolic functions and their inverses.

```swan
math.cosh 0          # => 1
```

## `math.max`, `math.min`

```text
math.max numbers -> Numb
math.min numbers -> Numb
```

Return the largest and the smallest item of the tuple `numbers`. They reduce
the whole tuple to one value. Every item must be a number, and the tuple must
not be empty; otherwise the result is `Undefined`.

```swan
math.max (1, 5, 2)   # => 5
math.min (1, 5, 2)   # => 1
math.max (list.enum [3, 9])    # => 9
math.max ()          # => Undefined
```
