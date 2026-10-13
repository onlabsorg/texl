# The Complete Swan Language Manual

Swan is a concise, dynamically-typed, expression-oriented language designed for embedding in template engines and host applications. Every valid Swan snippet is an expression that evaluates to a value. Swan never panics or throws uncaught runtime exceptions; invalid operations produce an `Undefined` value that propagates predictably through your code.

Examples on consecutive lines are evaluated in order in the same context, like an interactive session. To combine several expressions into one program, see *Sequencing expressions*.

---

## Table of Contents

1. [A Quick Tour](#1-a-quick-tour)
2. [Basics](#2-basics)
3. [Operators and Precedence](#3-operators-and-precedence)
4. [Types and Truthiness](#4-types-and-truthiness)
5. [Tuples](#5-tuples)
6. [Lists](#6-lists)
7. [Namespaces](#7-namespaces)
8. [Variables, Scope and Sequencing](#8-variables-scope-and-sequencing)
9. [Conditionals and Logic](#9-conditionals-and-logic)
10. [Functions](#10-functions)
11. [Undefined Values and Errors](#11-undefined-values-and-errors)
12. [Built-in Reference](#12-built-in-reference)
13. [Worked Examples](#13-worked-examples)
14. [Common Pitfalls](#14-common-pitfalls)

---

## 1. A Quick Tour

Here's a quick look at some of Swan's features:

```text
name = "Swan"
nums = [1, 2, 3]
double = x -> x * 2
double 21                          # => 42
(1, 2, 3) => double                # => (2, 4, 6)
age = 20
age >= 18 ? "Adult" ; "Minor"      # => "Adult"
```

## 2. Basics

Swan is an expression-oriented language, meaning everything is an expression that evaluates to a value.

### Comments
Comments start with `#` and run to the end of the line:
```text
# This is a comment
x = 10  # Assign 10 to x
```

### Identifiers
Identifiers are names for values. They start with a letter or `_` and can contain letters, digits, or `_`.
```text
foo = 1
_bar = 2
myVar1 = 3
```

### Literals
Literals are direct representations of values.

*   **Numbers**: Swan uses double-precision floating-point numbers.
    ```text
    42      # => 42
    3.14    # => 3.14
    -0.5    # => -0.5 (The `-` is a prefix operator)
    ```

*   **Text**: Text values (strings) are enclosed in single or double quotes.
    ```text
    "hello" # => "hello"
    'world' # => "world"
    ```

*   **Booleans**: The truth values.
    ```text
    TRUE    # => TRUE
    FALSE   # => FALSE
    ```

*   **Nothing**: Represents the absence of a value.
    ```text
    ()      # => ()
    ```

### Grouping
Expressions can be grouped using:
*   Parentheses `(...)` for grouping or creating tuples.
*   Square brackets `[...]` for creating lists.
*   Curly braces `{...}` for defining namespaces.

### Prefix Operators
Swan has two prefix operators:
*   `-`: Negation (for numbers).
    ```text
    -10     # => -10
    ```
*   `!`: Logical NOT. This converts its operand to a boolean using Swan's truthiness rules (see [Types and Truthiness](#4-types-and-truthiness)) and then inverts it.
    ```text
    !TRUE    # => FALSE
    !0       # => TRUE
    !"hello" # => FALSE
    ```

## 3. Operators and Precedence

Operators combine expressions to form more complex expressions. Higher number = binds tighter. Use parentheses whenever in doubt.

| Precedence | Operator | Description | Associativity |
| :---: | :---: | :--- | :---: |
| **30** | `F X` · `x.y` | Function application / Sub-contexting | Left |
| **27** | `x ^ y` | Exponentiation | Right |
| **26** | `x * y` · `x / y` · `x % y` | Multiplicative arithmetic | Left |
| **25** | `x + y` · `x - y` | Additive arithmetic / Concatenation | Left |
| **24** | `==` · `!=` · `<` · `<=` · `>` · `>=` | Comparison operators | Non-assoc |
| **23** | `x & y` · `x \| y` | Logical AND / Logical OR | Left |
| **22** | `x ? y` | Conditional (returns `y` if `x` is truthy) | Left |
| **21** | `x ; y` | Alternative (returns `x` unless `Nothing`, then `y`) | Left |
| **15** | `params -> body` | Function definition | Right |
| **14** | `f >> g` | Function composition | Right |
| **14** | `g << f` | Reverse composition | Left |
| **13** | `x => f` | Element-wise mapping | Left |
| **12** | `name = x` · `name : x` | Assignment / Labelling | Right |
| **10** | `x , y` | Pairing (Tuple construction) | Left |

### Reading the table
*   **Function application** is written by juxtaposition (`f x`) and binds tighter than any other operator. You can also use parentheses (`f(x)`).
    ```text
    double = x -> x * 2
    double 2 + 3    # => 7 (Equivalent to (double 2) + 3)
    double (2 + 3)  # => 10
    ```
*   Arithmetic follows standard rules:
    ```text
    2 + 3 * 4       # => 14
    ```
*   Comparison operators bind tighter than logical AND (`&`) and OR (`|`):
    ```text
    a = 5
    b = 10
    a > 1 & b < 20  # => TRUE
    ```

### Notes
*   **Comma gotcha**: The comma operator `,` binds loosest (precedence 10).
    ```text
    x = 1, 2        # => 2. This means (x = 1), 2. `x` is assigned `1`.
    x               # => 1
    x = (1, 2)      # => () Assigns the tuple (1, 2) to x.
    x               # => (1, 2)
    ```
*   **Parameter gotcha**: When defining functions with tuple parameters or tuple bodies, wrap them in parentheses.
    ```text
    f = x -> x, 1   # => (x -> x), 1. This means (f = x -> x), 1.
    f               # => x -> x
    f = (x -> (x, 1)) # => ()
    f 5             # => (5, 1)
    ```
*   When mixing `&` and `|` (which have the same precedence and are left-associative), you should use parentheses to make the order of evaluation explicit if it matters.
*   Comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`) are non-associative. This means `a < b < c` is not allowed. Instead, use `a < b & b < c`.

## 4. Types and Truthiness

Swan supports several types for values:

| Type | Syntax / Example | Description |
| :--- | :--- | :--- |
| `Nothing` | `()` | The unit value, representing the absence of a value. |
| `Bool` | `TRUE`, `FALSE` | Boolean truth values. |
| `Numb` | `42`, `3.14`, `-0.5` | Double-precision floating-point numbers. |
| `Text` | `"hello"`, `'world'` | UTF-8 string value. |
| `List` | `[1, 2, 3]` | An ordered, integer-indexed collection of values. |
| `Tuple` | `1, 2, "a"` | A group of values separated by commas. |
| `Namespace` | `{a: 1, b: 2}` | A collection of named key-value pairs (bindings). |
| `Func` | `x -> x * 2` | A function you define, or one that is built in. |
| `Promise` | `promise.create()` | An asynchronous non-rejecting value container. |
| `Undefined` | *(result of invalid op)* | An error value indicating an invalid operation. |

### Truthiness Rules
When Swan needs to convert a value to a boolean (e.g., in `!`, `&`, `|`, `?` operations), it uses these rules:
*   **Falsy**: `Nothing`, `FALSE`, `0`, `""` (empty text), `[]` (empty list), `{}` (empty namespace), and `Undefined`.
*   **Truthy**: All other values, including non-zero numbers, non-empty strings, non-empty lists, non-empty namespaces, functions, and tuples containing at least one truthy element.

### Comparing values
Swan provides comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`).
*   Numbers compare numerically.
*   Text compares alphabetically (lexicographically).
*   Lists and tuples compare element by element.
*   `Nothing` is considered less than any other value.

### Type checking
You can get the type name of any value as a `Text` string using `type x`.
```text
type 42         # => "Numb"
type "hello"    # => "Text"
type (1, 2)     # => "Tuple"
type ()         # => "Nothing"
```

## 5. Tuples

Tuples are ordered, immutable groups of values. They are created with the comma operator `,`.

```text
1, 2, "three"   # => (1, 2, "three")
(1, "b")        # => (1, "b")
```

*   A single item followed by a comma creates a 1-tuple, which normalizes to the item itself.
    ```text
    (1,)            # => 1
    1,              # => 1
    ```
*   A 0-tuple (empty parentheses) normalizes to `Nothing`.
    ```text
    ()              # => ()
    ```
*   `Nothing` items inside tuples are skipped automatically.
    ```text
    (1, (), 2)      # => (1, 2)
    ```
*   Tuples automatically flatten nested tuples.
    ```text
    (1, (2, 3))     # => (1, 2, 3)
    ```

### Tuple Broadcasting
Binary operations on tuples automatically work item-by-item across the tuple elements.
```text
(10, 20) + (1, 2)       # => (11, 22)
(10, 20) * 2            # => (20, 40)
```
If tuple lengths differ during broadcasting, missing elements are padded with `Nothing`.
```text
(1, 2) + (10, 20, 30)   # => (11, 22, 30) (Because 2 + () is 2, etc.)
```

### Notes
*   Because `Nothing` items disappear from tuples, tuples are also how you sequence expressions (see [Sequencing expressions](#8-variables-scope-and-sequencing)).

## 6. Lists

Lists are ordered collections enclosed in `[...]`. Unlike tuples, lists can be modified.

```text
nums = [1, 2, 3] # => ()
```

### Concatenation
You can combine two lists using the `+` operator.
```text
[1, 2] + [3, 4]       # => [1, 2, 3, 4]
```

### Indexing
You retrieve an element from a list by applying the list as a function with a 0-based index.
```text
nums = [1, 2, 3]
nums 0                # => 1
nums 1                # => 2
```
Accessing an index out of bounds yields `Nothing`.
```text
nums 10               # => ()
```

### Notes
*   Lists, unlike tuples, do not flatten automatically and do not drop `Nothing` items.
    ```text
    [1, (2, 3)]     # => [1, (2, 3)]
    [1, (), 2]      # => [1, (), 2]
    ```

## 7. Namespaces

Namespaces are collections of named bindings (key-value pairs) and are created using `{ ... }`. A namespace's body is evaluated in its own fresh scope, and the bindings it creates become its members.

```text
point = {x = 10, y = 20} # => ()
```

### Accessing Members
You can access members in two ways:

1.  **Sub-contexting (`.` operator)**: The `.` operator evaluates an expression within the namespace's context.
    ```text
    point.x              # => 10
    point.(x + y)        # => 30
    ```
2.  **Function Application**: You can treat a namespace like a function and pass a `Text` key to retrieve its value.
    ```text
    point "x"            # => 10
    ```
    Accessing a missing key yields `Nothing`.
    ```text
    point "z"            # => ()
    ```

### Notes
*   For functions to inspect namespace scopes, see `nspace.enum`, `nspace.size`, and `nspace.parent` in the [Built-in Reference](#12-built-in-reference).

## 8. Variables, Scope and Sequencing

### Assignment `=`
The assignment operator `=` binds a name to a value in the current scope. It always evaluates to `Nothing`.
```text
x = 10                # => ()
x                     # => 10
```

### Labelling `:`
The labelling operator `:` also binds a name to a value but evaluates to the bound value.
```text
x : 10                # => 10
x                     # => 10
```

### Destructuring
You can assign multiple variables at once by destructuring a tuple of names with a tuple of values.
```text
(a, b) = (10, 20)     # => ()
a                     # => 10
b                     # => 20
```

### Scope
*   **Closures**: Functions in Swan are closures. They "remember" the variables that were in scope where they were defined, even if called elsewhere.
    ```text
    x = 10
    f = () -> x * 2   # f "captures" x
    x = 20            # Changing x *before* f is called changes f's result
    f()               # => 40
    ```
*   Parameters and assignments inside a function are local to that function call.
*   Namespaces and function calls create child scopes that can read bindings from their parent scopes.

### Sequencing expressions
Because the assignment operator `=` evaluates to `Nothing`, and `Nothing` vanishes from tuples, you can sequence multiple expressions by separating them with commas. The entire sequence will evaluate to the value of the last expression.
```text
x = 10, y = 5, x + y  # => 15
```
This works because `x = 10` evaluates to `()`, and `y = 5` evaluates to `()`. So the expression becomes `(), (), x + y`, which simplifies to `x + y` because `Nothing` is skipped in tuples.

## 9. Conditionals and Logic

Swan provides operators for conditional execution and logical operations.

### Conditional (`?`) and Alternative (`;`)
*   The **conditional operator `?`** returns its right operand if its left operand (the condition) is truthy. Otherwise, it returns `Nothing`.
    ```text
    TRUE ? "yes"    # => "yes"
    FALSE ? "yes"   # => ()
    ```
*   The **alternative operator `;`** evaluates its right operand lazily *only if* its left operand is `Nothing`.
    ```text
    () ; "fallback" # => "fallback"
    "value" ; "fallback" # => "value"
    ```

By combining `?` and `;`, you can create standard if-else logic:
```text
age = 18
status = age >= 18 ? "Adult" ; "Minor"   # => "Adult"
```
Let's break `age >= 18 ? "Adult" ; "Minor"` down:
1.  `age >= 18` evaluates to `TRUE`.
2.  `TRUE ? "Adult"` evaluates to `"Adult"`.
3.  The expression becomes `"Adult" ; "Minor"`. Since `"Adult"` is not `Nothing`, the right side (`"Minor"`) is not evaluated, and the result is `"Adult"`.

If `age = 10`:
1.  `age >= 18` evaluates to `FALSE`.
2.  `FALSE ? "Adult"` evaluates to `Nothing`.
3.  The expression becomes `() ; "Minor"`. Since the left side is `Nothing`, the right side is evaluated, and the result is `"Minor"`.

You can also chain them for multiple conditions:
```text
grade = score -> score >= 90 ? "A" ; score >= 80 ? "B" ; "C"
grade 95                  # => "A"
grade 85                  # => "B"
grade 75                  # => "C"
```

### Notes
*   The alternative operator `;` only reacts to `Nothing`, not to other falsy values.
    ```text
    FALSE ; 5     # => FALSE
    0 ; 5         # => 0
    "" ; "hello"  # => ""
    ```
*   If the "true" branch of `?` evaluates to `Nothing`, it will "fall through" to the `;` branch.
    ```text
    condition = TRUE
    result = condition ? () ; "fallback" # => "fallback"
    ```

### Short-Circuit Logic (`&` and `|`)
These operators perform logical AND and OR operations with short-circuiting behavior. They return one of their operands, not necessarily a `Bool`.

*   `A & B` (Logical AND): If `A` is falsy, it returns `A`. Otherwise, it returns `B`.
    ```text
    TRUE & "hello"  # => "hello"
    FALSE & 10      # => FALSE
    0 & "world"     # => 0
    ```
*   `A | B` (Logical OR): If `A` is truthy, it returns `A`. Otherwise, it returns `B`.
    ```text
    TRUE | "hello"  # => TRUE
    FALSE | 10      # => 10
    0 | "world"     # => "world"
    ```

## 10. Functions

Functions are blocks of code that can be executed. They are first-class values in Swan.

<h3>Defining and Calling</h3>
Functions are defined using the arrow syntax `params -> body`.
You can call a function by juxtaposing it with its argument (`f x`) or by using parentheses (`f(x)`).

```text
double = x -> x * 2 # Define a function called 'double'
double 5            # => 10
double(10)          # => 20
```
If a function takes no meaningful arguments, or you don't need to pass one, call it with `()`.
```text
f = () -> "hello"
f()                 # => "hello"
```

### Several Parameters
For functions with multiple parameters, use a tuple for `params`.
```text
add = (a, b) -> a + b
add(3, 4)           # => 7
```
Parameters can also be destructured.
```text
swap = (a, b) -> (b, a)
(x, y) = swap(1, 2) # => ()
x                   # => 2
y                   # => 1
```

### Currying
Functions can be defined to take parameters one at a time, a technique called currying.
```text
add_curried = a -> b -> a + b
add_curried 3 4         # => 7
inc = add_curried 1     # Create a new function 'inc' that adds 1
inc 10                  # => 11
```

### Closures
Functions form closures, meaning they capture and remember the environment (variables) where they were created.
```text
factory = multiplier -> {
    (val) -> val * multiplier
}
# 'make_double' remembers 'multiplier' as 2
make_double = factory 2
make_double 5               # => 10
```

### Composition `>>` and `<<`
Functions can be composed to create new functions.
*   `f >> g`: Applies `f` first, then `g`.
*   `g << f`: Applies `f` first, then `g` (reads as `g` after `f`).

```text
add1 = x -> x + 1
double = x -> x * 2

pipeline = add1 >> double
pipeline 3              # => 8 (first 3 + 1 = 4, then 4 * 2 = 8)

reverse_pipeline = double << add1
reverse_pipeline 3      # => 8 (first 3 + 1 = 4, then 4 * 2 = 8)
```

### Mapping `=>`
The `=>` operator applies a function (or a tuple of functions) to an argument (or a tuple of arguments). It performs element-wise application.
```text
(1, 2, 3) => (x -> x * 10)   # => (10, 20, 30)
```
Applying a built-in function to a tuple maps it over each element.
```text
(4, 9, 16) => math.sqrt      # => (2, 3, 4)
```

## 11. Undefined Values and Errors

When an operation in Swan doesn't make sense (for example, trying to subtract a number from text like `"a" - 1`), Swan does not stop execution or "crash". Instead, it produces an `Undefined` value. This `Undefined` value records that something went wrong.

```text
"hello" - 1   # => Undefined
```
An `Undefined` value is falsy and will propagate through subsequent operations. This means your program will continue to run and finish with a value, even if some intermediate operations were invalid.
```text
("hello" - 1) + 5 # => Undefined
Undefined + 10    # => Undefined
```
This predictable error handling ensures that Swan programs always finish with a value.

## 12. Built-in Reference

Swan provides several built-in functions and namespaces for common tasks.

### Root Functions & Constants
These are available directly in the global scope.

*   `type x`: Returns the type name of `x` as `Text`.
    ```text
    type 10         # => "Numb"
    ```
*   `parse code_text`: Turns Swan source text (a `Text` value) into a function. Applying this function to a namespace evaluates the code within that namespace.
    ```text
    code = "x + 1"
    eval_code = parse code
    ns = {x: 10}
    eval_code ns    # => 11
    ```
*   `bind names`: Binds global variables. Evaluates to `Nothing`.
    ```text
    bind x = 10, y = 20 # => ()
    ```
*   `label names`: Binds global variables and evaluates to the bound values.
    ```text
    label x = 10, y = 20 # => (10, 20)
    ```
*   `TRUE`: The boolean truth value.
*   `FALSE`: The boolean false value.

### `math` Namespace
Contains mathematical constants and operations.

#### Constants
*   `math.PI`: The mathematical constant $\pi$.
*   `math.E`: The mathematical constant $e$.
*   `math.INF`, `math.INFINITY`: Represents positive infinity.

#### Single-Argument Functions
These functions take one number argument.
*   `math.sqrt x`: Square root of `x`.
*   `math.abs x`: Absolute value of `x`.
*   `math.floor x`: Largest integer less than or equal to `x`.
*   `math.ceil x`: Smallest integer greater than or equal to `x`.
*   `math.round x`: `x` rounded to the nearest integer.
*   `math.trunc x`: Integer part of `x`.
*   `math.sin x`, `math.cos x`, `math.tan x`: Trigonometric sine, cosine, tangent.
*   `math.asin x`, `math.acos x`, `math.atan x`: Inverse trigonometric functions.
*   `math.sinh x`, `math.cosh x`, `math.tanh x`: Hyperbolic sine, cosine, tangent.
*   `math.asinh x`, `math.acosh x`, `math.atanh x`: Inverse hyperbolic functions.
*   `math.exp x`: $e$ raised to the power of `x`.
*   `math.ln x`: Natural logarithm of `x`.
*   `math.log2 x`: Base-2 logarithm of `x`.
*   `math.log10 x`: Base-10 logarithm of `x`.

#### Curried Two-Argument Functions
These functions expect two arguments and are curried (e.g., $f(\text{arg}_1) \to g(\text{arg}_2)$).
*   `math.pow exp base`: Computes `base ^ exp`.
    ```text
    pow10 = math.pow 10
    pow10 2               # => 1024
    ```
*   `math.log base x`: Computes logarithm of `x` in `base`.
*   `math.atan2 x y`: Computes arc tangent of `y / x`.
*   `math.hypot x y`: Computes $\sqrt{x^2 + y^2}$.

#### Variadic Functions
These functions can take a variable number of arguments.
*   `math.max(...)`: Returns the maximum numeric value among arguments.
    ```text
    math.max(1, 5, 2)     # => 5
    ```
*   `math.min(...)`: Returns the minimum numeric value among arguments.
    ```text
    math.min(1, 5, 2)     # => 1
    ```

### `nspace` Namespace
Provides functions for inspecting namespace scopes.
*   `nspace.enum ns`: Returns a sorted tuple of locally bound names in `ns` as `Text` values.
    ```text
    ns = {a: 1, b: 2}
    nspace.enum ns          # => ("a", "b")
    ```
*   `nspace.size ns`: Returns the number of local bindings in `ns`.
    ```text
    ns = {a: 1, b: 2}
    nspace.size ns          # => 2
    ```
*   `nspace.parent ns`: Returns the enclosing parent namespace of `ns`, or `Nothing` if `ns` is a top-level namespace.

### `promise` Namespace
Used for non-rejecting asynchronous workflows.
*   `promise.create()`: Returns a new pending `Promise` value.
    ```text
    p = promise.create() # => (Promise value)
    ```
*   `promise.resolve p v`: Resolves the promise `p` with the value `v`. If `v` is an error or `Undefined`, the promise resolves to `Nothing`.
    ```text
    p = promise.create()
    promise.resolve p 42 # => ()
    ```
*   `P ~> F`: This operator chains promise execution. It runs function `F` when promise `P` gets its value.
    ```text
    p = promise.create()
    promise.resolve p 42
    result = p ~> (v -> v + 1)
    result                # => 43
    ```
*   Promises never reject; resolving with an `Undefined` value means the promise will eventually hold `Nothing`.

## 13. Worked Examples

Here are some short, self-contained examples to illustrate Swan concepts.

### Factorial Calculation
A classic example of recursion using `self`.
```text
fact = n -> n <= 1 ? 1 ; n * self(n - 1)
fact 5                  # => 120
```

### Grade Classification
Using chained conditionals for multi-way branching.
```text
grade_calculator = score -> score >= 90 ? "A" ; score >= 80 ? "B" ; score >= 70 ? "C" ; "D"
grade_calculator 85     # => "B"
```

### Element-wise Tuple Operations
Arithmetic on tuples works element by element, and you can map functions over them.
```text
coords = (10, 20)
offset = (1, 5)
new_coords = coords + offset       # => (11, 25)
lengths = (3, 4, 12)
(lengths => math.sqrt)             # => (1.73205081, 2, 3.46410162)
```

### Using a Namespace as a Record
Namespaces are ideal for bundling related data.
```text
person = {name: "Alice", age: 30}
person.name                     # => "Alice"
person.age + 1                  # => 31
```

## 14. Common Pitfalls

Be aware of these common issues when writing Swan code:

*   **Comma operator binding**: `x = 1, 2` binds only `1` to `x`. If you want a tuple, write `x = (1, 2)`.
*   **Chained comparisons**: `a < b < c` is not allowed. Use `a < b & b < c`.
*   **Alternative operator (`:`)**: It only reacts to `Nothing`, not to `FALSE` or `0`. `FALSE ; 5` evaluates to `FALSE`.
*   **Indexing**: List and tuple indexing is 0-based. The first element is at index 0.
*   **Assignment value**: `x = 10` evaluates to `Nothing`. If you need the value, use `x : 10`.
*   **Tuple parameters**: When defining a function with multiple parameters as a tuple, always parenthesize the parameter list: `(a, b) -> a + b`.
