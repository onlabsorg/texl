# Introduction

Texl is a template engine for text. A Texl template is any text, such as
Markdown, HTML or a configuration file, with expressions between `<%` and
`%>`. Rendering the template replaces each expression with its value.

```texl
<% user = {name: "Ada", tasks: ["review", "deploy"]} %>
Hi <% user.name %>, 
you have <% list.size user.tasks %> open tasks: <% list.enum user.tasks %>.
```

```text
Hi Ada,
you have 2 open tasks: review, deploy.
```

The expressions are written in **Swan**, a small language designed for this
job. Swan has values, operators, functions and namespaces, but no statements
and no exceptions: every expression produces a value, and an invalid operation
produces an `Undefined` value instead of stopping the rendering.

Templates can import other templates, by relative path or URL, to share
headers, footers and libraries of functions. They can also wait for data that
arrives later, such as a document fetched over the network.

## Where Texl runs

Texl is written in Rust, and the same engine runs in several hosts:

| Host                                   | Status      |
|----------------------------------------|-------------|
| [Command line](hosts/cli.md)           | available   |
| [JavaScript, in the browser](hosts/javascript.md) | available |
| [Rust](hosts/rust.md)                  | available   |
| [Python](hosts/python.md)              | available   |
| Node.js                                | coming soon |

## How this manual is organized

- **[Getting started](getting-started.md)** renders your first template.
- **[Writing templates](templates/index.md)** explains how to write templates:
  tags, data, conditions, repetition and imports. Start here if you write
  templates.
- **[The Swan language](swan/index.md)** describes the expression language in
  detail, with a reference of every [built-in function](swan/builtins/index.md).
- **[Running texl](hosts/index.md)** explains how to render templates from the
  command line, JavaScript, Python and Rust, with the API reference of each.

## Project status

Texl is young: the current version is 0.1. The language and the APIs may still
change. The source code, the issue tracker and the change history are on
[GitHub](https://github.com/onlabsorg/texl). Texl is released under the MIT
license.
