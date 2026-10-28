# Custom loaders

A *loader* decides where documents come from. This example, in Python, adds
three: a loader for a scheme of your own, a cache in front of any loader, and
a file loader that serves one folder only. The same ideas apply in Rust with
`Engine::add_loader` and `loader_fn` (see the [Rust guide](../api/rust.md#loading-documents)).

## A scheme of your own

A loader takes the full URL of a document and returns its source, or an
awaitable of it. To fail, it raises an exception.

```python
templates = {
    "mem://docs/greeting.texl": 'Hello <% reader %>! <% texl.import("footer.texl") ~> (doc -> doc.rendering) %>',
    "mem://docs/footer.texl": "Bye.",
}

def load_memory(uri):
    return templates[uri]          # a KeyError means: not found
```

Use URLs with a host and a path (`mem://docs/greeting.texl`), like web URLs:
relative imports, such as `"footer.texl"` above, resolve against the path. In
`mem://greeting` the host is `greeting` and the path is empty, so a relative
import would not land where you expect.

## A cache

The engine has no document cache: every rendering loads its imports again,
so it always sees their current content (see
[Swan and Texl in depth](../language.md#one-cache-per-rendering-decision-d17)).
A program that knows when its documents change can cache them itself, by
wrapping its loaders:

```python
class Cached:
    def __init__(self, loader):
        self.loader = loader
        self.sources = {}
        self.calls = 0

    async def __call__(self, uri):
        if uri not in self.sources:
            self.calls += 1
            source = self.loader(uri)
            if asyncio.iscoroutine(source):
                source = await source
            self.sources[uri] = source
        return self.sources[uri]

    def forget(self, uri):
        self.sources.pop(uri, None)
```

Register it like any loader:

```python
engine = pytexl.Engine({"reader": "Ada"})
memory = Cached(load_memory)
engine.add_loader("mem", memory)
for _ in range(3):
    doc = await engine.load("mem://docs/greeting.texl")
print(doc.rendering)       # Hello Ada! Bye.
print(memory.calls)        # 2
```

Three renderings, two documents, two loads. Call `memory.forget(uri)` when a
document changes. A failed load is not cached: the exception passes through.

## A file loader for one folder

The built-in file loader reads any file the process can read, and an imported
document's `source` is part of the import result. A program that renders
templates it does not trust replaces the loaders, so that templates reach
only what they should:

```python
from pathlib import Path
from pytexl import loaders

def inside(root, loader):
    root = Path(root).resolve()
    async def load(uri):
        path = loaders.file_path(uri).resolve()
        if not path.is_relative_to(root):
            raise PermissionError(f"{uri} is outside {root}")
        return await loader(uri)
    return load

def refuse(uri):
    raise PermissionError(uri)

safe = pytexl.Engine({"reader": "Ada"}, base_url=Path("site"))
safe.add_loader("file", inside("site", loaders.load_file))
safe.set_default_loader(refuse)
safe.add_loader("http", refuse)
safe.add_loader("https", refuse)
```

With `site/parts/welcome.texl` containing `Welcome, <% reader %>!` and a line
break, and a file `secret.txt` next to `site`:

```python
source = '<% texl.import("parts/welcome.texl") ~> (d -> d.rendering) %><% texl.import("../secret.txt") ~> (d -> d.source ; "(refused)") %>'
doc = await safe.render("page.texl", source)
print(doc.rendering)
```

```text
Welcome, Ada!
(refused)
```

The refused import resolves to `Nothing`, like any failed load, and the `;`
gives the fallback. What else a template can reach is described in
[Determinism and sandboxing](../language.md#determinism-and-sandboxing).
