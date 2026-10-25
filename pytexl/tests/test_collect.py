"""Tests of pytexl.collect, the cycle collector."""

import unittest

import pytexl


class CollectTest(unittest.IsolatedAsyncioTestCase):
    async def test_collect_frees_dropped_contexts(self):
        engine = pytexl.Engine()
        pytexl.collect()

        # A function stored in the context holds its scope: a reference cycle
        context = engine.create_context()
        await context.render("<% f = x -> x %>")
        del context

        self.assertGreater(pytexl.collect(), 0)
        self.assertEqual(pytexl.collect(), 0)

    async def test_collect_keeps_what_is_in_use(self):
        engine = pytexl.Engine()
        context = engine.create_context({"rate": 2})
        await context.render("<% convert = a -> a * rate %>")
        doc = await engine.render("lib.texl", "<% twice = x -> x * 2 %>")
        twice = doc.data["twice"]
        del doc

        pytexl.collect()
        self.assertEqual(await context.render("<% convert 10 %>"), "20")
        self.assertEqual(twice(5), 10)


if __name__ == "__main__":
    unittest.main()
