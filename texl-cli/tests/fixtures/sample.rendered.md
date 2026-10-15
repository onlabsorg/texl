Texl Sample Document
====================

Text outside the tags is copied as it is. Each tag is replaced by the value of
its expression. Tags run in order and share one scope, so names assigned in a
tag (like `title`, in the first tag of this file) can be used by later tags.


1. Literals and types
---------------------

  42                       -> 42
  3.14                     -> 3.14
  -0.5                     -> -0.5
  "hello"                  -> hello
  TRUE                     -> TRUE
  ()                       -> []   (Nothing renders as empty text)
  type 42                  -> Numb
  type "hello"             -> Text
  type [1, 2]              -> List
  type {a: 1}              -> Namespace
  type (x -> x)            -> Func


2. Arithmetic
-------------

  2 + 3 * 4                -> 14
  (2 + 3) * 4              -> 20
  2 ^ 10                   -> 1024
  7 / 2                    -> 3.5
  7 % 3                    -> 1
  "Te" + "xl"              -> Texl


3. Comparison and logic
-----------------------

  3 < 5                    -> TRUE
  "abc" < "abd"            -> TRUE
  [1, 2] == [1, 2]         -> TRUE
  !0                       -> TRUE
  TRUE & "hello"           -> hello
  0 | "world"              -> world
  TRUE ? "yes"             -> yes
  FALSE ? "yes"            -> []
  () ; "fallback"          -> fallback
  FALSE ; "fallback"       -> FALSE
  5 > 3 ? "big" ; "small"  -> big


4. Variables
------------

Let price = 12.5 and qty = 4

  price * qty              -> 50
  x : 7                    -> 7   (`:` binds and returns the value)
  x = 7                    -> []   (`=` binds and returns Nothing)
  (a, b) = (1, 2), (b, a)  -> 2, 1
  y = 10, z = 5, y + z     -> 15
  label "w" 99             -> 99
  w                        -> 99


5. Tuples
---------

  1, 2, "three"            -> 1, 2, three
  (1, (), (2, 3))          -> 1, 2, 3
  (10, 20) + (1, 2)        -> 11, 22
  (1, 2) == (1, 2)         -> TRUE, TRUE


6. Lists
--------

Let nums = [10, 20, 30]

  nums                     -> [[List of 3]]
  nums 1                   -> 20
  nums 10                  -> []
  list.size nums           -> 3
  list.enum nums           -> 10, 20, 30
  list.enum (nums + [40])  -> 10, 20, 30, 40
  list.reverse nums        -> 30, 20, 10   (via list.enum)
  list.head 2 nums         -> 10, 20   (via list.enum)
  list.tail 2 nums         -> 30   (via list.enum)
  list.find 20 nums        -> 1
  list.find 99 nums        -> -1


7. Namespaces
-------------

Let point = {x: 3, y: 4}

  point                    -> [[Namespace of 2]]
  point.x                  -> 3
  point.(x * y)            -> 12
  point "y"                -> 4
  nspace.enum point        -> x, y
  nspace.size point        -> 2


8. Functions
------------

Let double = x -> x * 2
Let add = (a, b) -> a + b
Let fact = n -> n <= 1 ? 1 ; n * fact(n - 1)
Let grade = s -> s >= 90 ? "A" ; s >= 80 ? "B" ; "C"

  double                   -> [[Func]]
  double 21                -> 42
  double 2 + 3             -> 7
  add(3, 4)                -> 7
  fact 10                  -> 3628800
  (95, 85, 70) => grade    -> A, B, C
  (1, 2, 3) => double      -> 2, 4, 6
  (double >> double) 5     -> 20
  (math.sqrt << double) 8  -> 4

Currying and closures:

  inc = (a -> b -> a + b) 1        inc 10    -> 11
  triple = scale 3                 triple 5  -> 15


9. Built-in namespaces
----------------------

  text.size "hello"              -> 5
  text.upper "texl"              -> TEXL
  text.lower "TEXL"              -> texl
  text.trim "  padded  "         -> [padded]
  text.find "l" "hello"          -> 2
  text.rfind "l" "hello"         -> 3
  text.head 3 "swanky"           -> swa
  text.tail 4 "swanky"           -> ky
  text.split "," "a,b,c"         -> a, b, c
  text.join "-" ("a", "b", "c")  -> a-b-c
  text.enum "abc"                -> a, b, c
  text.from 42 + "!"             -> 42!
  numb.from "41" + 1             -> 42
  bool.from ""                   -> FALSE
  bool.any (0, 1)                -> TRUE
  bool.all (0, 1)                -> FALSE
  math.PI                        -> 3.141592653589793
  math.round (math.PI * 100)     -> 314
  math.pow 10 2                  -> 1024   (2 ^ 10)
  math.max (1, 5, 2)             -> 5
  (4, 9, 16) => math.sqrt        -> 2, 3, 4
  time.to_string 1780000000      -> 2026-05-28T20:26:40.000Z
  time.from_string "2026-05-28"  -> 1779926400
  time.week_day 1780000000       -> 4   (0 is Sunday)
  time.week_number 1780000000    -> 22
  (time.to_date 0).year          -> 1970
  (parse "x + 1") {x: 41}        -> 42


10. Undefined values
--------------------

Invalid operations do not stop the rendering: they produce an Undefined value,
which spreads through the expressions that use it.

  "hello" - 1                    -> [[Undefined]]
  ("hello" - 1) + 5              -> [[Undefined]]
  !("hello" - 1)                 -> TRUE   (Undefined is falsy)
  type ("hello" - 1)             -> Undefined

Let err = undef.inspect("hello" - 1)

  err.type                       -> BinaryOperation
  err.op                         -> subtraction
  err.(lhv, rhv)                 -> hello, 1


11. Promises
------------

A tag that returns a promise makes the rendering wait until it resolves.

  p = promise.create(),
  doubled = p ~> double,
  promise.resolve p 21,
  doubled                        -> 42


12. Imports
-----------

`texl.import(uri)` returns a promise of the document namespace
{uri, source, data, rendering}. Relative URIs resolve against the folder of
the importing document.

Dear Ada, this paragraph comes from partials/greeting.texl.
It reads `reader` from the document that imported it, and it imports
"signature.texl" from its own folder: -- the Texl team (2 imports deep)

The definitions of an imported document are in its `data`. Here they are
bound to the global name `fmt`:

  fmt.plural(1, "apple")         -> 1 apple
  fmt.plural(3, "apple")         -> 3 apples
  fmt.pad 12 "Total"             -> Total.......
  fmt.price (price * qty / 3)    -> $16.67

Fields of an imported document:

  nspace.enum doc                -> data, rendering, source, uri
  fmt.basename doc.uri           -> format.texl
  text.size doc.source > 0       -> TRUE

Each document is loaded once: later imports, even through a different relative
path, share the first result.

  counted.texl has been evaluated 1 time(s)
  counted.texl has been evaluated 1 time(s)
  load_count                     -> 1

A document that cannot be loaded gives Nothing:

  missing.texl                   -> (not found)


13. How values render
---------------------

  [1, 2, 3]                      -> [[List of 3]]
  {a: 1, b: 2}                   -> [[Namespace of 2]]
  x -> x                         -> [[Func]]
  "a" - 1                        -> [[Undefined]]
  (1, "two", TRUE)               -> 1, two, TRUE
