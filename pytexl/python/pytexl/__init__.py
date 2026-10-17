"""Text templates with Swan expressions: the Python library of Texl.

A Texl template is text with ``<% ... %>`` tags. Each tag holds an expression
in Swan, and rendering replaces the tag with the value of its expression.

Example:
    >>> import asyncio, pytexl
    >>> engine = pytexl.Engine({"user": {"name": "Ada"}})
    >>> asyncio.run(engine.render("Hello <% user.name %>!"))
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

See the Python guide: https://onlabsorg.github.io/texl/book/hosts/python.html
"""

from __future__ import annotations

import os
import threading
from collections.abc import Awaitable, Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Union

from . import loaders
from ._pytexl import Function, NativeContext, NativeEngine, Promise, Undefined

__all__ = ["Context", "Document", "Engine", "Function", "Loader", "Promise", "Undefined", "loaders"]

#: A document loader: takes a URI and returns the source, or an awaitable of it.
Loader = Callable[[str], Union[str, Awaitable[str]]]


@dataclass(frozen=True)
class Document:
    """A loaded document, as returned by :meth:`Engine.load`.

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
    """Renders templates and loads documents from files and URLs.

    An engine starts with three loaders (see :mod:`pytexl.loaders`): local
    files for ``file://`` URLs and for paths without a scheme, and HTTP for
    ``http://`` and ``https://`` URLs. Add others with :meth:`add_loader`.

    Imports resolve against the importing document: relative paths against
    its folder, absolute paths (``/dir/doc.texl``) against its root. Templates
    without a document (:meth:`render`, contexts) resolve imports against the
    root path ``/``, which the default loader reads from the local file system.

    Each document is loaded and rendered at most once per engine: later loads
    and imports of the same URL share the first result.

    The globals, documents and engine-wide names (bound by templates with
    ``bind``) are shared by everything the engine renders.

    An engine, its contexts and the promises and functions that come from
    them can only be used on the thread that created the engine. The methods
    of an engine or a context raise :class:`RuntimeError` on another thread.

    Args:
        globals: Values bound as globals in every template and document,
            including contexts created earlier. Optional.
        base_url: The URL that :meth:`load` resolves relative URIs against. A
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

        The global is also visible in the contexts created earlier. A global
        with the same name is replaced.

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

        Args:
            loader: A function that takes the path and returns the source of the
                document, or an awaitable of it, like the loaders of
                :meth:`add_loader`.

        Example:
            >>> root = Path("site")
            >>> engine.set_default_loader(lambda path: (root / path.lstrip("/")).read_text())
        """
        self._native.set_default_loader(loader)

    async def load(self, uri: str | os.PathLike[str]) -> Document | None:
        """Loads and renders a document.

        The source is rendered in a fresh scope, and the imports inside it
        resolve against the URL of the document. The result is cached, so later
        loads and imports of the same URL return it without loading again.

        Args:
            uri: The URL of the document, resolved against :attr:`base_url`. A
                path (:class:`os.PathLike`) is resolved against the current
                working directory and converted to a ``file://`` URL.

        Returns:
            The document, or ``None`` if it cannot be loaded.

        Raises:
            ValueError: If ``uri`` cannot be resolved to a URL.

        Example:
            >>> doc = await engine.load("templates/report.texl")
            >>> print(doc.rendering if doc else "not found")
        """
        if isinstance(uri, os.PathLike):
            uri = Path(uri).resolve().as_uri()
        doc = await self._native.load(uri)
        return None if doc is None else Document(doc["uri"], doc["source"], doc["data"], doc["rendering"])

    async def render(self, template: str) -> str:
        """Renders a template in a fresh scope.

        The names that the template binds with ``=`` or ``:`` are discarded
        afterwards. Use a :class:`Context` to keep them.

        Args:
            template: The template source.

        Returns:
            The rendered text.

        Example:
            >>> await engine.render("2 + 2 = <% 2 + 2 %>")
            '2 + 2 = 4'
        """
        return await self._native.render(template)

    def create_context(self, vars: Mapping[str, Any] | None = None) -> Context:
        """Creates a context: a scope that keeps its names across renders.

        Args:
            vars: Values bound in the context. Optional.

        Returns:
            A new context, which also sees the globals of the engine.

        Example:
            >>> context = engine.create_context({"user": "Ada"})
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
        includes ``texl``, the namespace that holds ``texl.import``.

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


def _base_url(base_url: str | os.PathLike[str] | None) -> str:
    """Returns the base URL of an engine as a string."""
    if base_url is None:
        base_url = Path.cwd()
    if isinstance(base_url, os.PathLike):
        path = Path(base_url).resolve()
        uri = path.as_uri()
        return uri + "/" if path.is_dir() and not uri.endswith("/") else uri
    return base_url
