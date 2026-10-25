# Python

The `pytexl` package runs the Texl engine in Python programs. It renders
documents, from strings or loaded from local files and over HTTP, and lets
templates call your Python functions, `async` ones included.

## Installing the package

The package is not published on PyPI yet. Install it from a clone of the
repository. You need [Rust](https://www.rust-lang.org/tools/install) 1.85 or
later and Python 3.10 or later:

```sh
pip install ./pytexl
```

pip builds the native module with [maturin](https://www.maturin.rs). To build
a wheel instead, run `maturin build --release -m pytexl/Cargo.toml`; the wheel
is written to `target/wheels`.

## Quick start

```python
import asyncio
import pytexl

engine = pytexl.Engine({"user": {"name": "Ada"}})
doc = asyncio.run(engine.render("hello.texl", "Hello <% user.name %>!"))
print(doc.rendering)                           # Hello Ada!
```

`render` renders a source as the document at a URL, here `hello.texl` in the
current working directory. The engine is asynchronous: `render` and `load` are
coroutines, because a tag
may wait for a document being loaded or for an `async` function. In a
program that is not asynchronous, run them with `asyncio.run`. In asyncio
code, `await` them:

```python
async def main():
    engine = pytexl.Engine()
    doc = await engine.render("sum.texl", "2 + 2 = <% 2 + 2 %>")
    print(doc.rendering)                       # 2 + 2 = 4

asyncio.run(main())
```

## Loading documents

`engine.load` reads a document, renders it, and returns its rendering, source
and data:

```python
from pathlib import Path

doc = await engine.load(Path("templates/report.texl"))
if doc:
    Path("report.md").write_text(doc.rendering)
```

A path is resolved against the current working directory. A string is a URL,
resolved against the engine's base URL, which is the current working
directory by default: `engine.load("templates/report.texl")` reads the same
file. Full URLs work too:

```python
doc = await engine.load("https://example.com/templates/report.texl")
```

The engine reads documents with *loaders*, chosen by the scheme of the URL:

| URL                                  | Loader                          | Reads                     |
|--------------------------------------|---------------------------------|---------------------------|
| `file:///home/ada/report.texl`       | `loaders.load_file`             | a local file, as UTF-8    |
| `/home/ada/report.texl` (no scheme)  | `loaders.load_file`, the default loader | a local file, by absolute path |
| `http://...`, `https://...`          | `loaders.load_http`             | a document over HTTP(S)   |

Inside a document, [imports](../templates/documents.md) are resolved against
the URL of the document:

| In `file:///home/ada/docs/report.texl`, `texl.import(...)` of | reads                                   |
|---------------------------------------------------------------|-----------------------------------------|
| `"header.texl"`                                               | `/home/ada/docs/header.texl`            |
| `"../lib/format.texl"`                                        | `/home/ada/lib/format.texl`             |
| `"/srv/lib/format.texl"`                                      | `/srv/lib/format.texl`                  |
| `"https://example.com/a.texl"`                                | `https://example.com/a.texl`, over HTTP |

In a document fetched over HTTP, relative paths and absolute paths resolve
against its URL in the same way, so `"/lib/format.texl"` in
`https://example.com/docs/report.texl` fetches
`https://example.com/lib/format.texl`.

A document that cannot be read counts as a failed load: `load` gives `None`,
and `texl.import` gives `Nothing`. A response with an HTTP error status is a
failed load too.

## Rendering a source

`engine.render` renders a source that you already have, such as the text of an
editor or standard input, as the document at a URL or path, without reading
it. Its imports resolve against that location, as if the document had been
loaded from there:

```python
doc = await engine.render(Path("templates/report.texl"), source)   # imports resolve in templates/
```

A source with no real location can use any URL, for example a file in the
folder of the documents it imports. `engine.fetch` returns the source of a
document without rendering it, so `await engine.load(uri)` is
`await engine.render(uri, await engine.fetch(uri))`.

## Caching

The engine holds configuration only: its globals and loaders. Every `render`
and `load` starts from a new scope and a new import cache. Within one
rendering, each imported document is read and rendered once, however many
times it is imported; the next rendering reads it again, so it sees the
changes to the documents.

An imported document renders in its own scope: it sees the engine globals, but
not the names of the document that imports it.

### Your own loaders

A loader is a function that takes the URL of a document and returns its
source, or an awaitable of it. Register it for a scheme with `add_loader`:

```python
templates = {"mem://greeting": "Hello <% reader %>!"}

engine = pytexl.Engine({"reader": "Ada"})
engine.add_loader("mem", lambda uri: templates[uri])

doc = await engine.load("mem://greeting")
print(doc.rendering)                           # Hello Ada!
```

If the loader raises an exception, or returns something other than a string,
the document cannot be loaded. `async` loaders work the same way:

```python
async def load_from_db(uri):
    row = await db.fetch_one("SELECT source FROM templates WHERE uri = ?", uri)
    return row["source"]

engine.add_loader("db", load_from_db)
```

A loader runs every time a document is fetched: the engine does not cache
sources across renders (see [Caching](#caching)). To cache them, wrap the
loader.

`set_default_loader` replaces the loader of the URLs without a scheme. Since
every URI resolves against the base URL to a full URL, and so do the imports
in documents, the engine does not currently call it.

## Globals and context values

The values you pass to the engine and to contexts become Swan values:

| Python                                      | Swan                                          |
|---------------------------------------------|-----------------------------------------------|
| `None`                                      | `Nothing`                                     |
| `bool`                                      | `Bool`                                        |
| `int`, `float`                              | `Numb`                                        |
| `str`                                       | `Text`                                        |
| `list`                                      | `List`                                        |
| `tuple`                                     | `Tuple`                                       |
| `dict`, or any other mapping                | `Namespace` of its string keys                |
| awaitable: coroutine, future or task        | `Promise`; an exception resolves to `Nothing` |
| function, or any other callable             | `Func` (see below)                            |
| [`Promise`](#promise), [`Function`](#function), [`Undefined`](#undefined) | the Swan value they wrap |
| anything else                               | `Nothing`                                     |

Values that come back to Python, from `context.get`, from document data and
as function arguments, are converted the other way:

| Swan                     | Python                                                  |
|--------------------------|---------------------------------------------------------|
| `Nothing`                | `None`                                                  |
| `Bool`, `Text`           | `bool`, `str`                                           |
| `Numb`                   | `int` if it is a whole number (up to 2<sup>53</sup>), `float` otherwise |
| `List`                   | `list`                                                  |
| `Tuple`                  | `tuple`                                                 |
| `Namespace`              | `dict` of its members, sorted by name                   |
| `Promise`                | [`Promise`](#promise), which you can `await`            |
| `Func`                   | [`Function`](#function), which you can call             |
| `Undefined`              | [`Undefined`](#undefined), with the details of the error |

A tuple with one item is that item, and an empty tuple is `Nothing`, as in
Swan.

### Functions

A Python function becomes a Swan function. A Swan call with several
arguments passes them as separate arguments, and a call with `()` passes
none:

```python
engine = pytexl.Engine({
    "shout": lambda s: s.upper() + "!",
    "add": lambda a, b: a + b,
    "answer": lambda: 42,
})
(await engine.render("doc.texl", '<% shout "hi" %> <% add(2, 3) %> <% answer() %>')).rendering   # 'HI! 5 42'
```

The return value is converted to a Swan value. If the function raises an
exception, the result is `Nothing`. An `async` function, or a function that
returns an awaitable, gives a Swan promise, and the tag waits for it:

```python
async def weather(city):
    await asyncio.sleep(0.1)                   # a network request, in a real program
    return "sunny"

engine = pytexl.Engine({"weather": weather})
(await engine.render("doc.texl", 'Today in Rome: <% weather "rome" %>')).rendering   # 'Today in Rome: sunny'
```

A coroutine object passed as a value is awaited once, and every template sees
its result.

### Swan functions and promises

The functions that a template defines come back to Python as
[`Function`](#function) objects, which you call like Python functions. A
document can be a library:

```python
doc = await engine.load("lib/format.texl")     # defines plural = (count, word) -> ...
doc.data["plural"](3, "apple")                 # '3 apples'
```

Swan promises come back as [`Promise`](#promise) objects, which you `await`:

```python
context = engine.create_context({"weather": weather})
await context.render('<% forecast = weather "rome" %>')
await context.get("forecast")                  # 'sunny'
```

### Undefined values

An invalid operation in Swan does not raise an exception: it produces an
[undefined value](../swan/undefined.md), which records what went wrong. It
comes back to Python as an [`Undefined`](#undefined) object, with the same
details as [`undef.inspect`](../swan/builtins/undef.md#undefinspect):

```python
context = engine.create_context({"price": "12.5"})    # a string, by mistake
await context.render("<% total = price * 4 %>")
total = context.get("total")
total                  # <pytexl.Undefined BinaryOperation op='product' lhv='12.5' rhv=4 pos=15>
total.op, total.lhv    # ('product', '12.5')
bool(total)            # False
```

`None` always means `Nothing`: a name that is not bound, or an empty tuple.
To tell the two apart, test `isinstance(value, pytexl.Undefined)`.

## Contexts

`engine.render` uses a new scope each time, so the names a template binds
are only in the `data` of the document it returns. A *context* keeps them: it
is a scope that you render several templates in, and that you can read and
write from Python.

```python
context = engine.create_context({"price": 12.5})
await context.render("<% total = price * 4 %>")
context.get("total")                           # 50
await context.render("Total: <% total %>")     # 'Total: 50'
```

A context is not a document: its templates cannot import documents, and
`texl.import` is not defined in it. To render a source with imports, use
[`engine.render`](#rendering-a-source).

## Event loops and threads

An engine runs its work on the asyncio event loop of the code that awaits it.
It can outlive the loop: you can call `asyncio.run` several times with the
same engine. Await [`Promise`](#promise) objects in a running loop.

The engine is single-threaded. Use an engine, its contexts, and the
promises and functions that come from them on the thread that created the
engine: the methods of an engine or a context raise `RuntimeError` on another
thread. Each thread can have engines of its own. The loaders read files and
fetch URLs in worker threads, so they do not block the event loop.

## Memory

A function defined in a template holds the scope it was defined in, so a
scope that stores one keeps itself alive: a reference cycle. The cycle
collector, [`pytexl.collect`](#collect), frees the scopes that nothing
outside such cycles refers to any more. `engine.render` and `engine.load` run
it when they are done, so a program that only renders documents has nothing
to do. After dropping a context that defined functions, call `collect`:

```python
del context
pytexl.collect()
```

Python's own garbage collector does not reach Swan scopes, and `collect` does
not reach Python objects. A Swan value held by a Python object (a
[`Function`](#function) in a list, for example) is kept while that object
lives.

## API reference

The docstrings of the package document the same API, in Google style. Run
`help(pytexl)` to read them.

### `Engine`

An engine renders and loads documents. It holds configuration only: its
globals and loaders are shared by everything it renders, and nothing else is
(see [Caching](#caching)).

#### `Engine(globals, base_url)`

```python
Engine(globals: Mapping[str, Any] | None = None,
       base_url: str | os.PathLike[str] | None = None)
```

Creates an engine with the [file and HTTP loaders](#loaders).

| Argument   | Description                                                                 |
|------------|-----------------------------------------------------------------------------|
| `globals`  | A mapping whose items become globals of every template and document. Optional. |
| `base_url` | The URL that `fetch`, `render` and `load` resolve relative URLs against. A path becomes a `file://` URL, with a final slash if it is a folder. Optional; defaults to the current working directory. |

Raises `ValueError` if `base_url` is not an absolute URL.

```python
engine = pytexl.Engine({"site": {"name": "My site"}}, base_url=Path("templates"))
```

#### `engine.base_url`

```python
base_url: str
```

Returns the base URL of the engine, normalized. It is read-only.

```python
pytexl.Engine(base_url="https://Example.com/a/../b/").base_url   # 'https://example.com/b/'
```

#### `engine.set(name, value)`

```python
def set(name: str, value: Any) -> None
```

Binds a global, visible to every template and document of the engine,
from now on. Contexts created earlier do not see it. A global with the same
name is replaced.

| Argument | Description                                             |
|----------|---------------------------------------------------------|
| `name`   | The name of the global.                                 |
| `value`  | Its value, [converted](#globals-and-context-values) to a Swan value. |

```python
engine.set("today", datetime.date.today().isoformat())
```

#### `engine.add_loader(scheme, loader)`

```python
def add_loader(scheme: str, loader: Loader) -> None
```

Registers the [loader](#loader) of the URLs with a scheme, replacing any
previous loader of that scheme, the built-in ones included.

| Argument | Description                                                      |
|----------|------------------------------------------------------------------|
| `scheme` | The URL scheme, without the colon, such as `"mem"`. It is case-insensitive. |
| `loader` | A function that takes the full URL and returns the source, or an awaitable of it. |

```python
engine.add_loader("mem", lambda uri: templates[uri])
```

#### `engine.set_default_loader(loader)`

```python
def set_default_loader(loader: Loader) -> None
```

Registers the [loader](#loader) of the URLs without a scheme, replacing
`loaders.load_file`. The loader receives absolute, percent-encoded paths, such
as `/dir/my%20doc.texl`. Every URI that the engine renders is a full URL, so it
does not currently call this loader.

| Argument | Description                                                      |
|----------|------------------------------------------------------------------|
| `loader` | A function that takes the path and returns the source, or an awaitable of it. |

```python
engine.set_default_loader(lambda path: (root / path.lstrip("/")).read_text())
```

#### `engine.fetch(uri)`

```python
async def fetch(uri: str | os.PathLike[str]) -> str | None
```

Reads the source of the document at `uri`, without rendering it. Nothing is
cached: every call runs the loader again.

| Argument | Description                                                          |
|----------|----------------------------------------------------------------------|
| `uri`    | The URL of the document, resolved against `base_url`. A path is resolved against the current working directory instead. |

Returns the source, or `None` if it cannot be read. Raises `ValueError` if
`uri` cannot be resolved to a URL.

```python
source = await engine.fetch(Path("templates/report.texl"))
```

#### `engine.render(uri, source)`

```python
async def render(uri: str | os.PathLike[str], source: str) -> Document
```

Renders `source` as the document at `uri`, in a new scope, without reading
it. Imports in `source` resolve against the URL of `uri`; an import of that
URL itself gives an [`Undefined`](#undefined) (an
[import cycle](../templates/documents.md#import-cycles)).

| Argument | Description                                                          |
|----------|----------------------------------------------------------------------|
| `uri`    | The URL of the document, resolved like the `uri` of [`fetch`](#enginefetchuri). |
| `source` | The template source.                                                 |

Returns the [document](#document). Raises `ValueError` if `uri` cannot be
resolved to a URL.

```python
doc = await engine.render("draft.texl", "2 + 2 = <% 2 + 2 %>")
doc.rendering                                  # '2 + 2 = 4'
```

#### `engine.load(uri)`

```python
async def load(uri: str | os.PathLike[str]) -> Document | None
```

Reads and renders the document at `uri`: [`fetch`](#enginefetchuri) followed
by [`render`](#enginerenderuri-source).

| Argument | Description                                                          |
|----------|----------------------------------------------------------------------|
| `uri`    | The URL of the document, resolved like the `uri` of [`fetch`](#enginefetchuri). |

Returns the [document](#document), or `None` if it cannot be loaded. Raises
`ValueError` if `uri` cannot be resolved to a URL.

```python
doc = await engine.load("templates/report.texl")
print(doc.rendering if doc else "not found")
```

#### `engine.create_context(vars)`

```python
def create_context(vars: Mapping[str, Any] | None = None) -> Context
```

Creates a [context](#context): a scope that keeps its names across renders.
Templates in a context cannot import documents.

| Argument | Description                                                         |
|----------|---------------------------------------------------------------------|
| `vars`   | A mapping whose items are bound in the context. Optional.           |

```python
context = engine.create_context({"user": "Ada"})
```

### `Context`

A scope of an engine, created with `engine.create_context`. Templates rendered
in a context share its names, and so do `get` and `set`. The context also sees
the globals of its engine. It cannot import documents.

#### `context.get(name)`

```python
def get(name: str) -> Any
```

Returns the value of a name, [converted](#globals-and-context-values) to a
Python value. The name is looked up in the context, then in the engine
globals and the built-ins. A name that is not bound gives `None`.

| Argument | Description            |
|----------|------------------------|
| `name`   | The name to look up.   |

```python
await context.render("<% x = (1, 2) %>")
context.get("x")                               # (1, 2)
```

#### `context.set(name, value)`

```python
def set(name: str, value: Any) -> None
```

Binds a name in the context, replacing any previous value.

| Argument | Description                                             |
|----------|---------------------------------------------------------|
| `name`   | The name to bind.                                       |
| `value`  | Its value, [converted](#globals-and-context-values) to a Swan value. |

```python
context.set("items", ["a", "b"])
```

#### `context.names()`

```python
def names() -> list[str]
```

Returns the names bound in the context, sorted. Engine globals and built-ins
are not included. The list always includes `texl`, the engine's namespace,
which has no `import` in a context.

```python
engine.create_context({"b": 1, "a": 2}).names()   # ['a', 'b', 'texl']
```

#### `context.render(template)`

```python
async def render(template: str) -> str
```

Renders a template in the context, and returns the rendered text. The names
the template binds stay in the context.

| Argument   | Description            |
|------------|------------------------|
| `template` | The template source.   |

```python
await context.render("<% count = 1 %>")
await context.render("Count: <% count %>")     # 'Count: 1'
```

### `Document`

```python
@dataclass(frozen=True)
class Document:
    uri: str
    source: str
    data: dict[str, Any]
    rendering: str
```

A rendered document, as returned by `engine.load` and `engine.render`.

| Attribute   | Description                                                        |
|-------------|--------------------------------------------------------------------|
| `uri`       | The URL of the document.                                           |
| `source`    | The template source.                                               |
| `data`      | The names the document's tags bound, [converted](#globals-and-context-values) to Python values. It also holds `texl`. |
| `rendering` | The rendered text.                                                 |

```python
doc = await engine.load("settings.texl")
print(doc.data["title"], doc.rendering)
```

### `Promise`

```python
class Promise:
    done: bool
    def __await__(self) -> Generator[Any, None, Any]
```

A Swan promise. Awaiting it returns its value, converted to a Python value.
Promises never fail: a promise whose value would be undefined resolves to
`None` instead, as in Swan. The `done` attribute is `True` once the promise has a value. Await a
promise in a running event loop.

```python
await context.render("<% p = promise.create() %>")
p = context.get("p")                           # <pytexl.Promise pending>
await context.render("<% promise.resolve p 42 %>")
await p                                        # 42
```

### `Function`

```python
class Function:
    def __call__(self, *args: Any) -> Any
```

A Swan function. Calling it with no argument passes `Nothing`, with one
argument passes that argument, and with several arguments passes them as a
tuple. The result is converted to a Python value. A call never raises: an
invalid call returns an [`Undefined`](#undefined).

```python
await context.render("<% double = x -> x * 2 %>")
double = context.get("double")
double(21)             # 42
double("a")            # <pytexl.Undefined BinaryOperation op='product' lhv='a' rhv=2 pos=17>
```

### `Undefined`

```python
class Undefined:
    type: str
    op: str | None
    val: Any
    lhv: Any
    rhv: Any
    pos: int | None
    expected: str | None
    token: str | None
    def inspect(self) -> dict[str, Any]
```

A Swan undefined value, the result of an invalid operation. Its attributes
are the members of [`undef.inspect`](../swan/builtins/undef.md#undefinspect):
`type` names the kind of error, and the other attributes depend on it. The
attributes that do not apply are `None`.

| `type`              | Attributes                | Meaning                                    |
|---------------------|---------------------------|--------------------------------------------|
| `"UnaryOperation"`  | `op`, `val`, `pos`        | operation `op` failed on the operand `val` |
| `"BinaryOperation"` | `op`, `lhv`, `rhv`, `pos` | operation `op` failed on `lhv` and `rhv`   |
| `"Termination"`     | `expected`, `pos`         | the closing bracket `expected` is missing  |
| `"Token"`           | `token`, `pos`            | a token is in the wrong place              |
| `"Name"`            | `pos`                     | a name was expected                        |
| `"Value"`           | `val`                     | created by `undef.from`                    |

The operands are [converted](#globals-and-context-values) to Python values,
so an operand that is itself undefined is an `Undefined` too. `pos` is the
position of the error in the source of the tag, in bytes; built-in functions
report 0. `inspect()` returns `type` and the attributes that apply, as a
dict.

An `Undefined` is falsy. Two `Undefined` objects are equal when they have the
same type and attributes, whatever their position. Passed back to Swan, an
`Undefined` is the original undefined value.

```python
await context.render('<% err = text.size 1 %>')
err = context.get("err")
err.type               # 'UnaryOperation'
err.inspect()          # {'type': 'UnaryOperation', 'op': 'text.size', 'val': 1, 'pos': 0}
```

### `collect`

```python
def collect() -> int
```

Frees the Swan scopes and values that only reference cycles keep alive (see
[Memory](#memory)), for every engine of the current thread. Returns the number
of scopes and promises it cleared.

```python
pytexl.collect()       # 0 when there is nothing to free
```

### `Loader`

```python
Loader = Callable[[str], str | Awaitable[str]]
```

The type of a document loader: a function that takes a URL and returns the
source of the document, or an awaitable of it. It raises an exception if the
document cannot be read.

### `loaders`

The module `pytexl.loaders` holds the loaders that every engine starts with.
You can register them again, for example after replacing one.

#### `loaders.load_file(uri)`

```python
async def load_file(uri: str) -> str
```

Reads a document from the local file system, as UTF-8, in a worker thread.
It serves `file://` URLs and paths without a scheme, which are absolute paths.
Raises `ValueError` if `uri` is not a local file URL or path, and `OSError`
if the file cannot be read.

```python
await loaders.load_file("file:///home/ada/report.texl")
```

#### `loaders.load_http(uri, timeout)`

```python
async def load_http(uri: str, *, timeout: float = 30.0) -> str
```

Fetches a document over HTTP or HTTPS, in a worker thread, with
`urllib.request`. Redirects are followed, and the body is decoded with the
charset of the response, UTF-8 by default. Raises `urllib.error.URLError` if
the request fails or the response has an error status, and `ValueError` if
`uri` is not an HTTP or HTTPS URL.

| Argument  | Description                                       |
|-----------|---------------------------------------------------|
| `uri`     | An `http://` or `https://` URL.                   |
| `timeout` | The timeout of the request, in seconds.           |

```python
engine.add_loader("https", functools.partial(loaders.load_http, timeout=5))
```

#### `loaders.file_path(uri)`

```python
def file_path(uri: str) -> pathlib.Path
```

Returns the local path of a `file://` URL or of an absolute path, with
percent-encoded characters decoded. The query and the fragment are ignored.
Raises `ValueError` if `uri` has another scheme or names a file on another
host.

```python
loaders.file_path("file:///my%20dir/doc.texl?v=1")   # PosixPath('/my dir/doc.texl')
```
