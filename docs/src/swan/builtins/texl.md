# texl

The `texl` namespace is bound by the Texl engine in every template. It is not
available in plain Swan.

## `texl.import`

```text
texl.import uri -> Promise
```

Loads the document at `uri`, renders it, and returns a promise of a namespace
that describes it:

| Member      | Type        | Content                                               |
|-------------|-------------|-------------------------------------------------------|
| `uri`       | `Text`      | The full URI of the document                          |
| `source`    | `Text`      | The template source                                   |
| `data`      | `Namespace` | The names the document's tags bound                   |
| `rendering` | `Text`      | The rendered text                                     |

If the document cannot be loaded, the promise resolves to `Nothing`. If the
import would close a cycle (a document importing itself, directly or through
other documents), `texl.import` returns `Undefined` instead of a promise.

A relative `uri` is resolved against the URI of the importing document, and
the fragment (`#...`) is ignored. The document renders in its own scope: it
does not see the names of the importer. Within one rendering, each document is
loaded and rendered at most once: importing it again, even through a different
relative path, returns the same promise.

`texl.import` exists only in documents. In a context created by a host
program to keep names across renders, `texl` has no `import`.

```texl,nocheck
<% texl.import("partials/header.texl") ~> (doc -> doc.rendering) %>
```

See [Documents and imports](../../templates/documents.md) for the details.
