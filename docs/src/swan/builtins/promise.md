# promise

Creating and resolving [promises](../promises.md). To use the value of a
promise, use the `~>` operator.

## `promise.create`

```text
promise.create () -> Promise
```

Returns a new pending promise. The argument is ignored.

```swan
p = promise.create ()
p                    # => Promise(pending)
```

## `promise.resolve`

```text
promise.resolve p value -> Nothing
```

Resolves the promise `p` with `value`, runs the functions waiting on `p`, and
returns `Nothing`. If `p` is already resolved, nothing happens. An undefined
`value` resolves `p` to `Nothing`. A tuple `value` is stored as a whole.

If `p` is not a promise, the result is `Undefined`.

```swan
p = promise.create ()
q = p ~> (x -> x * 2)
promise.resolve p 21
p                    # => Promise(21)
q                    # => Promise(42)
promise.resolve p 99
p                    # => Promise(21)
```
