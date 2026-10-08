const assert = require('node:assert/strict');
const { create_context, render } = require('../pkg/texl_wasm.js');

function testTexlWasm() {
    console.log("Running texl-wasm test suite...");

    // 1. Context Creation & Variable Setting
    const context = create_context({
        a: 10,
        b: 20,
        name: "World",
        double: (x) => 2 * x
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
    const textRender = render("Hello <% name %>!", context);
    assert.strictEqual(textRender, "Hello World!", "Template tag interpolation failed");

    const mathRender = render("Sum: <% a + b + c %>", context);
    assert.strictEqual(mathRender, "Sum: 60", "Arithmetic expression failed");

    // 3. Invoking JavaScript Callbacks from Swan
    const fnRender = render("Double: <% double(a) %>", context);
    assert.strictEqual(fnRender, "Double: 20", "JS function invocation failed");

    // 4. In-Template Context Assignment & Retrival
    // Assignment (=) and Labelling (:) mutate the context scope
    render("Assign: <% x = a * 5 %>, Label: <% y : b + 10 %>", context);
    assert.strictEqual(context.get('x'), 50, "Variable x assigned in template should be retrievable");
    assert.strictEqual(context.get('y'), 30, "Variable y labelled in template should be retrievable");

    // 5. Swan Core Built-ins (Math, Text, List, Namespace)
    const builtinsTemplate = `
        Sqrt: <% math.sqrt(16) %>
        Upper: <% text.upper("hello") %>
        ListSize: <% list.size([1, 2, 3, 4]) %>
        Nspace: <% { p: 100, q: 200 }.p %>
    `.trim();

    const builtinsResult = render(builtinsTemplate, context);
    assert.ok(builtinsResult.includes("Sqrt: 4"), "math.sqrt failed");
    assert.ok(builtinsResult.includes("Upper: HELLO"), "text.upper failed");
    assert.ok(builtinsResult.includes("ListSize: 4"), "list.size failed");
    assert.ok(builtinsResult.includes("Nspace: 100"), "Namespace property access failed");

    console.log("All texl-wasm tests passed successfully!");
}

testTexlWasm();
