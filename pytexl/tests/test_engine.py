"""Tests of pytexl.Engine: globals, rendering, documents, loaders and imports."""

import asyncio
import os
import tempfile
import threading
import unittest
from pathlib import Path

import pytexl
from support import FIXTURES, FileServer

RENDERING = "~> (doc -> doc.text)"


def memory_engine(docs, scheme="mem"):
    """Returns an engine whose `scheme` loader serves `docs` (keyed by URI), and the list
    of the URIs it was asked for."""
    requested = []

    def loader(uri):
        requested.append(uri)
        return docs[uri]

    engine = pytexl.Engine()
    engine.add_loader(scheme, loader)
    return engine, requested


async def render(engine, source, uri="mem://host/main.texl"):
    """Renders `source` as the document at `uri` and returns the rendered text."""
    return (await engine.render(uri, source)).text


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
    async def test_render_returns_the_document(self):
        doc = await pytexl.Engine(base_url="mem://host/docs/").render("main.texl#top", "<% x = 10 %>Hello <% x + 20 %>!")
        self.assertIsInstance(doc, pytexl.Document)
        self.assertEqual(doc.uri, "mem://host/docs/main.texl")
        self.assertEqual(doc.source, "<% x = 10 %>Hello <% x + 20 %>!")
        self.assertEqual(doc.text, "Hello 30!")
        self.assertEqual(doc.data["x"], 10)

    async def test_render_accepts_paths(self):
        cwd = os.getcwd()
        os.chdir(FIXTURES)
        self.addCleanup(os.chdir, cwd)
        engine = pytexl.Engine(base_url="http://127.0.0.1:1/")
        doc = await engine.render(Path("partials/new.texl"), '<% texl.import("signature.texl") ~> (doc -> doc.uri) %>')
        self.assertEqual(doc.uri, (FIXTURES / "partials" / "new.texl").as_uri())
        self.assertEqual(doc.text, (FIXTURES / "partials" / "signature.texl").as_uri())

    async def test_render_of_an_invalid_uri_raises(self):
        with self.assertRaises(ValueError):
            await pytexl.Engine(base_url="mem:opaque").render("relative.texl", "")

    async def test_render_uses_the_engine_globals(self):
        engine = pytexl.Engine({"a": 10, "name": "Ada"})
        self.assertEqual(await render(engine, "<% name %>: a = <% a %>"), "Ada: a = 10")

    async def test_set_adds_and_replaces_globals(self):
        engine = pytexl.Engine({"a": 10})
        engine.set("b", 32)
        self.assertEqual(await render(engine, "<% a + b %>"), "42")
        engine.set("a", 0)
        self.assertEqual(await render(engine, "<% a + b %>"), "32")

    async def test_globals_do_not_reach_contexts_created_earlier(self):
        engine = pytexl.Engine()
        context = engine.create_context()
        engine.set("late", "here")
        self.assertEqual((await context.render("[<% late %>]")).text, "[]")
        self.assertEqual((await engine.create_context().render("<% late %>")).text, "here")

    async def test_render_starts_from_a_fresh_scope(self):
        engine = pytexl.Engine()
        first = await render(engine, "<% x = 1 %><% x %>|")
        second = await render(engine, "[<% x %>]")
        self.assertEqual(first + second, "1|[]")

    async def test_bind_is_global_to_the_document(self):
        engine = pytexl.Engine()
        doc = await engine.render("mem://a", '<% (() -> bind "shared" 7)() %><% shared %>')
        self.assertEqual(doc.text, "7")
        self.assertEqual(doc.data["shared"], 7)  # bound in the document scope, like a top-level `=`
        self.assertEqual(await render(engine, "[<% shared %>]"), "[]")

    async def test_render_without_tags_copies_the_text(self):
        self.assertEqual(await render(pytexl.Engine(), "plain text"), "plain text")

    async def test_render_reports_invalid_expressions_as_undefined(self):
        self.assertEqual(await render(pytexl.Engine(), '<% "a" - 1 %>'), "[[Undefined]]")

    async def test_render_waits_for_promises(self):
        engine = pytexl.Engine()
        out = await render(engine, "<% p = promise.create(), d = p ~> (x -> x * 2), promise.resolve p 21, d %>")
        self.assertEqual(out, "42")

    async def test_concurrent_renders(self):
        async def slow(x):
            await asyncio.sleep(0.01 * x)
            return x

        engine = pytexl.Engine({"slow": slow})
        outs = await asyncio.gather(*(render(engine, f"<% slow {n} %>") for n in (3, 1, 2)))
        self.assertEqual(outs, ["3", "1", "2"])

    async def test_concurrent_renders_import_against_their_own_uri(self):
        engine, _ = memory_engine({"mem://a/lib.texl": "A", "mem://b/lib.texl": "B"})
        source = f'<% texl.import("lib.texl") {RENDERING} %>'
        outs = await asyncio.gather(render(engine, source, "mem://a/main.texl"), render(engine, source, "mem://b/main.texl"))
        self.assertEqual(outs, ["A", "B"])


class FetchTest(unittest.IsolatedAsyncioTestCase):
    async def test_fetch_returns_the_url_and_the_source(self):
        engine, requested = memory_engine({"mem://doc": "<% 1 + 1 %>"})
        fetched = await engine.fetch("mem://doc#part")
        self.assertIsInstance(fetched, pytexl.Fetched)
        self.assertEqual((fetched.url, fetched.source), ("mem://doc", "<% 1 + 1 %>"))
        url, source = await engine.fetch("mem://doc")
        self.assertEqual((url, source), ("mem://doc", "<% 1 + 1 %>"))
        self.assertEqual(requested, ["mem://doc", "mem://doc"])

    async def test_fetch_of_a_missing_document_raises(self):
        with self.assertRaises(pytexl.LoadError):
            await pytexl.Engine().fetch(FIXTURES / "missing.texl")
        with self.assertRaises(pytexl.LoadError) as caught:
            await pytexl.Engine().fetch("ftp://example.com/doc.texl")
        self.assertEqual(caught.exception.reason, "no loader for this URL")

    async def test_fetch_resolves_uris_like_load(self):
        engine = pytexl.Engine(base_url=FIXTURES)
        expected = (FIXTURES / "lib" / "format.texl").read_text(encoding="utf-8")
        url = (FIXTURES / "lib" / "format.texl").as_uri()
        self.assertEqual(await engine.fetch("partials/../lib/format.texl"), (url, expected))
        self.assertEqual(await engine.fetch(FIXTURES / "lib" / "format.texl"), (url, expected))

    async def test_fetch_of_an_invalid_uri_raises(self):
        with self.assertRaises(ValueError):
            await pytexl.Engine(base_url="mem:opaque").fetch("relative.texl")


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

        def render_in_thread():
            results.append(asyncio.run(render(pytexl.Engine(), "<% 6 * 7 %>")))

        threads = [threading.Thread(target=render_in_thread) for _ in range(4)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(results, ["42"] * 4)


class EventLoopTest(unittest.TestCase):
    def test_an_engine_outlives_its_event_loop(self):
        # asyncio.run creates a new event loop each time
        engine = pytexl.Engine({"x": 1})
        self.assertEqual(asyncio.run(render(engine, "<% x %>")), "1")
        self.assertEqual(asyncio.run(render(engine, "<% x + 1 %>")), "2")


class ClosedLoopTest(unittest.TestCase):
    def test_rendering_works_after_a_loop_that_never_ran_its_tasks(self):
        """A loop stopped with engine work scheduled on it must not block later loops."""
        engine = pytexl.Engine()
        context = engine.create_context({"a": 1})
        promises = []

        async def start_and_stop():
            asyncio.get_running_loop().stop()
            promises.append(engine._native.render("mem://lost", "<% 1 %>"))

        loop = asyncio.new_event_loop()
        loop.create_task(start_and_stop())
        loop.run_forever()
        loop.close()

        async def render():
            return (await asyncio.wait_for(context.render("<% a + 1 %>"), 5)).text

        self.assertEqual(asyncio.run(render()), "2")
        self.assertEqual(asyncio.run(render()), "2")


class LoadTest(unittest.IsolatedAsyncioTestCase):
    async def test_load_returns_the_document(self):
        engine, _ = memory_engine({"mem://doc": "<% x = 6 %>x * 7 = <% x * 7 %>"})
        doc = await engine.load("mem://doc")
        self.assertIsInstance(doc, pytexl.Document)
        self.assertEqual(doc.uri, "mem://doc")
        self.assertEqual(doc.source, "<% x = 6 %>x * 7 = <% x * 7 %>")
        self.assertEqual(doc.text, "x * 7 = 42")
        self.assertEqual(doc.data["x"], 6)
        self.assertIn("texl", doc.data)

    async def test_load_of_a_missing_document_raises(self):
        with self.assertRaises(pytexl.LoadError):
            await pytexl.Engine().load(FIXTURES / "missing.texl")

    async def test_load_of_an_unknown_scheme_raises(self):
        with self.assertRaises(pytexl.LoadError) as caught:
            await pytexl.Engine().load("ftp://example.com/doc.texl")
        self.assertEqual(caught.exception.reason, "no loader for this URL")

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

    async def test_load_is_fetch_and_render(self):
        engine, _ = memory_engine({"mem://host/main.texl": f'main <% texl.import("lib.texl") {RENDERING} %>', "mem://host/lib.texl": "lib"})
        loaded = await engine.load("mem://host/main.texl")
        rendered = await engine.render(*await engine.fetch("mem://host/main.texl"))
        self.assertEqual((loaded.uri, loaded.source, loaded.text), (rendered.uri, rendered.source, rendered.text))
        self.assertEqual(loaded.text, "main lib")

    async def test_documents_are_not_cached_across_renders(self):
        docs = {"mem://host/lib.texl": "first"}
        engine, requested = memory_engine(docs)
        source = f'<% texl.import("lib.texl") {RENDERING} %>'
        self.assertEqual(await render(engine, source), "first")
        docs["mem://host/lib.texl"] = "second"
        self.assertEqual(await render(engine, source), "second")
        self.assertEqual((await engine.load("mem://host/lib.texl")).text, "second")
        self.assertEqual(requested, ["mem://host/lib.texl"] * 3)

    async def test_load_reads_local_files(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "my doc.texl"
            path.write_text("<% 1 + 2 %> é", encoding="utf-8")
            doc = await pytexl.Engine().load(path)
        self.assertEqual(doc.text, "3 é")
        self.assertTrue(doc.uri.endswith("/my%20doc.texl"))

    async def test_load_fetches_http_documents(self):
        with FileServer() as server:
            doc = await pytexl.Engine().load(server.origin + "/partials/counted.texl")
            with self.assertRaises(pytexl.LoadError) as caught:
                await pytexl.Engine().load(server.origin + "/missing.texl")
        self.assertEqual(doc.text, "counted.texl has been evaluated 1 time(s)\n")
        self.assertIn("404", caught.exception.reason)

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
        self.assertEqual((await engine.load("one://a")).text, "one: one://a")
        self.assertEqual((await engine.load("two://b")).text, "two: two://b")
        self.assertEqual(await render(engine, f'<% texl.import("two://c/d.texl") {RENDERING} %>'), "two: two://c/d.texl")

    async def test_schemes_are_case_insensitive(self):
        engine = pytexl.Engine()
        engine.add_loader("MEM", lambda uri: "found")
        self.assertEqual((await engine.load("mem://a")).text, "found")

    async def test_add_loader_replaces_the_loader_of_a_scheme(self):
        engine = pytexl.Engine()
        engine.add_loader("file", lambda uri: "replaced")
        self.assertEqual((await engine.load(FIXTURES / "sample.texl")).text, "replaced")

    async def test_async_loaders(self):
        async def loader(uri):
            await asyncio.sleep(0.01)
            return "async source"

        engine = pytexl.Engine()
        engine.add_loader("mem", loader)
        self.assertEqual((await engine.load("mem://a")).text, "async source")

    async def test_loaders_returning_a_future(self):
        engine = pytexl.Engine()
        loop = asyncio.get_running_loop()

        def loader(uri):
            future = loop.create_future()
            loop.call_soon(future.set_result, "from a future")
            return future

        engine.add_loader("mem", loader)
        self.assertEqual((await engine.load("mem://a")).text, "from a future")

    async def test_failing_loaders_raise(self):
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
                with self.assertRaises(pytexl.LoadError):
                    await engine.load(uri)
        self.assertEqual(await render(engine, '[<% texl.import("sync://a") %>]'), "[[[Undefined]]]")

    async def test_failed_loads_are_retried(self):
        attempts = []

        def flaky(uri):
            attempts.append(uri)
            if len(attempts) == 1:
                raise OSError("first attempt")
            return "second attempt"

        engine = pytexl.Engine()
        engine.add_loader("mem", flaky)
        with self.assertRaises(pytexl.LoadError):
            await engine.load("mem://a")
        self.assertEqual((await engine.load("mem://a")).text, "second attempt")


def home_engine(docs):
    """Returns an engine whose `mem` loader resolves a `~` segment the way a project home
    does: `.../~/rest` is the document at `mem://host/rest`, its final URL. Sources come
    from `docs`, keyed by final URL. Also returns the list of the URIs it was asked for."""
    requested = []

    async def loader(uri):
        requested.append(uri)
        head, sep, rest = uri.rpartition("/~/")
        final_url = "mem://host/" + rest if sep else uri
        return docs[final_url] if final_url == uri else (final_url, docs[final_url])

    engine = pytexl.Engine()
    engine.add_loader("mem", loader)
    return engine, requested


class FinalUrlTest(unittest.IsolatedAsyncioTestCase):
    async def test_load_gives_the_final_url(self):
        engine, requested = home_engine({"mem://host/lib/x.texl": "x"})
        doc = await engine.load("mem://host/docs/~/lib/x.texl#top")
        self.assertEqual((doc.uri, doc.text), ("mem://host/lib/x.texl", "x"))
        self.assertEqual(await engine.fetch("mem://host/docs/~/lib/x.texl"), ("mem://host/lib/x.texl", "x"))
        # load is render at the fetched URL
        self.assertEqual((await engine.render(*await engine.fetch("mem://host/docs/~/lib/x.texl"))).uri, doc.uri)
        self.assertEqual(requested, ["mem://host/docs/~/lib/x.texl"] * 3)

    async def test_relative_final_urls_resolve_against_the_requested_url(self):
        engine = pytexl.Engine()
        engine.add_loader("mem", lambda uri: ("../b/new.texl", "moved"))
        self.assertEqual((await engine.load("mem://host/a/old.texl")).uri, "mem://host/b/new.texl")

    async def test_a_self_import_through_a_final_url_ends(self):
        engine, requested = home_engine({"mem://host/lib/x.texl": '<% y = texl.import("~/lib/x.texl") %>[<% y %>]'})
        doc = await engine.load("mem://host/lib/x.texl")
        self.assertEqual(doc.text, "[[[Undefined]]]")
        self.assertEqual(requested, ["mem://host/lib/x.texl", "mem://host/lib/~/lib/x.texl"])

    async def test_uris_with_one_final_url_share_one_rendering(self):
        renderings = []
        engine, requested = home_engine({
            "mem://host/lib/x.texl": '<% n = rendered() %>x',
            "mem://host/docs/sub/c.texl": '<% texl.import("~/lib/x.texl") ~> (doc -> bind "x" doc) %>',
        })
        engine.set("rendered", lambda: renderings.append(1))
        doc = await engine.render("mem://host/docs/main.texl", (
            '<% texl.import("~/lib/x.texl") ~> (doc -> bind "a" doc) %>'
            '<% texl.import("sub/c.texl") ~> (doc -> bind "c" doc) %>'
            '<% texl.import("../lib/x.texl") ~> (doc -> bind "b" doc) %>'
        ))
        self.assertEqual(doc.data["a"]["uri"], "mem://host/lib/x.texl")
        self.assertEqual(doc.data["b"]["uri"], "mem://host/lib/x.texl")
        self.assertEqual(len(renderings), 1)
        self.assertEqual(requested, [
            "mem://host/docs/~/lib/x.texl", "mem://host/docs/sub/c.texl", "mem://host/docs/sub/~/lib/x.texl",
        ])

    async def test_redirected_documents_import_relative_to_their_final_url(self):
        engine, requested = home_engine({
            "mem://host/lib/x.texl": f'x<% texl.import("y.texl") {RENDERING} %>',
            "mem://host/lib/y.texl": "y",
        })
        self.assertEqual(await render(engine, f'<% texl.import("~/lib/x.texl") {RENDERING} %>', "mem://host/docs/main.texl"), "xy")
        self.assertEqual(requested, ["mem://host/docs/~/lib/x.texl", "mem://host/lib/y.texl"])

    async def test_a_cycle_with_one_redirected_leg_ends(self):
        engine, _ = home_engine({
            "mem://host/a.texl": '<% texl.import("b.texl") ~> (doc -> bind "b" doc) %>a',
            "mem://host/b.texl": '<% x = texl.import("~/a.texl") %>[<% x %>]',
        })
        doc = await engine.load("mem://host/a.texl")
        self.assertEqual(doc.data["b"]["text"], "[[[Undefined]]]")

    async def test_other_results_fail(self):
        engine = pytexl.Engine()
        for result in (["mem://host/a", "list"], ("mem://host/a",), ("mem://host/a", 42), (1, "source")):
            engine.add_loader("mem", lambda uri, result=result: result)
            with self.subTest(result=result):
                with self.assertRaises(pytexl.LoadError) as raised:
                    await engine.load("mem://host/z")
                self.assertEqual(raised.exception.reason, "the loader did not return a string or a (final_url, source) tuple")

    async def test_http_redirects_give_the_final_url(self):
        with FileServer() as server:
            doc = await pytexl.Engine().load(server.origin + "/redirect/partials/greeting.texl")
            self.assertEqual(doc.uri, server.origin + "/partials/greeting.texl")
            self.assertIn("-- the Texl team", doc.text)


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
        self.assertEqual(doc.text, "[main [header [footer top]]]")
        self.assertEqual(
            requested,
            [
                "mem://host/docs/main.texl",
                "mem://host/docs/parts/header.texl",
                "mem://host/docs/footer.texl",
                "mem://host/top.texl",
            ],
        )

    async def test_imports_resolve_against_the_rendered_uri(self):
        engine, requested = memory_engine({"mem://host/docs/lib.texl": "lib", "mem://host/top.texl": "top"})
        out = await render(engine, f'<% texl.import("lib.texl") {RENDERING} %>|<% texl.import("/top.texl") {RENDERING} %>', "mem://host/docs/main.texl")
        self.assertEqual(out, "lib|top")
        self.assertEqual(requested, ["mem://host/docs/lib.texl", "mem://host/top.texl"])

    async def test_absolute_paths_read_files_from_the_root(self):
        path = (FIXTURES / "partials" / "counted.texl").as_uri().removeprefix("file://")
        out = await render(pytexl.Engine(), f'<% texl.import("{path}") {RENDERING} %>', FIXTURES / "new.texl")
        self.assertEqual(out, "counted.texl has been evaluated 1 time(s)\n")

    async def test_imported_documents_do_not_see_the_importer_names(self):
        engine, _ = memory_engine({"mem://host/greet.texl": "Hello <% reader %>!"})
        out = await render(engine, f'<% reader = "Ada" %><% texl.import("greet.texl") {RENDERING} %>')
        self.assertEqual(out, "Hello !")

    async def test_a_self_import_gives_undefined(self):
        engine, requested = memory_engine({})
        doc = await engine.render("mem://host/main.texl", '<% me = texl.import("main.texl") %>done')
        self.assertIsInstance(doc.data["me"], pytexl.Undefined)
        self.assertEqual(doc.text, "done")
        self.assertEqual(requested, [])

    async def test_import_cycles_give_undefined(self):
        engine, _ = memory_engine({"mem://host/b.texl": '<% back = texl.import("main.texl") %>b'})
        doc = await engine.render("mem://host/main.texl", f'<% texl.import("b.texl") {RENDERING} %>')
        self.assertEqual(doc.text, "b")

    async def test_concurrent_imports_share_one_load(self):
        async def slow(uri):
            await asyncio.sleep(0.01)
            requested.append(uri)
            return "shared"

        requested = []
        engine = pytexl.Engine()
        engine.add_loader("mem", slow)
        out = await render(
            engine, f'<% a = texl.import("/s.texl"), b = texl.import("s.texl#x") %><% a {RENDERING} %>,<% b {RENDERING} %>'
        )
        self.assertEqual(out, "shared,shared")
        self.assertEqual(requested, ["mem://host/s.texl"])

    async def test_imports_return_the_document_namespace(self):
        engine, _ = memory_engine({"mem://lib": "<% double = x -> x * 2 %>lib"})
        main = await engine.render("mem://main", '<% doc = texl.import("mem://lib") %>')
        doc = await main.data["doc"]
        self.assertEqual(sorted(doc), ["data", "source", "tags", "text", "uri"])
        self.assertEqual(doc["text"], "lib")
        self.assertEqual(doc["data"]["double"](21), 42)

    async def test_import_of_a_missing_document_gives_undefined(self):
        out = await render(pytexl.Engine(base_url=FIXTURES), '[<% texl.import("/nowhere/missing.texl") %>]', "main.texl")
        self.assertEqual(out, "[[[Undefined]]]")

    async def test_a_failed_import_says_why(self):
        def failure(uri):
            raise ValueError("boom")

        engine = pytexl.Engine()
        engine.add_loader("sync", failure)
        doc = await engine.render("mem://main", 'é <% x = texl.import "sync://a" %><% x %>')
        x = await doc.data["x"]
        self.assertIsInstance(x, pytexl.Undefined)
        self.assertEqual(x.op, "document.load")
        self.assertEqual(x.lhv, "sync://a")
        self.assertEqual(x.rhv, "ValueError: boom")
        self.assertEqual(x.description, "cannot load `sync://a`: ValueError: boom")
        self.assertEqual(x.uri, "mem://main")
        self.assertEqual(x.pos, doc.source.index('"'))

    async def test_fetch_and_load_raise_the_reason(self):
        def failure(uri):
            raise ValueError("boom")

        engine = pytexl.Engine()
        engine.add_loader("sync", failure)
        for call in (engine.fetch("sync://a"), engine.load("sync://a#top")):
            with self.assertRaises(pytexl.LoadError) as caught:
                await call
            self.assertEqual(str(caught.exception), "cannot load `sync://a`: ValueError: boom")
            self.assertEqual(caught.exception.uri, "sync://a")
            self.assertEqual(caught.exception.reason, "ValueError: boom")

    async def test_contexts_cannot_import(self):
        context = pytexl.Engine().create_context()
        self.assertEqual(context.get("texl"), {})


class TagsTest(unittest.IsolatedAsyncioTestCase):
    async def test_document_tags(self):
        source = "a<% x = 1 %>b<% x + 1 %>c"
        doc = await pytexl.Engine().render("mem://host/main.texl", source)
        self.assertEqual(doc.text, "ab2c")
        self.assertEqual(doc.tags, [
            pytexl.Tag(1, 12, 1, 1, None),
            pytexl.Tag(13, 24, 2, 3, 2),
        ])
        self.assertEqual(source[doc.tags[1].start:doc.tags[1].end], "<% x + 1 %>")

    async def test_positions_are_code_points(self):
        source = 'µ° è <% y = "a" - 1 %>😀 <% y %>!'
        doc = await pytexl.Engine().render("mem://host/main.texl", source)
        tag = doc.tags[1]
        self.assertEqual(source[tag.start:tag.end], "<% y %>")
        self.assertEqual(doc.text[tag.out_start:tag.out_end], "[[Undefined]]")
        # The assignment renders nothing; the error is in `data`, with its place in the source
        u = doc.data["y"]
        self.assertEqual(source[u.pos], "-")
        self.assertEqual(u.source, source)
        self.assertEqual(u.uri, "mem://host/main.texl")
        # The same value through the second tag has the same position
        self.assertEqual(tag.value.pos, u.pos)

    async def test_imported_documents_have_their_own_tags_and_positions(self):
        lib = 'è <% f = x -> x - 1 %>'
        engine, _ = memory_engine({"mem://host/lib.texl": lib})
        main = '<% texl.import "lib.texl" ~> (l -> (bind "f" (l.data.f), l)) %><% f "z" %>'
        doc = await engine.render("mem://host/main.texl", main)
        lib_doc = doc.tags[0].value
        self.assertEqual(lib_doc["tags"][0]["start"], 2)
        u = doc.tags[1].value
        self.assertEqual(u.uri, "mem://host/lib.texl")
        self.assertEqual(u.source, lib)
        self.assertEqual(lib[u.pos], "-")

    async def test_context_rendering(self):
        context = pytexl.Engine().create_context({"a": 1})
        rendering = await context.render("° <% b = a + 1 %><% b %>")
        self.assertIsInstance(rendering, pytexl.Rendering)
        self.assertEqual(rendering.text, "° 2")
        self.assertEqual(rendering.data["b"], 2)
        self.assertEqual(rendering.data["a"], 1)
        self.assertEqual(rendering.tags[1], pytexl.Tag(17, 24, 2, 3, 2))


if __name__ == "__main__":
    unittest.main()
