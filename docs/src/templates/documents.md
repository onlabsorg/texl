# Documents and imports

A *document* is a template with a location: a file path for the command-line
tool, a URL in a web page. Every template the engine renders is a document,
even one that a program renders from a string: the program gives it a
location. A document can import other
documents, to reuse pieces of text (*partials*) or collections of functions
(*libraries*).

## Importing a document

`texl.import(uri)` loads the document at `uri`, renders it, and returns a
promise of a namespace with four members:

| Member      | Content                                                    |
|-------------|------------------------------------------------------------|
| `uri`       | the full location of the document                          |
| `source`    | its template text                                          |
| `data`      | a namespace with the names its tags bound                  |
| `rendering` | its rendered text                                          |

Because the result is a [promise](../swan/promises.md), you use it with `~>`:
`texl.import(uri) ~> (doc -> ...)` applies the function to the document once
it is loaded. The tag waits for the result before rendering continues.

## Inserting a partial

Suppose `partials/footer.texl` contains:

```texl,nocheck
Questions? Write to help@example.com.
```

This template inserts it:

```texl
Thank you for your order.
<% texl.import("partials/footer.texl") ~> (doc -> doc.rendering) %>
```

```text
Thank you for your order.
Questions? Write to help@example.com.

```

The extra empty line is the line break at the end of the footer file, followed
by the line break after the tag. Use `text.trim_tail` to drop the first one:

```texl
Thank you for your order.
<% texl.import("partials/footer.texl") ~> (doc -> text.trim_tail doc.rendering) %>
```

```text
Thank you for your order.
Questions? Write to help@example.com.
```

## Each document has its own names

An imported document renders in its own scope. It sees the built-in names and
the host's globals, but not the names of the template that imports it, so it
renders the same whoever imports it. With `partials/header.texl` containing:

```texl,nocheck
== <% title ; "Untitled" %> ==
```

the title of the importer does not reach it:

```texl
<% title = "Monthly report" %><% texl.import("partials/header.texl") ~> (doc -> text.trim_tail doc.rendering) %>
```

```text
== Untitled ==
```

To produce a piece of text with data from the importer, write a function in a
library, and call it with the data (see the next section).

Within one rendering, each document is loaded and rendered **only once**:
later imports of it, even through a different relative path, return the same
promise. The next rendering loads it again, so it sees the changes made to the
document in between.

## Using a library

A library is a document that binds functions and renders nothing. Suppose
`lib/format.texl` contains:

```texl,nocheck
<%
# Formatting helpers. This document renders nothing.
plural = (n, word) -> text.from n + " " + word + (n == 1 ? "" ; "s"),
money = n -> "$" + text.from (math.round (n * 100) / 100)
%>
```

Its functions are in the `data` member of the document. Bind that namespace to
a global name with [`bind`](../swan/builtins/root.md#bind), and use it in the
following tags:

```texl
<% texl.import("lib/format.texl") ~> (lib -> bind "fmt" lib.data) %>You ordered <% fmt.plural (3, "book") %> for <% fmt.money 41.5 %>.
```

```text
You ordered 3 books for $41.5.
```

This works because the import tag returns a promise, and rendering waits for
it: by the time the next tag runs, the callback has bound `fmt`.

For a single use, call the function inside the callback instead:

```texl
<% texl.import("lib/format.texl") ~> (lib -> lib.data.plural (1, "book")) %>
```

```text
1 book
```

## Locations

The `uri` of `texl.import` is resolved against the location of the importing
document, the way links work in a web page:

| `uri`                         | Refers to                                                 |
|-------------------------------|-----------------------------------------------------------|
| `footer.texl`                 | a file in the same folder as the importing document       |
| `partials/footer.texl`        | a file in a subfolder of that folder                      |
| `../shared/footer.texl`       | a file in a sibling folder                                |
| `/shared/footer.texl`         | a path from the root: the root of the web site on the web, the root of the file system on the command line |
| `https://example.com/a.texl`  | a full URL (on the web)                                   |
| `file:///home/me/a.texl`      | a full file URL (on the command line)                     |

A template that a program renders from a string resolves relative locations
against the location the program gives it.

Imports inside an imported document are resolved against *its* location, not
the location of the first template. So a partial can import its own
neighbors with simple relative paths.

## Missing documents

If a document cannot be loaded, the promise resolves to `Nothing`. Give a
fallback with `;`:

```texl
<% texl.import("missing.texl") ~> (doc -> doc.rendering ; "(not found)") %>
```

```text
(not found)
```

## Import cycles

A document that imports itself, directly or through other documents, would
wait for itself forever. Instead, the import that closes the cycle returns
[`Undefined`](../swan/undefined.md), not a promise:

```texl
<% me = texl.import("cycle.texl") %>[<% me %>]
```

```text
[[[Undefined]]]
```

(The example is rendered as the document `cycle.texl`.) In a chain where `a`
imports `b` and `b` imports `a`, the import of `a` in `b` returns `Undefined`,
and `b` renders without it.

## What a document can see

A document is rendered:

- with the built-in names and the host's globals;
- with its own `texl` namespace, which imports relative to its own location.

It does not see the names of the template that imported it. `bind` and
`label` bind names in the global scope of the document, where every tag of the
document sees them but no other document does. Its `data` contains the names
its own tags bound with `=` and `:`, plus `texl`, but not the names bound with
`bind` or `label`.
