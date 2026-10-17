"""Document loaders: the functions that read the source of a document.

A loader is a callable that takes the URI of a document and returns its source
as a string, or an awaitable of it. It raises an exception if the document
cannot be read, and the engine then treats the document as missing.

Every :class:`pytexl.Engine` starts with the loaders of this module:

- :func:`load_file` for ``file://`` URLs, and as the default loader, for
  paths without a scheme (absolute paths of the local file system);
- :func:`load_http` for ``http://`` and ``https://`` URLs.

Example:
    A loader of your own, for documents kept in memory::

        templates = {"mem://greeting": "Hello <% reader %>!"}
        engine.add_loader("mem", lambda uri: templates[uri])
"""

from __future__ import annotations

import asyncio
import urllib.parse
import urllib.request
from pathlib import Path

__all__ = ["file_path", "load_file", "load_http"]

#: The default timeout of :func:`load_http`, in seconds.
DEFAULT_TIMEOUT = 30.0


def file_path(uri: str) -> Path:
    """Returns the local path of a ``file://`` URL or of an absolute path.

    The engine hands paths without a scheme to the default loader already
    normalized: absolute and percent-encoded, like the path of a ``file://``
    URL. The query and the fragment are ignored.

    Args:
        uri: A ``file://`` URL, or a path such as ``/dir/doc.texl``.

    Returns:
        The local path, with percent-encoded characters decoded.

    Raises:
        ValueError: If ``uri`` has a scheme other than ``file``, or names a
            file on another host.

    Example:
        >>> file_path("file:///my%20dir/doc.texl?v=1")
        PosixPath('/my dir/doc.texl')
    """
    parts = urllib.parse.urlsplit(uri)
    if parts.scheme not in ("", "file"):
        raise ValueError(f"{uri}: not a file URL")
    if parts.netloc not in ("", "localhost"):
        raise ValueError(f"{uri}: not a local file")
    return Path(urllib.request.url2pathname(parts.path))


async def load_file(uri: str) -> str:
    """Reads a document from the local file system.

    It serves ``file://`` URLs and paths without a scheme, which are absolute
    paths of the local file system. The file is read as UTF-8 in a worker
    thread, so it does not block the event loop.

    Args:
        uri: A ``file://`` URL, or an absolute path such as ``/dir/doc.texl``.

    Returns:
        The content of the file.

    Raises:
        ValueError: If ``uri`` is not a local file URL or path.
        OSError: If the file cannot be read.

    Example:
        >>> await load_file("file:///home/ada/docs/report.texl")
        'Report of <% date %>...'
    """
    path = file_path(uri)
    return await asyncio.to_thread(path.read_text, encoding="utf-8")


async def load_http(uri: str, *, timeout: float = DEFAULT_TIMEOUT) -> str:
    """Fetches a document over HTTP or HTTPS.

    The request runs in a worker thread with :mod:`urllib.request`, so it does
    not block the event loop. Redirects are followed. The body is decoded with
    the charset of the response, UTF-8 by default.

    Args:
        uri: An ``http://`` or ``https://`` URL.
        timeout: The timeout of the request, in seconds.

    Returns:
        The body of the response.

    Raises:
        urllib.error.URLError: If the request fails, or the response has an
            error status (``urllib.error.HTTPError``).
        ValueError: If ``uri`` is not an HTTP or HTTPS URL.

    Example:
        >>> await load_http("https://example.com/templates/header.texl")
        '<header><% site.name %></header>'
    """
    if urllib.parse.urlsplit(uri).scheme not in ("http", "https"):
        raise ValueError(f"{uri}: not an HTTP or HTTPS URL")
    return await asyncio.to_thread(_fetch, uri, timeout)


def _fetch(uri: str, timeout: float) -> str:
    """Fetches ``uri`` and returns the body of the response, decoded."""
    with urllib.request.urlopen(uri, timeout=timeout) as response:
        charset = response.headers.get_content_charset() or "utf-8"
        return response.read().decode(charset)
