# Names and data

The expressions in your tags use names: `price`, `customer.name`,
`text.upper`. A name can come from four places.

## The names you bind

A tag can bind names with `=` or `:`, and every later tag of the same
template can use them. It is common to bind the values a template needs in a
first tag:

```texl
<%
company = "Acme",
year = 2026,
copyright = "© " + text.from year + " " + company
%>Contact <% company %> for details.
<% copyright %>
```

```text
Contact Acme for details.
© 2026 Acme
```

Functions work the same way. Define them once, and call them in later tags:

```texl
<% money = n -> "$" + text.from (math.round (n * 100) / 100) %>Subtotal: <% money 19.999 %>
Tax: <% money (19.999 * 0.2) %>
```

```text
Subtotal: $20
Tax: $4
```

## Built-in names

Every template can use the [built-in library](../swan/builtins/index.md):
`text`, `list`, `math`, `time` and the other namespaces, plus `TRUE`, `FALSE`,
`type` and a few more. The engine adds the [`texl`](../swan/builtins/texl.md)
namespace, which imports other documents.

## Data from the host

The program that renders your template can bind *globals*: names with the data
the template needs. Which globals exist depends on the host, so a template
usually documents what it expects, for example in a comment at the top:

```texl,nocheck
<%
# Expects:
#   customer  {name, email}
#   orders    a list of {id, total}
%>
```

Values from the host become Swan values: strings become `Text`, numbers
`Numb`, arrays `List`, objects namespaces, and functions Swan functions. See
the host pages for how to pass globals: [JavaScript](../hosts/javascript.md#globals-and-context-values)
and [Rust](../hosts/rust.md#globals-and-host-functions). The command-line tool
does not pass any.

## Names from other documents

A document can import another document and use the names it binds. See
[Using a library](documents.md#using-a-library).

## Missing names

A name that is not bound anywhere is `Nothing`, which writes nothing. This is
handy for optional data, but it also hides typos. Give optional names a
default with `;`:

```texl
Hello, <% reader ; "guest" %>!
```

```text
Hello, guest!
```

## Scopes of templates and documents

Each rendering of a template gets its own scope for the names it binds, below
the globals and the built-ins. Binding a global or built-in name in a template
hides it for the rest of that template only.

An imported document is rendered in a scope below the scope of the template
that imports it, so it can read the importer's names (see
[Passing data to a partial](documents.md#passing-data-to-a-partial)).

### Engine-wide names

[`bind`](../swan/builtins/root.md#bind) binds a name in the global scope of
the engine. The name is then visible in every template and document that the
engine renders afterwards, not only in the current one. That is how a
document shares a library with its importer (see
[Using a library](documents.md#using-a-library)), but it also means that two
documents binding the same global name overwrite each other. Choose distinctive
names for globals.
