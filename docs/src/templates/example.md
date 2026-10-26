# A complete example

This example renders an invoice in Markdown. It uses most of what the previous
chapters describe: data in a first tag, a library, a partial, repetition and a
condition.

## The files

```text
invoice/
├── invoice.texl
├── lib/
│   └── money.texl
└── partials/
    └── letterhead.texl
```

### invoice.texl

The main document. Its first tag binds the data; in a real application the
host would pass it as [globals](names.md#data-from-the-host). The next two
tags import the library and the letterhead.

```texl,nocheck
<%
# The data of the invoice. A real application would pass it as globals.
number = "2026-042",
date = time.from_string "2026-10-16",
customer = {name: "Ada Lovelace", city: "London"},
items = [
    {what: "Analytical Engine manual", qty: 1, price: 120},
    {what: "Punched cards (box)", qty: 3, price: 15.5},
    {what: "Bernoulli tables", qty: 2, price: 40}
],
nl = "
"
%><% texl.import("lib/money.texl") ~> (lib -> bind "money" lib.data)
%><% texl.import("partials/letterhead.texl") ~> (doc -> doc.rendering) %>
# Invoice <% number %>

Date: <% text.head 10 (time.to_string date) %>
Bill to: <% customer.name %>, <% customer.city %>

<%
amount = item -> item.qty * item.price,
row = item -> "| " + item.what + " | " + text.from item.qty + " | "
    + money.format item.price + " | " + money.format (amount item) + " |"
%>| Item | Qty | Price | Amount |
|------|----:|------:|-------:|
<% text.join nl (list.enum items => row) %>

**Total: <% money.format (money.sum [list.enum items => amount]) %>**

<% list.size items > 2 ? "Thank you for being a loyal customer!" ; "Thank you!" %>
```

### lib/money.texl

A [library](documents.md#using-a-library): it binds two functions and renders
nothing. `sum` is recursive.

```texl,nocheck
<%
# Formats an amount with two decimals: 15.5 -> "$15.50"
format = n -> (
    cents = math.round (n * 100),
    rest = cents % 100,
    "$" + text.from ((cents - rest) / 100) + "." + (rest < 10 ? "0" ; "") + text.from rest
),

# Adds up the numbers in a list
sum = numbers -> list.size numbers == 0 ? 0 ; numbers 0 + sum (list.tail 1 numbers)
%>
```

### partials/letterhead.texl

A [partial](documents.md#inserting-a-partial): plain text to insert at the
top.

```texl,nocheck
**Babbage & Co.** · 12 Dorset Street, London
```

## Rendering it

With the [Python library](../hosts/python.md):

```python
import asyncio
from pathlib import Path
import pytexl

doc = asyncio.run(pytexl.Engine().load(Path("invoice/invoice.texl")))
Path("invoice.md").write_text(doc.rendering)
```

`invoice.md` contains:

```text
**Babbage & Co.** · 12 Dorset Street, London

# Invoice 2026-042

Date: 2026-10-16
Bill to: Ada Lovelace, London

| Item | Qty | Price | Amount |
|------|----:|------:|-------:|
| Analytical Engine manual | 1 | $120.00 | $120.00 |
| Punched cards (box) | 3 | $15.50 | $46.50 |
| Bernoulli tables | 2 | $40.00 | $80.00 |

**Total: $246.50**

Thank you for being a loyal customer!
```

## How it works

- **The data tag** binds `number`, `date`, `customer`, `items` and `nl`, a
  line break used to join the rows. Each `%>` is placed right before the next
  text, so that the tags leave no empty lines (see
  [Line breaks around tags](tags.md#line-breaks-around-tags)).
- **The library** is imported with `texl.import`, and its `data` is bound to
  the global name `money` with `bind`. Rendering waits for the import, so the
  later tags can call `money.format` and `money.sum`.
- **The letterhead** is inserted with its `rendering`. It ends with a line
  break, which separates it from the title.
- **The table rows** are produced by mapping `row` over the items and joining
  the results with line breaks. See
  [Repeating text for each item](logic.md#repeating-text-for-each-item).
- **The total** maps `amount` over the items, puts the results in a list, and
  adds them up with `money.sum`.
- **The closing line** depends on a condition.

The imports use paths relative to `invoice.texl`, so they resolve the same
way whatever the current folder is.
