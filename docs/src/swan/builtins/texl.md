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

If the document cannot be loaded, the promise resolves to `Nothing`.

A relative `uri` is resolved against the URI of the importing document. Each
document is loaded and rendered at most once per engine: importing it again,
even through a different relative path, returns the same result.

```texl,nocheck
<% texl.import("partials/header.texl") ~> (doc -> doc.rendering) %>
```

See [Documents and imports](../../templates/documents.md) for the details.
