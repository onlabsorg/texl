const assert = require('node:assert/strict');
const { create_context, render } = require('../pkg-node/texl_wasm.js');

async function testTexlWasm() {
    console.log("Running texl-wasm test suite...");

    // 1. Context Creation & Variable Setting
    const context = create_context({
        a: 10,
        b: 20,
        name: "World",
        double: (x) => 2 * x,
        asyncDouble: async (x) => 2 * x,
    });

    assert.ok(context, "Context should be created successfully");

    // Test context.names()
    const initialNames = context.names();
    assert.ok(initialNames.includes('a'), "Context should include 'a'");
    assert.ok(initialNames.includes('double'), "Context should include 'double'");

    // Test context.get()
    assert.strictEqual(context.get('a'), 10, "context.get('a') should return 10");
    assert.strictEqual(context.get('name'), "World", "context.get('name') should return 'World'");

    // Test context.set()
    context.set('c', 30);
    assert.strictEqual(context.get('c'), 30, "context.set should successfully set key 'c'");

    // 2. Template Rendering & Basic Expressions
    const textRender = await render("Hello <% name %>!", context);
    assert.strictEqual(textRender, "Hello World!", "Template tag interpolation failed");

    const mathRender = await render("Sum: <% a + b + c %>", context);
    assert.strictEqual(mathRender, "Sum: 60", "Arithmetic expression failed");

    // 3. Invoking JavaScript Callbacks from Swan
    const fnRender = await render("Double: <% double(a) %>", context);
    assert.strictEqual(fnRender, "Double: 20", "JS function invocation failed");

    // 4. In-Template Context Assignment & Retrival
    // Assignment (=) and Labelling (:) mutate the context scope
    await render("Assign: <% x = a * 5 %>, Label: <% y : b + 10 %>", context);
    assert.strictEqual(context.get('x'), 50, "Variable x assigned in template should be retrievable");
    assert.strictEqual(context.get('y'), 30, "Variable y labelled in template should be retrievable");

    // 5. Swan Core Built-ins (Math, Text, List, Namespace)
    const builtinsTemplate = `
        Sqrt: <% math.sqrt(16) %>
        Upper: <% text.upper("hello") %>
        ListSize: <% list.size([1, 2, 3, 4]) %>
        Nspace: <% { p: 100, q: 200 }.p %>
    `.trim();

    const builtinsResult = await render(builtinsTemplate, context);
    assert.ok(builtinsResult.includes("Sqrt: 4"), "math.sqrt failed");
    assert.ok(builtinsResult.includes("Upper: HELLO"), "text.upper failed");
    assert.ok(builtinsResult.includes("ListSize: 4"), "list.size failed");
    assert.ok(builtinsResult.includes("Nspace: 100"), "Namespace property access failed");

    // 6. Asynchronous / Promise behaviour
    // 6a. render returns a Promise
    const pending = render("Hi <% name %>", context);
    assert.ok(pending instanceof Promise, "render must return a Promise");
    assert.strictEqual(await pending, "Hi World");

    // 6b. async JS function is treated as a function returning a Swan promise
    assert.strictEqual(
        await render("Async: <% asyncDouble(a) %>", context),
        "Async: 20",
        "Async JS function result should be awaited during render");

    // 6c. Delayed async function
    context.set('slowTriple', async (x) => {
        await new Promise((r) => setTimeout(r, 20));
        return 3 * x;
    });
    assert.strictEqual(
        await render("Slow: <% slowTriple(a) %>", context),
        "Slow: 30",
        "Delayed async function should resolve before rendering");

    // 6d. JS Promise passed as a context value is awaited
    context.set('later', new Promise((r) => setTimeout(() => r(42), 10)));
    assert.strictEqual(
        await render("Later: <% later %>", context),
        "Later: 42",
        "JS Promise value should be mapped to Swan promise and awaited");

    // 6e. Rejected promises and throwing async functions resolve to Nothing (empty)
    context.set('bad', Promise.reject(new Error("boom")));
    assert.strictEqual(await render("[<% bad %>]", context), "[]",
        "Rejected promise should render as Nothing");
    context.set('badFn', async () => { throw new Error("boom"); });
    assert.strictEqual(await render("[<% badFn(1) %>]", context), "[]",
        "Rejecting async function should render as Nothing");

    // 6f. Swan promise mapped back to a JS Promise
    context.set('p', Promise.resolve(7));
    const got = context.get('p');
    assert.ok(got instanceof Promise, "context.get of a Swan promise should return a JS Promise");
    assert.strictEqual(await got, 7, "Mapped JS Promise should resolve to the original value");

    // 6g. Promise produced by Swan evaluation is retrievable as a JS Promise
    await render("<% r = asyncDouble(a) %>", context);
    const r = context.get('r');
    assert.ok(r instanceof Promise, "Promise bound in template should be a JS Promise");
    assert.strictEqual(await r, 20, "Bound promise should resolve to 20");

    console.log("All texl-wasm tests passed successfully!");
}

testTexlWasm().catch((err) => { console.error(err); process.exit(1); });
