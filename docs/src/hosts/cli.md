# Command line

The `texl` command renders a template file and writes the result to the
terminal or to a file.

```sh
texl report.texl                 # print the rendering
texl report.texl -o report.md    # write it to report.md
```

## Installation

You need [Rust](https://www.rust-lang.org/tools/install) 1.85 or later.
Install the command from the repository:

```sh
cargo install --git https://github.com/onlabsorg/texl texl-cli
```

or, from a clone of the repository:

```sh
cargo install --path texl-cli
```

Both install a binary named `texl` in Cargo's `bin` folder (usually
`~/.cargo/bin`).

## Reference

### Synopsis

```text
texl [-o OUTPUT] FILE
texl -h
```

### Description

`texl` loads the template `FILE`, renders it, and writes the rendering to
standard output, or to `OUTPUT` if given. The output is exactly the rendered
text, encoded as UTF-8; `texl` adds nothing to it, not even a final line
break.

### Arguments and options

| Argument                         | Meaning                                                                 |
|----------------------------------|-------------------------------------------------------------------------|
| `FILE`                           | The template file to render. Required. A relative path is relative to the current folder. |
| `-o OUTPUT`, `--output OUTPUT`   | Writes the rendering to the file `OUTPUT` instead of standard output. The file is created, or replaced if it exists. Its folder must exist. |
| `-h`, `--help`                   | Prints a summary of the usage and exits.                                |

### Imports

`FILE` is loaded as a document, by its absolute `file://` URL. The
[imports](../templates/documents.md) inside it are resolved against its
location, whatever the current folder is:

| In `/home/me/site/index.texl`, `texl.import(...)` of | loads                     |
|------------------------------------------------------|---------------------------|
| `"header.texl"`                                      | `/home/me/site/header.texl` |
| `"../shared/footer.texl"`                            | `/home/me/shared/footer.texl` |
| `"/etc/texl/lib.texl"`                               | `/etc/texl/lib.texl`      |
| `"file:///etc/texl/lib.texl"`                        | `/etc/texl/lib.texl`      |
| `"https://example.com/a.texl"`                       | nothing: other URL schemes are not supported |

An import that cannot be read resolves to `Nothing`, and the rendering goes on.

### Globals

The command passes no globals: a template can use the built-in names and the
names it binds or imports.

### Exit status

| Status | Meaning                                                                 |
|--------|-------------------------------------------------------------------------|
| 0      | The template was rendered and written.                                  |
| 1      | `FILE` could not be read, or `OUTPUT` could not be written.             |
| 2      | The command line is invalid, for example `FILE` is missing.             |

When the status is not 0, a message on standard error explains why:

```text
texl: report.texl: No such file or directory (os error 2)
```

Errors *inside* the template do not change the status: they appear in the
output as `[[Undefined]]` (see [Troubleshooting](../templates/troubleshooting.md)).

## Examples

Render a template and page through the result:

```sh
texl notes.texl | less
```

Render every template in a folder into Markdown files:

```sh
for f in pages/*.texl; do
    texl "$f" -o "build/$(basename "$f" .texl).md"
done
```
