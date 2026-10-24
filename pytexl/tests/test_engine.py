"""Tests of pytexl.Engine: globals, rendering, documents, loaders and imports."""

import asyncio
import os
import tempfile
import threading
import unittest
from pathlib import Path

import pytexl
from support import FIXTURES, FileServer

RENDERING = "~> (doc -> doc.rendering)"


def memory_engine(docs, scheme="mem"):
    """Returns an engine whose `scheme` and default loaders serve `docs` (keyed by URI),
    and the list of the URIs they were asked for."""
    requested = []

    def loader(uri):
        requested.append(uri)
        return docs[uri]

    engine = pytexl.Engine()
    engine.add_loader(scheme, loader)
    engine.set_default_loader(loader)
    return engine, requested


class BaseUrlTest(unittest.TestCase):
    def test_base_url_defaults_to_the_working_directory(self):
        self.assertEqual(pytexl.Engine().base_url, Path.cwd().as_uri() + "/")

    def test_explicit_base_url_is_normalized(self):
        engine = pytexl.Engine(base_url="HTTP://Example.com/a/../b/doc.texl")
        self.assertEqual(engine.base_url, "http://example.com/b/doc.texl")

    def test_path_base_url_becomes_a_file_url(self):
        self.assertEqual(pytexl.Engine(base_url=FIXTURES).base_url, FIXTURES.as_uri() + "/")
        sample = FIXTURES / "sample.texl"
        self.assertEqual(pytexl.Engine(base_url=sample).base_url, sample.as_uri())

    def test_root_path_base_url(self):
        # On Windows the root path is the root of the current drive, such as file:///C:/
        self.assertEqual(pytexl.Engine(base_url=Path("/")).base_url, Path("/").resolve().as_uri())

    def test_invalid_base_url_raises(self):
        with self.assertRaises(ValueError):
            pytexl.Engine(base_url="not a url")


class RenderTest(unittest.IsolatedAsyncioTestCase):
    async def test_render(self):
        self.assertEqual(await pytexl.Engine().render("Hello <% 10 + 20 %>!"), "Hello 30!")

    async def test_render_uses_the_engine_globals(self):
        engine = pytexl.Engine({"a": 10, "name": "Ada"})
        self.assertEqual(await engine.render("<% name %>: a = <% a %>"), "Ada: a = 10")

    async def test_set_adds_and_replaces_globals(self):
        engine = pytexl.Engine({"a": 10})
        engine.set("b", 32)
        self.assertEqual(await engine.render("<% a + b %>"), "42")
        engine.set("a", 0)
        self.assertEqual(await engine.render("<% a + b %>"), "32")

    async def test_globals_reach_contexts_created_earlier(self):
        engine = pytexl.Engine()
        context = engine.create_context()
        engine.set("late", "here")
        self.assertEqual(await context.render("<% late %>"), "here")

    async def test_render_starts_from_a_fresh_scope(self):
        engine = pytexl.Engine()
        first = await engine.render("<% x = 1 %><% x %>|")
        second = await engine.render("[<% x %>]")
        self.assertEqual(first + second, "1|[]")

    async def test_bind_shares_names_across_the_engine(self):
        engine = pytexl.Engine()
        await engine.render('<% bind "shared" 7 %>')
        self.assertEqual(await engine.render("<% shared %>"), "7")

    async def test_render_without_tags_copies_the_text(self):
        self.assertEqual(await pytexl.Engine().render("plain text"), "plain text")

    async def test_render_reports_invalid_expressions_as_undefined(self):
        self.assertEqual(await pytexl.Engine().render('<% "a" - 1 %>'), "[[Undefined]]")

    async def test_render_waits_for_promises(self):
        engine = pytexl.Engine()
        out = await engine.render("<% p = promise.create(), d = p ~> (x -> x * 2), promise.resolve p 21, d %>")
        self.assertEqual(out, "42")

    async def test_concurrent_renders(self):
        async def slow(x):
            await asyncio.sleep(0.01 * x)
            return x

        engine = pytexl.Engine({"slow": slow})
        outs = await asyncio.gather(*(engine.render(f"<% slow {n} %>") for n in (3, 1, 2)))
        self.assertEqual(outs, ["3", "1", "2"])


class ThreadTest(unittest.TestCase):
    def test_engines_and_contexts_raise_on_other_threads(self):
        engine = pytexl.Engine()
        context = engine.create_context()
        errors = []

        def use(call):
            try:
                call()
            except RuntimeError as error:
                errors.append(str(error))

        for call in (lambda: engine.set("x", 1), lambda: engine.create_context(), lambda: context.get("x")):
            thread = threading.Thread(target=use, args=(call,))
            thread.start()
            thread.join()
        self.assertEqual(len(errors), 3)
        self.assertIn("can only be used on the thread that created it", errors[0])
        engine.set("x", 1)  # still fine on this thread

    def test_each_thread_can_have_its_own_engine(self):
        results = []

        def render():
            results.append(asyncio.run(pytexl.Engine().render("<% 6 * 7 %>")))

        threads = [threading.Thread(target=render) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(results, ["42"] * 4)


class EventLoopTest(unittest.TestCase):
    def test_an_engine_outlives_its_event_loop(self):
        # asyncio.run creates a new event loop each time
        engine = pytexl.Engine({"x": 1})
        self.assertEqual(asyncio.run(engine.render("<% x %>")), "1")
        self.assertEqual(asyncio.run(engine.render("<% x + 1 %>")), "2")


class LoadTest(unittest.IsolatedAsyncioTestCase):
    async def test_load_returns_the_document(self):
        engine, _ = memory_engine({"mem://doc": "<% x = 6 %>x * 7 = <% x * 7 %>"})
        doc = await engine.load("mem://doc")
        self.assertIsInstance(doc, pytexl.Document)
        self.assertEqual(doc.uri, "mem://doc")
        self.assertEqual(doc.source, "<% x = 6 %>x * 7 = <% x * 7 %>")
        self.assertEqual(doc.rendering, "x * 7 = 42")
        self.assertEqual(doc.data["x"], 6)
        self.assertIn("texl", doc.data)

    async def test_load_of_a_missing_document_gives_none(self):
        self.assertIsNone(await pytexl.Engine().load(FIXTURES / "missing.texl"))

    async def test_load_of_an_unknown_scheme_gives_none(self):
        self.assertIsNone(await pytexl.Engine().load("ftp://example.com/doc.texl"))

    async def test_load_resolves_relative_uris_against_the_base_url(self):
        engine = pytexl.Engine(base_url=FIXTURES)
        doc = await engine.load("partials/../lib/format.texl")
        self.assertEqual(doc.uri, (FIXTURES / "lib" / "format.texl").as_uri())

    async def test_load_accepts_paths(self):
        cwd = os.getcwd()
        os.chdir(FIXTURES)
        self.addCleanup(os.chdir, cwd)
        doc = await pytexl.Engine(base_url="http://127.0.0.1:1/").load(Path("lib/format.texl"))
        self.assertEqual(doc.uri, (FIXTURES / "lib" / "format.texl").as_uri())

    async def test_load_of_an_invalid_uri_raises(self):
        engine = pytexl.Engine(base_url="mem:opaque")
        with self.assertRaises(ValueError):
            await engine.load("relative.texl")

    async def test_load_caches_documents(self):
        engine, requested = memory_engine({"mem://doc": "<% bind \"n\" ((n ; 0) + 1) %><% n %>"})
        first = await engine.load("mem://doc")
        second = await engine.load("mem://doc")
        self.assertEqual(first.rendering, "1")
        self.assertEqual(second.rendering, "2")  # load always renders the document again...
        out = await engine.render(f'<% texl.import("mem://doc") {RENDERING} %>')
        self.assertEqual(out, "2")  # ...but imports reuse the cached result
        self.assertEqual(requested, ["mem://doc", "mem://doc"])

    async def test_load_reads_local_files(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "my doc.texl"
            path.write_text("<% 1 + 2 %> é", encoding="utf-8")
            doc = await pytexl.Engine().load(path)
        self.assertEqual(doc.rendering, "3 é")
        self.assertTrue(doc.uri.endswith("/my%20doc.texl"))

    async def test_load_fetches_http_documents(self):
        with FileServer() as server:
            doc = await pytexl.Engine().load(server.origin + "/partials/counted.texl")
            missing = await pytexl.Engine().load(server.origin + "/missing.texl")
        self.assertEqual(doc.rendering, "counted.texl has been evaluated 1 time(s)\n")
        self.assertIsNone(missing)

    async def test_document_data_functions_can_be_called(self):
        doc = await pytexl.Engine().load(FIXTURES / "lib" / "format.texl")
        self.assertEqual(doc.data["plural"](3, "apple"), "3 apples")
        self.assertEqual(doc.data["pad"](8)("Total"), "Total...")
        self.assertEqual(doc.data["basename"]("/a/b/c.texl"), "c.texl")


class LoaderTest(unittest.IsolatedAsyncioTestCase):
    async def test_loaders_are_routed_by_scheme(self):
        engine = pytexl.Engine()
        engine.add_loader("one", lambda uri: f"one: {uri}")
        engine.add_loader("two", lambda uri: f"two: {uri}")
        engine.set_default_loader(lambda uri: f"default: {uri}")
        self.assertEqual((await engine.load("one://a")).rendering, "one: one://a")
        self.assertEqual((await engine.load("two://b")).rendering, "two: two://b")
        self.assertEqual(await engine.render(f'<% texl.import("c/d.texl") {RENDERING} %>'), "default: /c/d.texl")

    async def test_schemes_are_case_insensitive(self):
        engine = pytexl.Engine()
        engine.add_loader("MEM", lambda uri: "found")
        self.assertEqual((await engine.load("mem://a")).rendering, "found")

    async def test_add_loader_replaces_the_loader_of_a_scheme(self):
        engine = pytexl.Engine()
        engine.add_loader("file", lambda uri: "replaced")
        self.assertEqual((await engine.load(FIXTURES / "sample.texl")).rendering, "replaced")

    async def test_async_loaders(self):
        async def loader(uri):
            await asyncio.sleep(0.01)
            return "async source"

        engine = pytexl.Engine()
        engine.add_loader("mem", loader)
        self.assertEqual((await engine.load("mem://a")).rendering, "async source")

    async def test_loaders_returning_a_future(self):
        engine = pytexl.Engine()
        loop = asyncio.get_running_loop()

        def loader(uri):
            future = loop.create_future()
            loop.call_soon(future.set_result, "from a future")
            return future

        engine.add_loader("mem", loader)
        self.assertEqual((await engine.load("mem://a")).rendering, "from a future")

    async def test_failing_loaders_give_none(self):
        async def async_failure(uri):
            raise OSError("boom")

        def failure(uri):
            raise KeyError(uri)

        engine = pytexl.Engine()
        engine.add_loader("sync", failure)
        engine.add_loader("async", async_failure)
        engine.add_loader("numb", lambda uri: 42)
        engine.add_loader("none", lambda uri: None)
        for uri in ("sync://a", "async://a", "numb://a", "none://a"):
            with self.subTest(uri=uri):
                self.assertIsNone(await engine.load(uri))
        self.assertEqual(await engine.render('[<% texl.import("sync://a") %>]'), "[]")

    async def test_failed_loads_are_retried(self):
        attempts = []

        def flaky(uri):
            attempts.append(uri)
            if len(attempts) == 1:
                raise OSError("first attempt")
            return "second attempt"

        engine = pytexl.Engine()
        engine.add_loader("mem", flaky)
        self.assertIsNone(await engine.load("mem://a"))
        self.assertEqual((await engine.load("mem://a")).rendering, "second attempt")


class ImportTest(unittest.IsolatedAsyncioTestCase):
    async def test_imports_resolve_against_the_importing_document(self):
        engine, requested = memory_engine(
            {
                "mem://host/docs/main.texl": f'[main <% texl.import("parts/header.texl") {RENDERING} %>]',
                "mem://host/docs/parts/header.texl": f'[header <% texl.import("../footer.texl") {RENDERING} %>]',
                "mem://host/docs/footer.texl": f'[footer <% texl.import("/top.texl") {RENDERING} %>]',
                "mem://host/top.texl": "top",
            }
        )
        doc = await engine.load("mem://host/docs/main.texl")
        self.assertEqual(doc.rendering, "[main [header [footer top]]]")
        self.assertEqual(
            requested,
            [
                "mem://host/docs/main.texl",
                "mem://host/docs/parts/header.texl",
                "mem://host/docs/footer.texl",
                "mem://host/top.texl",
            ],
        )

    async def test_imports_without_a_document_resolve_against_the_root(self):
        engine, requested = memory_engine({"/dir/doc.texl": "doc"})
        out = await engine.render(f'<% texl.import("dir/doc.texl") {RENDERING} %>|<% texl.import("/dir/doc.texl") {RENDERING} %>')
        self.assertEqual(out, "doc|doc")
        self.assertEqual(requested, ["/dir/doc.texl"])

    async def test_imports_without_a_document_read_absolute_file_paths(self):
        path = (FIXTURES / "partials" / "counted.texl").as_uri().removeprefix("file://")
        out = await pytexl.Engine().render(f'<% texl.import("{path}") {RENDERING} %>')
        self.assertEqual(out, "counted.texl has been evaluated 1 time(s)\n")

    async def test_imports_in_a_context_with_a_uri_resolve_against_it(self):
        engine, requested = memory_engine({"mem://host/docs/lib.texl": "lib", "mem://host/top.texl": "top"})
        context = engine.create_context({"x": 1}, uri="mem://host/docs/main.texl")
        out = await context.render(f'<% texl.import("lib.texl") {RENDERING} %>|<% texl.import("/top.texl") {RENDERING} %>|<% x %>')
        self.assertEqual(out, "lib|top|1")
        self.assertEqual(requested, ["mem://host/docs/lib.texl", "mem://host/top.texl"])

    async def test_a_context_uri_resolves_against_the_base_url(self):
        context = pytexl.Engine(base_url=FIXTURES).create_context(uri="partials/new.texl")
        out = await context.render('<% texl.import("signature.texl") ~> (doc -> doc.uri) %>')
        self.assertEqual(out, (FIXTURES / "partials" / "signature.texl").as_uri())

    async def test_a_context_accepts_a_path_as_uri(self):
        cwd = os.getcwd()
        os.chdir(FIXTURES)
        self.addCleanup(os.chdir, cwd)
        context = pytexl.Engine(base_url="http://127.0.0.1:1/").create_context(uri=Path("partials/new.texl"))
        out = await context.render('<% texl.import("../lib/format.texl") ~> (doc -> doc.uri) %>')
        self.assertEqual(out, (FIXTURES / "lib" / "format.texl").as_uri())

    async def test_a_context_with_an_invalid_uri_raises(self):
        with self.assertRaises(ValueError):
            pytexl.Engine(base_url="mem:opaque").create_context(uri="relative.texl")

    async def test_imported_documents_see_the_importer_names(self):
        engine, _ = memory_engine({"/greet.texl": "Hello <% reader %>!"})
        out = await engine.render(f'<% reader = "Ada" %><% texl.import("greet.texl") {RENDERING} %>')
        self.assertEqual(out, "Hello Ada!")

    async def test_concurrent_imports_share_one_load(self):
        async def slow(uri):
            await asyncio.sleep(0.01)
            requested.append(uri)
            return "shared"

        requested = []
        engine = pytexl.Engine()
        engine.set_default_loader(slow)
        out = await engine.render(
            f'<% a = texl.import("/s.texl"), b = texl.import("/s.texl") %><% a {RENDERING} %>,<% b {RENDERING} %>'
        )
        self.assertEqual(out, "shared,shared")
        self.assertEqual(requested, ["/s.texl"])

    async def test_imports_return_the_document_namespace(self):
        engine, _ = memory_engine({"mem://lib": "<% double = x -> x * 2 %>lib"})
        context = engine.create_context()
        await context.render('<% doc = texl.import("mem://lib") %>')
        doc = await context.get("doc")
        self.assertEqual(sorted(doc), ["data", "rendering", "source", "uri"])
        self.assertEqual(doc["rendering"], "lib")
        self.assertEqual(doc["data"]["double"](21), 42)

    async def test_import_of_a_missing_document_gives_nothing(self):
        out = await pytexl.Engine(base_url=FIXTURES).render('[<% texl.import("/nowhere/missing.texl") %>]')
        self.assertEqual(out, "[]")


if __name__ == "__main__":
    unittest.main()
