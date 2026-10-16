# Waiting for values

Some values are not available right away: an imported document has to be
loaded, and a host function may have to fetch data from a server. Such values
are [promises](../swan/promises.md).

## Tags that return a promise

When a tag's value is a promise, rendering stops at that tag until the promise
resolves, writes the resolved value, and then continues with the rest of the
template. You do not have to do anything special:

```texl
<% 20 ~> (n -> n + 1) %> is a promise of 21.
```

```text
21 is a promise of 21.
```

Promises usually come from:

- [`texl.import`](documents.md), which loads a document;
- functions provided by the host. In a web page, a JavaScript `async`
  function passed to the template returns a promise.

## Transforming the value

Use `~>` to apply a function to the value before it is written. If a host
function `fetch_rate` returned a promise of an exchange rate, you would write:

```texl,nocheck
1 EUR = <% fetch_rate "USD" ~> (rate -> text.from (math.round (rate * 100) / 100)) %> USD
```

## Using the value in later tags

The function after `~>` can bind the value to a global name with `bind`. The
tag waits for the promise, so the name is bound before the next tag runs:

```texl
<% 42 ~> (n -> bind "answer" n) %>The answer is <% answer %>.
```

```text
The answer is 42.
```

This is the usual way to load a [library](documents.md#using-a-library).

## Several promises

Rendering only waits for a tag whose value *is* a promise. A tuple that
contains promises is written right away, and a promise that is still pending
is written as `[[Pending Promise]]`:

```texl
<% promise.create (), 1 %>
```

```text
[[Pending Promise]], 1
```

To wait for several promises, nest the callbacks, so that the tag returns a
single promise:

```texl,nocheck
<% texl.import("a.texl") ~> (a -> texl.import("b.texl") ~> (b -> a.rendering + b.rendering)) %>
```

Or import them in separate tags, one after the other.

## Promises never fail

A promise always resolves. When the work behind it fails, for example when a
document cannot be loaded, the promise resolves to `Nothing`. A JavaScript
promise that is rejected also becomes `Nothing`. Use `;` to show a fallback.
