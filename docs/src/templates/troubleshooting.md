# Troubleshooting

Texl never stops with an error. When something goes wrong, the output shows it
in one of the ways below.

## The output contains [[Undefined]]

An operation in the tag was invalid. Find out which one by inspecting the
value with [`undef.inspect`](../swan/builtins/undef.md#undefinspect). Show the
members of the description, or a few of them:

```texl
<% e = undef.inspect ("Total" + 10) %><% e.type %>: <% e.op %> of <% e.lhv %> and <% e.rhv %>
```

```text
BinaryOperation: sum of Total and 10
```

Here the sum of a text and a number is invalid. The fix is
`"Total" + text.from 10`.

`pos` tells where the error is, counted in bytes from the start of the tag's
content (just after `<%`). Errors raised by built-in functions report 0.

The most common causes are:

- mixing types: `"Total: " + 10`, or a number given to a text function;
- a missing argument, which makes it `Nothing`: `add 3` instead of `add (3, 4)`;
- calling the wrong value: `f g x` instead of `f (g x)` (see
  [pitfalls](../swan/pitfalls.md#application-is-left-associative));
- a syntax error, such as a missing bracket or a single-quoted text.

## A value is missing

If a tag writes nothing where you expected a value:

- **Check the spelling.** A misspelled name is `Nothing`, and `Nothing` writes
  nothing.
- **Check that the name is bound before the tag.** Tags run in order, and a
  name bound in a later tag is not yet available.
- **Check the host's globals.** Is the data you expect actually passed to the
  template?
- **Check `?` conditions.** `cond ? value` writes nothing when the condition
  is false.

Give optional names a default to make missing values visible:
`<% customer.name ; "MISSING NAME" %>`.

## The output contains [[List of N]], [[Namespace of N]] or [[Func]]

The tag's value is a list, a namespace or a function, which have no text
form.

- For a list, write its items: `list.enum items`, or
  `text.join ", " (list.enum items)`.
- For a namespace, pick a member: `customer.name`.
- For a function, you probably forgot to call it: `today ()` instead of
  `today`.

## The output contains [[Pending Promise]]

A promise inside a tuple, or inside another value, was not ready when the tag
was written. Only a promise returned by the tag itself is awaited. See
[Several promises](async.md#several-promises).

## Values are separated by commas

The tag's value is a tuple, which is written as its items separated by `, `.
Often this is a sequence of steps whose intermediate values are not `Nothing`:
use `=` rather than `:` for the intermediate steps. To join items with another
separator, use `text.join`.

## Unexpected empty lines

A tag alone on its line leaves the line break behind. See
[Line breaks around tags](tags.md#line-breaks-around-tags).

## Rendering crashes with a stack overflow

A recursive function never reaches the case that stops it. A common cause is a
recursive call after `?` instead of after `;`: both sides of `?` are always
evaluated. See
[Evaluation order](../swan/conditions.md#evaluation-order).

## Part of the template is copied as it is

A `<%` without a matching `%>` makes the rest of the template plain text. A
`%>` inside a text literal ends the tag early. See
[Writing the delimiters as text](tags.md#writing-the-delimiters-as-text).

## Known issues

- Infinite recursion stops the program instead of producing `Undefined`.
