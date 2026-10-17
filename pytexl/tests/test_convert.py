"""Tests of the conversions between Python values and Swan values."""

import unittest
from collections.abc import Mapping

import pytexl


class Settings(Mapping):
    """A mapping that is not a dict."""

    def __init__(self, **items):
        self._items = items

    def __getitem__(self, key):
        return self._items[key]

    def __iter__(self):
        return iter(self._items)

    def __len__(self):
        return len(self._items)


class PythonToSwanTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def swan_type(self, value):
        """Returns the Swan type of `value` and its rendering, or "Nothing|" for Nothing."""
        self.context.set("v", value)
        return await self.context.render('<% type v ; "Nothing" %>|<% v %>')

    async def test_scalars(self):
        cases = [
            (None, "Nothing|"),
            (True, "Bool|TRUE"),
            (False, "Bool|FALSE"),
            (42, "Numb|42"),
            (1.5, "Numb|1.5"),
            (-0.25, "Numb|-0.25"),
            ("hello", "Text|hello"),
            ("", "Text|"),
        ]
        for value, expected in cases:
            with self.subTest(value=value):
                self.assertEqual(await self.swan_type(value), expected)

    async def test_lists(self):
        self.assertEqual(await self.swan_type([1, "a"]), "List|[[List of 2]]")
        self.assertEqual(await self.context.render("<% list.enum v %>"), "1, a")
        self.context.set("v", [[1, 2], [3]])
        self.assertEqual(await self.context.render("<% list.size (v 0) %>"), "2")

    async def test_tuples(self):
        self.assertEqual(await self.swan_type((1, "a")), "Numb, Text|1, a")
        self.assertEqual(await self.swan_type(()), "Nothing|")
        self.assertEqual(await self.swan_type((7,)), "Numb|7")
        self.assertEqual(await self.swan_type((1, (), (2, 3))), "Numb, Numb, Numb|1, 2, 3")

    async def test_mappings(self):
        self.assertEqual(await self.swan_type({"x": 3, "y": 4}), "Namespace|[[Namespace of 2]]")
        self.assertEqual(await self.context.render("<% v.x * v.y %>"), "12")
        self.assertEqual(await self.swan_type(Settings(a=1)), "Namespace|[[Namespace of 1]]")
        self.assertEqual(await self.context.render("<% v.a %>"), "1")

    async def test_non_string_keys_are_skipped(self):
        self.assertEqual(await self.swan_type({"a": 1, 2: "b", (3,): "c"}), "Namespace|[[Namespace of 1]]")

    async def test_nested_values(self):
        self.context.set("v", {"user": {"name": "Ada", "tags": ["a", "b"]}})
        self.assertEqual(await self.context.render("<% v.user.name %>: <% list.enum v.user.tags %>"), "Ada: a, b")

    async def test_callables(self):
        class Doubler:
            def __call__(self, x):
                return 2 * x

        self.assertEqual(await self.swan_type(len), "Func|[[Func]]")
        self.assertEqual(await self.swan_type(Doubler()), "Func|[[Func]]")
        self.assertEqual(await self.context.render("<% v 21 %>"), "42")

    async def test_unsupported_values_give_nothing(self):
        for value in (object(), b"bytes", {1, 2}, 3j):
            with self.subTest(value=value):
                self.assertEqual(await self.swan_type(value), "Nothing|")

    async def test_large_integers(self):
        self.assertEqual(await self.swan_type(2**60), "Numb|1152921504606847000")
        self.assertEqual(await self.swan_type(10**400), "Nothing|")


class SwanToPythonTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def value(self, expression):
        """Returns the value of a Swan expression, converted to Python."""
        await self.context.render(f"<% v = ({expression}) %>")
        return self.context.get("v")

    async def test_scalars(self):
        cases = [
            ("()", None),
            ("TRUE", True),
            ("FALSE", False),
            ('"hello"', "hello"),
            ("1.5", 1.5),
        ]
        for expression, expected in cases:
            with self.subTest(expression=expression):
                self.assertEqual(await self.value(expression), expected)

    async def test_integral_numbers_are_ints(self):
        for expression, expected in [("42", 42), ("-3", -3), ("0", 0), ("2 ^ 53 - 1", 2**53 - 1)]:
            with self.subTest(expression=expression):
                value = await self.value(expression)
                self.assertEqual(value, expected)
                self.assertIs(type(value), int)

    async def test_other_numbers_are_floats(self):
        for expression in ("0.5", "2 ^ 53", "1 / 0", "math.PI"):
            with self.subTest(expression=expression):
                self.assertIs(type(await self.value(expression)), float)
        self.assertEqual(await self.value("2 ^ 60"), 2.0**60)
        self.assertEqual(await self.value("1 / 0"), float("inf"))

    async def test_lists_and_tuples(self):
        self.assertEqual(await self.value('[1, "a", [TRUE]]'), [1, "a", [True]])
        self.assertEqual(await self.value('1, "a"'), (1, "a"))
        self.assertEqual(await self.value("[[1, 2], 3]"), [[1, 2], 3])

    async def test_namespaces_are_dicts_with_sorted_keys(self):
        value = await self.value("{b: 2, a: {c: [1]}}")
        self.assertEqual(value, {"a": {"c": [1]}, "b": 2})
        self.assertEqual(list(value), ["a", "b"])

    async def test_nothing_is_none(self):
        self.assertIsNone(await self.value("()"))
        self.assertIsNone(await self.value("(), ()"))
        self.assertIsNone(self.context.get("unbound"))

    async def test_undefined_values_are_undefined(self):
        self.assertIsInstance(await self.value('"a" - 1'), pytexl.Undefined)

    async def test_functions_and_promises(self):
        self.assertIsInstance(await self.value("x -> x"), pytexl.Function)
        self.assertIsInstance(await self.value("text.upper"), pytexl.Function)
        self.assertIsInstance(await self.value("promise.create()"), pytexl.Promise)


class UndefinedTest(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.context = pytexl.Engine().create_context()

    async def undefined(self, expression):
        """Returns the value of a Swan expression that must be undefined."""
        await self.context.render(f"<% u = ({expression}) %>")
        value = self.context.get("u")
        self.assertIsInstance(value, pytexl.Undefined)
        return value

    async def test_binary_operation(self):
        u = await self.undefined('"hello" - 1')
        self.assertEqual(u.type, "BinaryOperation")
        self.assertEqual((u.op, u.lhv, u.rhv), ("subtraction", "hello", 1))
        self.assertEqual(u.pos, 14)  # the position of "-" in ' u = ("hello" - 1)'
        self.assertEqual(u.inspect(), {"type": "BinaryOperation", "op": "subtraction", "lhv": "hello", "rhv": 1, "pos": 14})

    async def test_unary_operation(self):
        u = await self.undefined("text.size 1")
        self.assertEqual(u.inspect(), {"type": "UnaryOperation", "op": "text.size", "val": 1, "pos": 0})
        u = await self.undefined('-"a"')
        self.assertEqual((u.type, u.val), ("UnaryOperation", "a"))

    async def test_value(self):
        u = await self.undefined('undef.from ("not found", 404)')
        self.assertEqual(u.inspect(), {"type": "Value", "val": ("not found", 404)})

    async def test_syntax_errors(self):
        termination = await self.undefined('(parse "(1 + 2") {}')
        self.assertEqual(termination.inspect(), {"type": "Termination", "expected": ")", "pos": 6})
        assignment = await self.undefined('(parse "1 = 2") {}')
        self.assertEqual((assignment.type, assignment.op), ("BinaryOperation", "assignment"))
        self.assertEqual(assignment.lhv.inspect(), {"type": "Name", "pos": 0})

    async def test_attributes_that_do_not_apply_are_none(self):
        u = await self.undefined("undef.from 1")
        for name in ("op", "lhv", "rhv", "pos", "expected", "token"):
            with self.subTest(name=name):
                self.assertIsNone(getattr(u, name))

    async def test_nested_undefined_operands(self):
        u = await self.undefined('("a" - 1) + 5')
        self.assertEqual((u.op, u.rhv), ("sum", 5))
        self.assertIsInstance(u.lhv, pytexl.Undefined)
        self.assertEqual((u.lhv.op, u.lhv.lhv, u.lhv.rhv), ("subtraction", "a", 1))
        self.assertIsInstance(u.inspect()["lhv"], pytexl.Undefined)

    async def test_undefined_is_falsy(self):
        self.assertFalse(await self.undefined('"a" - 1'))

    async def test_equality_ignores_the_position(self):
        a = await self.undefined('"a" - 1')
        b = await self.undefined('   "a" - 1')
        c = await self.undefined('"b" - 1')
        self.assertNotEqual(a.pos, b.pos)
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)
        self.assertNotEqual(a, None)
        with self.assertRaises(TypeError):
            hash(a)

    async def test_repr(self):
        self.assertEqual(
            repr(await self.undefined('"hello" - 1')),
            "<pytexl.Undefined BinaryOperation op='subtraction' lhv='hello' rhv=1 pos=14>",
        )
        self.assertEqual(repr(await self.undefined('undef.from "x"')), "<pytexl.Undefined Value val='x'>")

    async def test_undefined_goes_back_to_swan_unchanged(self):
        u = await self.undefined('"hello" - 1')
        self.context.set("back", u)
        self.assertEqual(await self.context.render("<% type back %>"), "Undefined")
        self.assertEqual(await self.context.render("<% (undef.inspect back).(op, pos) %>"), "subtraction, 14")
        self.assertEqual(await self.context.render("<% back %>"), "[[Undefined]]")

    async def test_python_functions_receive_and_return_undefined(self):
        received = []
        self.context.set("spy", lambda x: received.append(x) or x)
        self.assertEqual(await self.context.render('<% type (spy ("a" - 1)) %>'), "Undefined")
        self.assertIsInstance(received[0], pytexl.Undefined)
        self.assertEqual(received[0].op, "subtraction")

    async def test_undefined_in_containers_and_document_data(self):
        await self.context.render('<% xs = [1, "a" - 1], ns = {bad: undef.from "x"} %>')
        self.assertIsInstance(self.context.get("xs")[1], pytexl.Undefined)
        self.assertEqual(self.context.get("ns")["bad"].val, "x")
        engine = pytexl.Engine()
        engine.add_loader("mem", lambda uri: '<% broken = "a" * "b" %>')
        doc = await engine.load("mem://doc")
        self.assertEqual(doc.data["broken"].op, "product")

    async def test_round_trips(self):
        for value in (None, True, 42, 1.5, "text", [1, [2]], (1, "a"), {"k": ["v"]}, {"n": None}):
            with self.subTest(value=value):
                self.context.set("v", value)
                self.assertEqual(self.context.get("v"), value)


if __name__ == "__main__":
    unittest.main()
