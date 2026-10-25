# pytexl

The Python library of [Texl](../README.md): renders text templates with Swan
expressions, loads documents from local files and over HTTP(S), and lets
templates call your Python functions, `async` ones included.

```sh
pip install maturin
maturin build --release -m pytexl/Cargo.toml   # builds a wheel into target/wheels
pip install target/wheels/pytexl-*.whl
```

```python
import asyncio
import pytexl

engine = pytexl.Engine({"user": {"name": "Ada"}})
doc = asyncio.run(engine.render("hello.texl", "Hello <% user.name %>!"))
print(doc.rendering)                           # Hello Ada!
```

See the [Python API reference](https://onlabsorg.github.io/texl/book/hosts/python.html).
The docstrings of the `pytexl` package document the same API.

## Layout

| Path             | Contents                                                       |
|------------------|----------------------------------------------------------------|
| `src/`           | The native module `pytexl._pytexl` (PyO3), on top of `texl-core` |
| `python/pytexl/` | The Python package: `Engine`, `Context`, `Document`, the loaders |
| `tests/`         | The Python unit tests, their fixtures, and the cargo test that runs them |

## Tests

```sh
cargo test -p pytexl --test python          # builds the module and runs the Python tests
```

The cargo test needs `python3` (3.10 or later) with its development files; it
is skipped, with a message, if Python is missing. Set `PYTEXL_PYTHON` to pick
an interpreter. In a virtual environment with the package installed
(`maturin develop`), run the tests directly:

```sh
python -m unittest discover -s pytexl/tests -v
```

## License

[MIT](../LICENSE)
