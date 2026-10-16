# Conditions and repetition

Texl has no special tags for conditions or loops. A tag is an expression, so
you use Swan's [conditional operators](../swan/conditions.md) and
[mapping](../swan/functions.md#mapping) to decide what to write.

## Optional text

`condition ? text` writes `text` when the condition is true, and nothing
otherwise. `condition ? text ; other` writes `other` when it is false:

```texl
<% stock = 0, vip = TRUE %>Status: <% stock > 0 ? "in stock" ; "sold out" %><% vip ? " (priority shipping)" %>
```

```text
Status: sold out (priority shipping)
```

Chain conditions for more cases:

```texl
<% size = n -> n < 10 ? "small" ; n < 100 ? "medium" ; "large" %><% size 5 %>, <% size 50 %>, <% size 500 %>
```

```text
small, medium, large
```

## Optional paragraphs

A text literal can span several lines, so a condition can produce a whole
paragraph:

```texl
<% overdue = TRUE %>Thank you for your order.
<% overdue ? "
Your payment is overdue. Please pay within
seven days to avoid a late fee.
" %>
Best regards
```

```text
Thank you for your order.

Your payment is overdue. Please pay within
seven days to avoid a late fee.

Best regards
```

Remember that Swan text cannot contain `"` and that `%>` ends the tag. For
longer optional parts, or parts with tags of their own, put them in a separate
document and [import](documents.md) it conditionally.

## Repeating text for each item

To write something for each item of a list:

1. turn the list into a tuple with `list.enum`;
2. map a function that makes the text for one item over the tuple, with `=>`;
3. join the results with `text.join`.

```texl
<%
fruits = ["apples", "pears", "plums"],
nl = "
"
%>Shopping list:
<% text.join nl (list.enum fruits => (f -> "- " + f)) %>
```

```text
Shopping list:
- apples
- pears
- plums
```

Binding the line break to a name, like `nl` here, keeps the tags readable.
Without `text.join`, the items would be written separated by commas, the way
tuples are rendered.

The items are often namespaces, and the function picks their members. Numbers
must be converted with `text.from` before they are joined to text:

```texl
<%
nl = "
",
orders = [{id: "A-1", total: 25}, {id: "B-7", total: 140}],
row = o -> "| " + o.id + " | " + text.from o.total + " |"
%>| Order | Total |
|-------|-------|
<% text.join nl (list.enum orders => row) %>
```

```text
| Order | Total |
|-------|-------|
| A-1 | 25 |
| B-7 | 140 |
```

### Empty lists

`list.enum []` is `Nothing`, so an empty list writes nothing. To write a
message instead, test the size:

```texl
<% items = [] %><% list.size items == 0 ? "No items." ; text.join ", " (list.enum items) %>
```

```text
No items.
```

### Filtering

A function that returns `Nothing` for some items drops them, because
`Nothing` disappears from tuples:

```texl
<%
orders = [{id: "A-1", total: 25}, {id: "B-7", total: 140}],
big = o -> o.total > 100 ? o.id
%>Large orders: <% list.enum orders => big %>
```

```text
Large orders: B-7
```

### Totals

Swan has no loop and no built-in sum, but a short recursive function does the
job:

```texl
<%
sum = xs -> list.size xs == 0 ? 0 ; xs 0 + sum (list.tail 1 xs),
totals = [25, 140, 60]
%>Total: <% sum totals %>
```

```text
Total: 225
```

Keep the recursive call on the right of `;`, as here. See
[Evaluation order](../swan/conditions.md#evaluation-order).

## Reusable helpers

Collect the helpers you use often in a first tag, or in a library document
that every template imports (see [Using a library](documents.md#using-a-library)):

```texl
<%
nl = "
",
bullets = items -> text.join nl (list.enum items => (i -> "- " + text.from i)),
plural = (n, word) -> text.from n + " " + word + (n == 1 ? "" ; "s")
%><% plural (3, "file") %> changed:
<% bullets ["README.md", "main.rs", "lib.rs"] %>
```

```text
3 files changed:
- README.md
- main.rs
- lib.rs
```
