# Templates

A Texl template is ordinary text with *tags*. A tag starts with `<%` and ends
with `%>`, and contains an expression in the [Swan](../swan/index.md)
language. Rendering the template replaces each tag with the value of its
expression, and keeps the rest of the text as it is.

```texl
<% name = "Ada", items = 3 %>Dear <% name %>,
your order of <% items %> items has shipped.
```

renders as

```text
Dear Ada,
your order of 3 items has shipped.
```

Templates can produce any kind of text: Markdown, HTML, configuration files,
source code, e-mails. Texl does not care about the format; it only replaces
the tags.

## How rendering works

1. The text outside the tags is copied to the output unchanged.
2. The tags are evaluated in order, from the start of the template to the end.
3. Each tag is replaced by its value, converted to text. A tag whose value is
   `Nothing`, like an assignment, produces no text at all.
4. All the tags of a template share one set of names. A name bound in one tag
   is available in every later tag.
5. If a tag's value is a promise, rendering waits until it resolves, then
   writes the resolved value.

## Where templates come from

You write a template in a file, usually with the `.texl` extension, or in a
string inside a program. A *host* renders it:

- the [JavaScript library](../hosts/javascript.md) renders templates in a web
  page;
- the [Python library](../hosts/python.md) renders templates in a Python
  program;
- the [Rust library](../hosts/rust.md) renders templates in a Rust program.

A Node.js library is coming soon.

The host can also give your template data to work with, such as the name of
the reader or a list of products. See [Names and data](names.md).

A template that a host loads by its file name or URL is called a *document*.
Documents can [import](documents.md) other documents, to reuse a header or a
library of functions.

## What to read next

- [Tags and output](tags.md): how tags are written and how values become text.
- [Names and data](names.md): where the names in your expressions come from.
- [Conditions and repetition](logic.md): optional text, lists and tables.
- [Documents and imports](documents.md): splitting your work into several files.
- [Waiting for values](async.md): data that arrives later.
- [Troubleshooting](troubleshooting.md): when the output is not what you expect.
- [A complete example](example.md): all of it together.

The expressions themselves are covered in the [Swan manual](../swan/index.md).
You can write useful templates knowing only a few parts of it: literals,
names, [operators](../swan/operators.md), [conditions](../swan/conditions.md)
and [functions](../swan/functions.md).
