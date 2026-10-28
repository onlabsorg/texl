# A report from Python data

This example renders a Markdown report from data that a Python program
passes to the template. It shows globals, a library document, an `async`
host function, and writing the result to a file. You need the `pytexl`
package (see [Getting started](../getting-started.md)).

## The files

```text
render_report.py
templates/
    report.texl
    lib/format.texl
```

`templates/lib/format.texl` is a library: it binds functions and renders
nothing.

```texl
<%
# Formatting helpers. This document renders nothing.
round = n -> math.round (n * 100) / 100,
euro = n -> "€" + text.from (round n),
dollar = n -> "$" + text.from (round n)
%>
```

`templates/report.texl` imports the library, asks the host for an exchange
rate, and writes one table row per region:

```texl
<% texl.import("lib/format.texl") ~> (lib -> bind "fmt" lib.data) %><% rate "USD" ~> (r -> bind "usd" r) %># Sales report, <% month %>

| Region | Sales | In dollars |
|--------|-------|------------|
<% row = r -> "| " + r.name + " | " + fmt.euro r.sales + " | " + fmt.dollar (r.sales * usd) + " |" %><% text.join "
" (list.enum regions => row) %>
```

- The first two tags return promises, so rendering waits for each before it
  goes on; their callbacks bind `fmt` and `usd` with `bind`, so the following
  tags can use them.
- `month`, `regions` and `rate` are globals that the program passes.
- `list.enum regions => row` applies `row` to every item of the list;
  `text.join` puts the rows on separate lines.

## The program

```python
import asyncio
from pathlib import Path
import pytexl

async def rate(currency):
    await asyncio.sleep(0.1)          # a request to a web service, in a real program
    return {"USD": 1.08, "GBP": 0.85}[currency]

async def main():
    engine = pytexl.Engine({"rate": rate})
    engine.set("month", "October 2026")
    engine.set("regions", [
        {"name": "North", "sales": 1250.5},
        {"name": "South", "sales": 980},
    ])
    doc = await engine.load(Path("templates/report.texl"))
    Path("report.md").write_text(doc.rendering)
    print(doc.rendering)

asyncio.run(main())
```

- `rate` is an `async` function: the template receives a promise, and the
  engine awaits the coroutine on the running asyncio loop.
- The dictionaries become Swan namespaces, the list a Swan list (see
  [value conversion](../api/python.md#globals-and-context-values)).
- `engine.load` resolves the path against the current working directory, and
  the import against the folder of `report.texl`.

## The result

`python render_report.py` writes `report.md`:

```text
# Sales report, October 2026

| Region | Sales | In dollars |
|--------|-------|------------|
| North | €1250.5 | $1350.54 |
| South | €980 | $1058.4 |
```

## Going further

- If `rate` raises an exception, its promise resolves to `Nothing`, and so
  `usd` is `Nothing`: the dollar column shows `$[[Undefined]]` instead of the
  program stopping. Test for it with `usd ; ...` to show a fallback.
- `doc.data` holds the names the report bound (`fmt`, `row`, `usd`, and
  `texl`), converted to Python values; `doc.data["row"]` is a callable
  `pytexl.Function`.
