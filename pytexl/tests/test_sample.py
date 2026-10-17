"""Renders fixtures/sample.texl and compares it with fixtures/sample.expected.txt."""

import asyncio
import unittest

import pytexl
from support import FIXTURES, FileServer


async def slow_double(x):
    await asyncio.sleep(0.02)
    return 2 * x


class SampleTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.server = FileServer()
        self.addCleanup(self.server.close)

    def create_engine(self):
        later = asyncio.get_running_loop().create_future()
        asyncio.get_running_loop().call_later(0.01, later.set_result, 42)
        return pytexl.Engine(
            {
                "origin": self.server.origin,
                "fixtures": FIXTURES.as_uri().removeprefix("file://"),
                "site": {"name": "Texl in Python", "tags": ["swan", "texl", "python"], "version": (1, 2)},
                "shout": lambda s: s.upper() + "!",
                "add": lambda a, b: a + b,
                "slow_double": slow_double,
                "later": later,
            }
        )

    async def test_sample_renders_as_expected(self):
        doc = await self.create_engine().load(FIXTURES / "sample.texl")
        expected = (FIXTURES / "sample.expected.txt").read_text(encoding="utf-8")
        self.maxDiff = None
        self.assertIsNotNone(doc)
        self.assertEqual(doc.rendering, expected)

    async def test_sample_document_fields(self):
        doc = await self.create_engine().load(FIXTURES / "sample.texl")
        self.assertEqual(doc.uri, (FIXTURES / "sample.texl").as_uri())
        self.assertEqual(doc.source, (FIXTURES / "sample.texl").read_text(encoding="utf-8"))
        self.assertEqual(doc.data["title"], "Texl Sample Document")
        self.assertEqual(doc.data["nums"], [10, 20, 30])
        self.assertEqual(doc.data["point"], {"x": 3, "y": 4})
        self.assertEqual(doc.data["fact"](5), 120)

    async def test_sample_over_http(self):
        # The same document, fetched over HTTP: its relative imports are fetched too
        doc = await self.create_engine().load(self.server.origin + "/sample.texl")
        self.assertIsNotNone(doc)
        self.assertEqual(doc.uri, self.server.origin + "/sample.texl")
        self.assertIn("Dear Ada, this paragraph comes from partials/greeting.texl.", doc.rendering)
        self.assertIn("fmt.plural(3, \"apple\")         -> 3 apples", doc.rendering)


if __name__ == "__main__":
    unittest.main()
