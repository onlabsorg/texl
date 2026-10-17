"""Tests of pytexl.loaders: the file and HTTP loaders."""

import os
import socket
import tempfile
import unittest
import urllib.error
from pathlib import Path

from pytexl import loaders
from support import FIXTURES, FileServer


class FilePathTest(unittest.TestCase):
    def test_file_urls_and_plain_paths(self):
        self.assertEqual(loaders.file_path("file:///a/b.texl"), Path("/a/b.texl"))
        self.assertEqual(loaders.file_path("file://localhost/a/b.texl"), Path("/a/b.texl"))
        self.assertEqual(loaders.file_path("/a/b.texl"), Path("/a/b.texl"))

    def test_percent_encoding_is_decoded_and_the_query_dropped(self):
        self.assertEqual(loaders.file_path("file:///my%20dir/b.texl"), Path("/my dir/b.texl"))
        self.assertEqual(loaders.file_path("/my%20dir/b.texl?v=1#top"), Path("/my dir/b.texl"))

    def test_other_schemes_and_hosts_are_rejected(self):
        for uri in ("http://example.com/b.texl", "file://example.com/b.texl"):
            with self.subTest(uri=uri), self.assertRaises(ValueError):
                loaders.file_path(uri)


class LoadFileTest(unittest.IsolatedAsyncioTestCase):
    async def test_reads_files_by_url_and_path(self):
        path = FIXTURES / "lib" / "format.texl"
        expected = path.read_text(encoding="utf-8")
        self.assertEqual(await loaders.load_file(path.as_uri()), expected)
        self.assertEqual(await loaders.load_file(path.as_uri().removeprefix("file://")), expected)

    async def test_reads_utf8(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "ünï cödé.texl"
            path.write_bytes("Grüße, 世界".encode("utf-8"))
            self.assertEqual(await loaders.load_file(path.as_uri()), "Grüße, 世界")

    async def test_missing_files_raise(self):
        with self.assertRaises(OSError):
            await loaders.load_file((FIXTURES / "missing.texl").as_uri())

    @unittest.skipIf(os.name == "nt", "directories cannot be opened as files on Windows either way")
    async def test_directories_raise(self):
        with self.assertRaises(OSError):
            await loaders.load_file(FIXTURES.as_uri())


class LoadHttpTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.server = FileServer()
        self.addCleanup(self.server.close)

    async def test_fetches_documents(self):
        expected = (FIXTURES / "partials" / "counted.texl").read_text(encoding="utf-8")
        self.assertEqual(await loaders.load_http(self.server.origin + "/partials/counted.texl"), expected)

    async def test_error_statuses_raise(self):
        with self.assertRaises(urllib.error.HTTPError) as raised:
            await loaders.load_http(self.server.origin + "/missing.texl")
        self.assertEqual(raised.exception.code, 404)

    async def test_unreachable_servers_raise(self):
        with socket.socket() as unused:
            unused.bind(("127.0.0.1", 0))
            port = unused.getsockname()[1]
        with self.assertRaises(urllib.error.URLError):
            await loaders.load_http(f"http://127.0.0.1:{port}/doc.texl", timeout=5)

    async def test_other_schemes_raise(self):
        with self.assertRaises(ValueError):
            await loaders.load_http("file:///etc/hosts")


if __name__ == "__main__":
    unittest.main()
