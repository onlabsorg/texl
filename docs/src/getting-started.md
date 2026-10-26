# Getting started

This page renders a first template with the Python library. If you want to
use Texl from a web page or a Rust program instead, see
[JavaScript](hosts/javascript.md) or [Rust](hosts/rust.md).

## Install the Python package

You need [Rust](https://www.rust-lang.org/tools/install) 1.85 or later and
Python 3.10 or later. Clone the repository and install the package from it:

```sh
git clone https://github.com/onlabsorg/texl
pip install ./texl/pytexl
```

Check that it works:

```sh
python -c "import pytexl"
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

Create a file named `render.py` in the same folder:

```python
import asyncio
from pathlib import Path
import pytexl

doc = asyncio.run(pytexl.Engine().load(Path("hello.texl")))
print(doc.rendering)
```

and run it:

```sh
python render.py
```

The rendered text goes to the terminal:

```text
Hello, world!
Today is 2026-10-16.
2 + 2 = 4
```

To write it to a file instead, replace the last line with
`Path("hello.txt").write_text(doc.rendering)`.

## Next steps

- Learn the [template basics](templates/index.md).
- Take the [quick tour](swan/index.md#a-quick-tour) of the Swan language.
- Read the full [Python guide](hosts/python.md).
