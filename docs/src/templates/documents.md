# Documents and imports

A *document* is a template that the engine loads by its location: a file path
for the command-line tool, a URL in a web page. A document can import other
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

## Passing data to a partial

An imported document can read the names of the template that imports it. With
`partials/header.texl` containing:

```texl,nocheck
== <% title %> ==
```

the importer decides the title:

```texl
<% title = "Monthly report" %><% texl.import("partials/header.texl") ~> (doc -> text.trim_tail doc.rendering) %>
```

```text
== Monthly report ==
```

Each document is loaded and rendered **only once** per engine, and later
imports reuse the first result. If two templates import the same partial with
different data, both get the rendering made with the data of the first one. To
produce a piece of text several times with different data, write a function
instead, in a library.

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

A template that is not a document, such as a string rendered by a program,
resolves relative locations against the root.

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

## What a document can see

An imported document is rendered:

- with the built-in names and the host's globals;
- with the names of the template that imported it, as they are at the time of
  the import;
- with its own `texl` namespace, which imports relative to its own location.

Its `data` contains the names its own tags bound, plus `texl`.
