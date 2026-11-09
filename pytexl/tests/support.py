"""Helpers shared by the tests: the fixtures folder and a local HTTP server."""

from __future__ import annotations

import functools
import http.server
import threading
from pathlib import Path

#: The folder of the test documents.
FIXTURES = Path(__file__).resolve().parent / "fixtures"


class _QuietHandler(http.server.SimpleHTTPRequestHandler):
    """Serves files without logging every request to stderr. ``/redirect/<path>``
    redirects to ``/<path>``."""

    def do_GET(self) -> None:
        if self.path.startswith("/redirect/"):
            self.send_response(302)
            self.send_header("Location", self.path.removeprefix("/redirect"))
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        super().do_GET()

    def log_message(self, format: str, *args: object) -> None:
        pass


class FileServer:
    """Serves a folder over HTTP on a free local port, on a background thread.

    Use it as a context manager, or call :meth:`close`.

    Args:
        folder: The folder to serve.

    Attributes:
        origin: The origin of the server, such as ``http://127.0.0.1:41234``.
    """

    def __init__(self, folder: Path = FIXTURES) -> None:
        handler = functools.partial(_QuietHandler, directory=str(folder))
        self._server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        host, port = self._server.server_address[:2]
        self.origin = f"http://{host}:{port}"
        self._thread = threading.Thread(target=self._server.serve_forever, args=(0.05,), daemon=True)
        self._thread.start()

    def close(self) -> None:
        """Stops the server."""
        self._server.shutdown()
        self._server.server_close()

    def __enter__(self) -> FileServer:
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()
