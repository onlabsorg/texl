# Promises

A promise stands for a value that may not be available yet, such as a document
that is still loading. It starts *pending* and later *resolves* to a value.

Promises usually come from the host or from the engine: `texl.import` returns
one, and so does a JavaScript `async` function passed to a template. You can
also create them yourself with the [`promise`](builtins/promise.md) namespace.

Swan promises are simple:

- A promise resolves at most once. Later attempts are ignored.
- A promise never fails. If it resolves to an `Undefined` value, it holds
  `Nothing` instead.

## Using a value that arrives later

Swan code cannot stop and wait. Instead, you give the promise a function to
apply to its value. `p ~> f` returns a new promise, which resolves to `f`
applied to the value of `p`:

```swan
p = promise.create()
q = p ~> (x -> x * 2)
promise.resolve p 21
q                    # => Promise(42)
```

If `p` is already resolved, `f` runs immediately. Otherwise it runs when `p`
resolves. Several functions can wait on the same promise.

To chain several steps, put parentheses around each step. `~>` is
right-associative, so without them `p ~> f ~> g` means `p ~> (f ~> g)`, which
does not do what you want:

```swan
p = promise.create()
q = (p ~> (x -> x + 1)) ~> (x -> x * 10)
promise.resolve p 1
q                    # => Promise(20)
```

Or compose the steps into one function: `p ~> (f >> g)`.

A value that is not a promise is treated as an already resolved promise:

```swan
3 ~> (x -> x + 1)    # => Promise(4)
```

A tuple of promises gives a tuple of new promises, and a tuple of functions
gives one new promise per function.

## Keeping the result

The function you pass to `~>` runs later, possibly after the rest of the
expression. To make its result available to later code, bind it globally with
[`bind`](builtins/root.md#bind):

```swan
p = promise.create()
p ~> (v -> bind "result" v)
promise.resolve p "done"
result               # => "done"
```

## Promises in templates

When a tag of a template returns a promise, rendering waits until the promise
resolves and writes its value. This is how a template uses an imported
document:

```texl,nocheck
<% texl.import("header.texl") ~> (doc -> doc.rendering) %>
```

The next tags run only after the promise resolves, so they can use what the
callback bound. See [Waiting for values](../templates/async.md).

## Truth value

A promise has no truth value. Test the value it resolves to, not the promise.

> **Known issue.** Using a promise as a condition (with `?`, `&`, `|`, `!` or
> `bool.from`) currently stops the program.
