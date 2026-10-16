# Getting started

This page renders a first template with the command-line tool. If you want to
use Texl from a program instead, see [JavaScript](hosts/javascript.md) or
[Rust](hosts/rust.md).

## Install the command-line tool

You need [Rust](https://www.rust-lang.org/tools/install) 1.85 or later. Then
install the `texl` command from the repository:

```sh
cargo install --git https://github.com/onlabsorg/texl texl-cli
```

Check that it works:

```sh
texl --help
```

## Write a template

Create a file named `hello.texl`:

```texl
<% name = "world", today = time.from_string "2026-10-16" %>Hello, <% name %>!
Today is <% text.head 10 (time.to_string today) %>.
2 + 2 = <% 2 + 2 %>
```

The first tag binds two names and writes nothing. The other tags write the
values of their expressions.

## Render it

```sh
texl hello.texl
```

The rendered text goes to the terminal:

```text
Hello, world!
Today is 2026-10-16.
2 + 2 = 4
```

To write it to a file instead, add `-o`:

```sh
texl hello.texl -o hello.txt
```

## Next steps

- Learn the [template basics](templates/index.md).
- Take the [quick tour](swan/index.md#a-quick-tour) of the Swan language.
- Read the full [command-line reference](hosts/cli.md).
