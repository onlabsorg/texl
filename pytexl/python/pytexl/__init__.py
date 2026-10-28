"""Text templates with Swan expressions: the Python library of Texl.

A Texl template is text with ``<% ... %>`` tags. Each tag holds an expression
in Swan, and rendering replaces the tag with the value of its expression.

Example:
    >>> import asyncio, pytexl
    >>> engine = pytexl.Engine({"user": {"name": "Ada"}})
    >>> asyncio.run(engine.render("hello.texl", "Hello <% user.name %>!")).rendering
    'Hello Ada!'

The engine is asynchronous: rendering waits for the promises that tags return,
such as imported documents and the results of ``async`` functions. Use it from
asyncio code, or run it with :func:`asyncio.run`.

Values passed to templates are converted to Swan values, and values that come
back from templates are converted to Python values:

===============================  ===========================================
Python                           Swan
===============================  ===========================================
``None``                         ``Nothing``
``bool``                         ``Bool``
``int``, ``float``               ``Numb``
``str``                          ``Text``
``list``                         ``List``
``tuple``                        ``Tuple``
mapping (``dict``, ...)          ``Namespace`` of its string keys
awaitable (coroutine, future)    ``Promise``
callable                         ``Func``
===============================  ===========================================

Swan values come back as ``None`` (``Nothing``), ``bool``, ``int`` (integral
numbers) or ``float``, ``str``, ``list``, ``tuple``, ``dict`` (namespaces),
:class:`Promise`, :class:`Function` and :class:`Undefined`. An
:class:`Undefined` keeps the details of the error that produced it.

See the Python guide: https://github.com/onlabsorg/texl/blob/main/docs/api/python.md
"""

from __future__ import annotations

import os
import threading
from collections.abc import Awaitable, Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Union

from . import loaders
from . import _pytexl
from ._pytexl import Function, NativeContext, NativeEngine, Promise, Undefined

__all__ = ["Context", "Document", "Engine", "Function", "Loader", "Promise", "Undefined", "collect", "loaders"]

#: A document loader: takes a URI and returns the source, or an awaitable of it.
Loader = Callable[[str], Union[str, Awaitable[str]]]


@dataclass(frozen=True)
class Document:
    """A rendered document, as returned by :meth:`Engine.load` and
    :meth:`Engine.render`.

    Attributes:
        uri: The URL of the document.
        source: The template source of the document.
        data: The names that the document's tags bound, converted to Python
            values. Functions defined by the document are :class:`Function`
            objects that you can call.
        rendering: The rendered text.
    """

    uri: str
    source: str
    data: dict[str, Any]
    rendering: str


class _ThreadBound:
    """Base of the classes that wrap a native object, which only works on the
    thread that created it."""

    def __init__(self, native: Any) -> None:
        self.__native = native
        self.__thread = threading.get_ident()

    @property
    def _native(self) -> Any:
        """The native object.

        Raises:
            RuntimeError: If the current thread is not the one that created it.
        """
        if threading.get_ident() != self.__thread:
            raise RuntimeError(f"a pytexl {type(self).__name__} can only be used on the thread that created it")
        return self.__native


class Engine(_ThreadBound):
    """Renders documents and loads them from files and URLs.

    An engine starts with three loaders (see :mod:`pytexl.loaders`): local
    files for ``file://`` URLs and for paths without a scheme, and HTTP for
    ``http://`` and ``https://`` URLs. Add others with :meth:`add_loader`.

    Every document has a URL: the URIs given to :meth:`fetch`, :meth:`render`
    and :meth:`load` resolve against :attr:`base_url`. Imports resolve against
    the importing document: relative paths against its folder, absolute paths
    (``/dir/doc.texl``) against its root.

    The engine holds configuration only: globals and loaders. Every
    :meth:`render` and :meth:`load` starts from a new scope and a new import
    cache: within one rendering, each imported document is fetched and
    rendered once; the next rendering fetches it again. An imported document
    renders in its own scope: it sees the globals, but not the names of its
    importer. To cache documents across renders, wrap the loaders.

    An engine, its contexts and the promises and functions that come from
    them can only be used on the thread that created the engine. The methods
    of an engine or a context raise :class:`RuntimeError` on another thread.

    Args:
        globals: Values bound as globals in every template and document,
            Optional.
        base_url: The URL that relative URIs resolve against. A
            path (:class:`os.PathLike`) is converted to a ``file://`` URL; a
            directory gets a final slash, so that URIs resolve inside it.
            Defaults to the current working directory.

    Raises:
        ValueError: If ``base_url`` is not an absolute URL.

    Example:
        >>> engine = pytexl.Engine({"site": {"name": "My site"}}, base_url=Path("templates"))
        >>> doc = await engine.load("index.texl")
    """

    def __init__(
        self,
        globals: Mapping[str, Any] | None = None,
        base_url: str | os.PathLike[str] | None = None,
    ) -> None:
        super().__init__(NativeEngine(_base_url(base_url)))
        self._native.add_loader("file", loaders.load_file)
        self._native.set_default_loader(loaders.load_file)
        self._native.add_loader("http", loaders.load_http)
        self._native.add_loader("https", loaders.load_http)
        for name, value in (globals or {}).items():
            self.set(name, value)

    @property
    def base_url(self) -> str:
        """str: The URL that relative URIs resolve against, normalized."""
        return self._native.base_url

    def set(self, name: str, value: Any) -> None:
        """Binds a global, visible to every template and document of the engine.

        Contexts created earlier and renderings in progress do not see it. A
        global with the same name is replaced.

        Args:
            name: The name of the global.
            value: Its value, converted to a Swan value. An awaitable becomes a
                promise, which a tag waits for.

        Example:
            >>> engine.set("today", datetime.date.today().isoformat())
        """
        self._native.set(name, value)

    def add_loader(self, scheme: str, loader: Loader) -> None:
        """Registers the loader of the URIs with a given scheme.

        The loader replaces any previous loader of the scheme, including the
        built-in ones.

        Args:
            scheme: The URI scheme, without the colon, such as ``"mem"``. It is
                case-insensitive.
            loader: A function that takes the full URI and returns the source
                of the document, or an awaitable of it. If it raises, or its
                result is not a string, the document cannot be loaded.

        Example:
            >>> templates = {"mem://greeting": "Hello <% reader %>!"}
            >>> engine.add_loader("mem", lambda uri: templates[uri])
        """
        self._native.add_loader(scheme, loader)

    def set_default_loader(self, loader: Loader) -> None:
        """Registers the loader of the URIs without a scheme.

        It replaces :func:`pytexl.loaders.load_file`. The engine normalizes the
        paths before calling the loader: they are absolute and percent-encoded,
        such as ``/dir/my%20doc.texl``.

        Note:
            The URIs given to the engine resolve against :attr:`base_url` to
            full URLs, and so do the imports in their documents, so the
            engine does not currently produce paths without a scheme.

        Args:
            loader: A function that takes the path and returns the source of the
                document, or an awaitable of it, like the loaders of
                :meth:`add_loader`.

        Example:
            >>> root = Path("site")
            >>> engine.set_default_loader(lambda path: (root / path.lstrip("/")).read_text())
        """
        self._native.set_default_loader(loader)

    async def fetch(self, uri: str | os.PathLike[str]) -> str | None:
        """Returns the source of a document, without rendering it.

        Nothing is cached: every call runs the loader again.

        Args:
            uri: The URL of the document, resolved against :attr:`base_url`. A
                path (:class:`os.PathLike`) is resolved against the current
                working directory and converted to a ``file://`` URL.

        Returns:
            The source, or ``None`` if it cannot be loaded.

        Raises:
            ValueError: If ``uri`` cannot be resolved to a URL.

        Example:
            >>> source = await engine.fetch("templates/report.texl")
        """
        return await self._native.fetch(_uri(uri))

    async def render(self, uri: str | os.PathLike[str], source: str) -> Document:
        """Renders a source as the document at a URL, without loading it.

        The source is rendered in a new scope, and the imports inside it
        resolve against the URL of ``uri``. An import of that URL itself gives
        an :class:`Undefined` (a cycle). A source with no real location, such
        as standard input or an unsaved buffer, can use any URL, for example a
        file in the folder of the documents it imports.

        Args:
            uri: The URL of the document, resolved like the URI of
                :meth:`fetch`.
            source: The template source.

        Returns:
            The document.

        Raises:
            ValueError: If ``uri`` cannot be resolved to a URL.

        Example:
            >>> doc = await engine.render(Path("templates/draft.texl"), "2 + 2 = <% 2 + 2 %>")
            >>> doc.rendering
            '2 + 2 = 4'
        """
        return _document(await self._native.render(_uri(uri), source))

    async def load(self, uri: str | os.PathLike[str]) -> Document | None:
        """Loads and renders a document: :meth:`fetch` followed by :meth:`render`.

        Args:
            uri: The URL of the document, resolved like the URI of
                :meth:`fetch`.

        Returns:
            The document, or ``None`` if it cannot be loaded.

        Raises:
            ValueError: If ``uri`` cannot be resolved to a URL.

        Example:
            >>> doc = await engine.load("templates/report.texl")
            >>> print(doc.rendering if doc else "not found")
        """
        doc = await self._native.load(_uri(uri))
        return None if doc is None else _document(doc)

    def create_context(self, vars: Mapping[str, Any] | None = None) -> Context:
        """Creates a context: a scope that keeps its names across renders.

        A context is not a document: its templates cannot import documents
        (``texl.import`` is not defined in it). Use :meth:`render` or
        :meth:`load` to render documents that import others.

        Args:
            vars: Values bound in the context. Optional.

        Returns:
            A new context, which also sees the globals of the engine.

        Example:
            >>> context = engine.create_context({"user": "Ada"})
            >>> await context.render("Hello <% user %>!")
            'Hello Ada!'
        """
        return Context(self._native.create_context(vars))


class Context(_ThreadBound):
    """A scope of an :class:`Engine` whose names persist across renders.

    Templates rendered in a context share its names, and so do :meth:`get`
    and :meth:`set`. A context also sees the globals of its engine. Create one
    with :meth:`Engine.create_context`.

    Example:
        >>> context = engine.create_context({"price": 12.5})
        >>> await context.render("<% total = price * 4 %>")
        ''
        >>> context.get("total")
        50
    """

    def __init__(self, native: NativeContext) -> None:
        super().__init__(native)

    def get(self, name: str) -> Any:
        """Returns the value of a name, converted to a Python value.

        The name is looked up in the context, then in the engine globals and
        the built-ins.

        Args:
            name: The name to look up.

        Returns:
            The value, or ``None`` if the name is not bound.

        Example:
            >>> await context.render("<% x = (1, 2) %>")
            ''
            >>> context.get("x")
            (1, 2)
        """
        return self._native.get(name)

    def set(self, name: str, value: Any) -> None:
        """Binds a name in the context, replacing any previous value.

        Args:
            name: The name to bind.
            value: Its value, converted to a Swan value.

        Example:
            >>> context.set("items", ["a", "b"])
        """
        self._native.set(name, value)

    def names(self) -> list[str]:
        """Returns the names bound in the context, sorted.

        Engine globals and built-ins are not included. The list always
        includes ``texl``, the namespace of the engine's helpers (in a
        context, it has no ``import``).

        Returns:
            The sorted names.

        Example:
            >>> engine.create_context({"b": 1, "a": 2}).names()
            ['a', 'b', 'texl']
        """
        return self._native.names()

    async def render(self, template: str) -> str:
        """Renders a template in the context.

        The names that the template binds stay in the context.

        Args:
            template: The template source.

        Returns:
            The rendered text.

        Example:
            >>> await context.render("<% count = 1 %>")
            ''
            >>> await context.render("Count: <% count %>")
            'Count: 1'
        """
        return await self._native.render(template)


def collect() -> int:
    """Frees the Swan scopes and values that only reference cycles keep alive.

    A function defined in a template holds the scope it was defined in, so a
    scope that stores one keeps itself alive. ``collect`` frees the scopes that
    nothing outside such cycles refers to any more, for every engine of the
    current thread. :meth:`Engine.render` and :meth:`Engine.load` run it when
    they are done; call it after dropping a :class:`Context` that defined
    functions.

    Returns:
        The number of Swan scopes and promises it cleared.

    Example:
        >>> pytexl.collect()
        0
    """
    return _pytexl.collect()


def _uri(uri: str | os.PathLike[str]) -> str:
    """Returns the URI of a document as a string: a path becomes a ``file://`` URL."""
    if isinstance(uri, os.PathLike):
        return Path(uri).resolve().as_uri()
    return uri


def _document(doc: dict[str, Any]) -> Document:
    """Converts a document namespace into a :class:`Document`."""
    return Document(doc["uri"], doc["source"], doc["data"], doc["rendering"])


def _base_url(base_url: str | os.PathLike[str] | None) -> str:
    """Returns the base URL of an engine as a string."""
    if base_url is None:
        base_url = Path.cwd()
    if isinstance(base_url, os.PathLike):
        path = Path(base_url).resolve()
        uri = path.as_uri()
        return uri + "/" if path.is_dir() and not uri.endswith("/") else uri
    return base_url
