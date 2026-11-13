"""Tests of pytexl.Context, of Python functions and awaitables in templates, and of
Swan promises and functions in Python."""

import asyncio
import unittest

import pytexl


class ContextTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.engine = pytexl.Engine({"g": "global"})
        self.context = self.engine.create_context({"a": 10, "b": 20, "name": "World"})

    async def test_recursion_without_end_is_undefined(self):
        rendering = await self.context.render("<% f = n -> 1 + f (n + 1) %><% f 0 %>|<% 1 + 1 %>")
        self.assertEqual(rendering.text, "[[Undefined]]|2")
        undefined = rendering.tags[1].value
        self.assertEqual(undefined.description, "recursion too deep")
        self.assertIsNone(undefined.cause)
        # Called from Python, and through a Python function
        self.assertEqual(self.context.get("f")(0).description, "recursion too deep")
        self.context.set("again", lambda n: self.context.get("g")(n + 1))
        deep = await self.context.render("<% g = n -> again n %><% g 0 %>")
        self.assertEqual(deep.tags[1].value.description, "recursion too deep")
        # A recursion that ends is not cut short
        self.assertEqual((await self.context.render("<% s = n -> n > 0 ? n + s (n - 1) ; 0, s 150 %>")).text, "11325")

    def test_recursion_without_end_is_undefined_on_another_thread(self):
        import threading

        result = []

        async def render():
            context = pytexl.Engine().create_context()
            rendering = await context.render("<% f = n -> 1 + f (n + 1), f 0 %>")
            result.append(rendering.tags[0].value.description)

        thread = threading.Thread(target=lambda: asyncio.run(render()))
        thread.start()
        thread.join()
        self.assertEqual(result, ["recursion too deep"])

    async def test_names_lists_the_context_bindings(self):
        self.assertEqual(self.context.names(), ["a", "b", "name", "texl"])
        self.assertEqual(self.engine.create_context().names(), ["texl"])

    async def test_get_reads_bindings_globals_and_builtins(self):
        self.assertEqual(self.context.get("a"), 10)
        self.assertEqual(self.context.get("g"), "global")
        self.assertIsInstance(self.context.get("math")["sqrt"], pytexl.Function)
        self.assertIsNone(self.context.get("unbound"))

    async def test_set_adds_and_replaces_bindings(self):
        self.context.set("c", 30)
        self.context.set("a", 1)
        self.assertEqual(self.context.get("c"), 30)
        self.assertEqual((await self.context.render("<% a + c %>")).text, "31")
        self.assertIn("c", self.context.names())

    async def test_set_shadows_globals(self):
        self.context.set("g", "local")
        self.assertEqual((await self.context.render("<% g %>")).text, "local")
        self.assertEqual((await self.engine.render("doc.texl", "<% g %>")).text, "global")

    async def test_render_interpolates(self):
        self.assertEqual((await self.context.render("Hello <% name %>!")).text, "Hello World!")
        self.assertEqual((await self.context.render("Sum: <% a + b %>")).text, "Sum: 30")
        self.assertEqual((await self.context.render("<% g %>")).text, "global")

    async def test_assignments_persist_in_the_context(self):
        self.assertEqual((await self.context.render("<% x = a * 5 %><% y : b + 10 %>")).text, "30")
        self.assertEqual(self.context.get("x"), 50)
        self.assertEqual(self.context.get("y"), 30)
        self.assertEqual((await self.context.render("<% x + y %>")).text, "80")
        self.assertEqual(self.context.names(), ["a", "b", "name", "texl", "x", "y"])

    async def test_contexts_are_independent(self):
        other = self.engine.create_context()
        await self.context.render("<% only_here = 1 %>")
        self.assertIsNone(other.get("only_here"))

    async def test_create_context_accepts_any_mapping(self):
        from types import MappingProxyType

        context = self.engine.create_context(MappingProxyType({"k": "v"}))
        self.assertEqual(context.get("k"), "v")


class PythonFunctionTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def render(self, template, **names):
        for name, value in names.items():
            self.context.set(name, value)
        return (await self.context.render(template)).text

    async def test_functions_are_called_with_one_argument(self):
        self.assertEqual(await self.render("<% double 21 %>", double=lambda x: 2 * x), "42")

    async def test_tuples_are_spread_as_arguments(self):
        self.assertEqual(await self.render("<% add(2, 3) %>", add=lambda a, b: a + b), "5")
        self.assertEqual(await self.render("<% count(1, 2, 3) %>", count=lambda *args: len(args)), "3")

    async def test_nothing_is_passed_as_no_argument(self):
        self.assertEqual(await self.render("<% answer() %>", answer=lambda: 42), "42")

    async def test_lists_and_namespaces_are_passed_as_one_argument(self):
        self.assertEqual(await self.render("<% size [1, 2, 3] %>", size=len), "3")
        self.assertEqual(await self.render("<% keys {b: 1, a: 2} %>", keys=lambda d: ",".join(d)), "a,b")

    async def test_mappings_render_with_their_text_hook(self):
        table = {"rows": [[1, 2], [3, 4]], "__text__": lambda t: f"<table of {len(t['rows'])}>"}
        self.assertEqual(await self.render("<% t: table %> <% list.size(t.rows) %>", table=table), "<table of 2> 2")
        self.assertEqual(await self.render("<% {__text__: hook} %>", hook=lambda t: 1), "[[Undefined]]")

    async def test_functions_can_be_mapped(self):
        self.assertEqual(await self.render("<% (1, 2, 3) => square %>", square=lambda x: x * x), "1, 4, 9")

    async def test_exceptions_give_undefined(self):
        def fail(x):
            raise RuntimeError("boom")

        self.assertEqual(await self.render("[<% fail 1 %>]", fail=fail), "[[[Undefined]]]")
        self.assertEqual(await self.render("[<% one 1 %>]", one=lambda: 1), "[[[Undefined]]]")

    async def test_async_functions_are_awaited(self):
        async def slow_double(x):
            await asyncio.sleep(0.01)
            return 2 * x

        self.assertEqual(await self.render("<% slow_double 21 %>", slow_double=slow_double), "42")
        self.assertEqual(await self.render("<% slow_double 1 ~> (x -> x + 1) %>"), "3")

    async def test_failing_async_functions_give_undefined(self):
        async def fail():
            raise RuntimeError("boom")

        self.assertEqual(await self.render("[<% fail() %>]", fail=fail), "[[[Undefined]]]")

    async def test_host_functions_receive_swan_functions(self):
        self.assertEqual(await self.render("<% apply(x -> x + 1, 41) %>", apply=lambda f, x: f(x)), "42")


class AwaitableTest(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.loop = asyncio.get_running_loop()
        self.context = pytexl.Engine().create_context()

    async def test_futures_are_awaited(self):
        future = self.loop.create_future()
        self.loop.call_later(0.01, future.set_result, 42)
        self.context.set("later", future)
        self.assertEqual((await self.context.render("<% later %>")).text, "42")

    async def test_coroutines_are_awaited_once(self):
        calls = []

        async def compute():
            calls.append(1)
            return "computed"

        self.context.set("value", compute())
        self.assertEqual((await self.context.render("<% value %> <% value %>")).text, "computed computed")
        self.assertEqual(calls, [1])

    async def test_tasks_are_awaited(self):
        async def compute():
            await asyncio.sleep(0.01)
            return [1, 2]

        self.context.set("task", asyncio.create_task(compute()))
        self.assertEqual((await self.context.render("<% task ~> list.enum %>")).text, "1, 2")

    async def test_awaitables_resolving_to_awaitables_are_awaited(self):
        async def inner():
            return "inner"

        async def outer():
            return inner()

        self.context.set("nested", outer())
        self.assertEqual((await self.context.render("<% nested %>")).text, "inner")

    async def test_failed_and_cancelled_futures_give_undefined(self):
        failed = self.loop.create_future()
        failed.set_exception(RuntimeError("boom"))
        cancelled = self.loop.create_future()
        cancelled.cancel()
        self.context.set("failed", failed)
        self.context.set("cancelled", cancelled)
        rendering = await self.context.render("<% failed %>|<% cancelled %>")
        self.assertEqual(rendering.text, "[[Undefined]]|[[Undefined]]")
        self.assertEqual(rendering.tags[0].value.error, "RuntimeError: boom")
        self.assertEqual(rendering.tags[1].value.error, "CancelledError")


class AwaitableOutsideLoopTest(unittest.TestCase):
    def test_awaitables_set_outside_an_event_loop(self):
        async def compute():
            return "late"

        engine = pytexl.Engine({"value": compute()})
        self.assertEqual(asyncio.run(engine.render("doc.texl", "<% value %>")).text, "late")


class PromiseTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def test_swan_promises_come_back_as_promises(self):
        await self.context.render("<% p = promise.create() %>")
        p = self.context.get("p")
        self.assertIsInstance(p, pytexl.Promise)
        self.assertFalse(p.done)
        self.assertEqual(repr(p), "<pytexl.Promise pending>")
        await self.context.render("<% promise.resolve p (1, 2) %>")
        self.assertTrue(p.done)
        self.assertEqual(repr(p), "<pytexl.Promise resolved>")
        self.assertEqual(await p, (1, 2))

    async def test_promises_can_be_awaited_before_they_resolve(self):
        await self.context.render("<% p = promise.create() %>")
        waiting = asyncio.ensure_future(self.context.get("p"))
        await asyncio.sleep(0)
        self.assertFalse(waiting.done())
        await self.context.render("<% promise.resolve p 42 %>")
        self.assertEqual(await waiting, 42)

    async def test_promises_can_be_awaited_many_times(self):
        await self.context.render('<% p = promise.create(), promise.resolve p "x" %>')
        p = self.context.get("p")
        self.assertEqual([await p, await p], ["x", "x"])

    async def test_promises_of_async_functions(self):
        async def slow(x):
            await asyncio.sleep(0.01)
            return 2 * x

        self.context.set("slow", slow)
        await self.context.render("<% r = slow 10 %>")
        self.assertEqual(await self.context.get("r"), 20)

    async def test_promises_go_back_to_swan_unchanged(self):
        await self.context.render("<% p = promise.create() %>")
        self.context.set("q", self.context.get("p"))
        await self.context.render("<% promise.resolve q 7 %>")
        self.assertEqual(await self.context.get("p"), 7)

    async def test_promises_of_failed_operations_resolve_to_undefined(self):
        await self.context.render('<% p = promise.create(), d = p ~> (x -> x - 1), promise.resolve p "a" %>')
        u = await self.context.get("d")
        self.assertIsInstance(u, pytexl.Undefined)
        self.assertEqual((u.op, u.lhv, u.rhv), ("subtraction", "a", 1))


class FunctionTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def test_swan_functions_come_back_as_functions(self):
        await self.context.render("<% double = x -> x * 2, add = (a, b) -> a + b, answer = () -> 42 %>")
        double = self.context.get("double")
        self.assertIsInstance(double, pytexl.Function)
        self.assertEqual(double(21), 42)
        self.assertEqual(self.context.get("add")(2, 3), 5)
        self.assertEqual(self.context.get("answer")(), 42)

    async def test_builtins_can_be_called(self):
        text = self.context.get("text")
        self.assertEqual(text["upper"]("texl"), "TEXL")
        self.assertEqual(text["split"](",")("a,b"), ("a", "b"))
        self.assertEqual(repr(text["upper"]), "<pytexl.Function text.upper>")

    async def test_text_template(self):
        tfunc = self.context.get("text")["template"]("My car is {color} and it is from {year}.")
        self.assertIsInstance(tfunc, pytexl.Function)
        # A dict is a namespace; integral numbers render without decimals
        self.assertEqual(tfunc({"color": "white", "year": 2012}), "My car is white and it is from 2012.")
        # A missing name renders nothing; a list of dicts is not a namespace
        self.assertEqual(tfunc({"colour": "white", "year": 2012}), "My car is  and it is from 2012.")
        self.assertIsInstance(tfunc([{"color": "white"}]), pytexl.Undefined)
        # A template made in Swan, filled with Python values
        self.context.set("car", {"color": "red", "year": 1999})
        result = await self.context.render('<% card = text.template "{color}/{year}" %><% card car %>')
        self.assertEqual(result.text, "red/1999")
        self.assertEqual(self.context.get("card")({"color": "blue"}), "blue/")

    async def test_invalid_calls_return_undefined(self):
        await self.context.render("<% double = x -> x * 2 %>")
        result = self.context.get("double")("a")
        self.assertIsInstance(result, pytexl.Undefined)
        self.assertEqual((result.op, result.lhv, result.rhv), ("product", "a", 2))

    async def test_python_functions_round_trip(self):
        def f(x):
            return x + 1

        self.context.set("f", f)
        self.assertIs(self.context.get("f"), f)
        await self.context.render("<% g = f, fs = [f] %>")
        self.assertIs(self.context.get("g"), f)
        self.assertIs(self.context.get("fs")[0], f)
        self.assertEqual((await self.context.render("<% f 1 %>")).text, "2")

    async def test_swan_functions_round_trip(self):
        await self.context.render("<% double = x -> x * 2 %>")
        self.context.set("d", self.context.get("double"))
        self.assertIsInstance(self.context.get("d"), pytexl.Function)
        self.assertEqual((await self.context.render("<% d 5 %>")).text, "10")

    async def test_functions_returning_promises(self):
        async def slow(x):
            return x

        self.context.set("slow", slow)
        await self.context.render("<% wrapped = x -> slow x %>")
        self.assertEqual(await self.context.get("wrapped")("value"), "value")


if __name__ == "__main__":
    unittest.main()
