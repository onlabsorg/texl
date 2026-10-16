# swan-core

The reference implementation of **Swan**, a small, dynamically typed,
expression-oriented language designed to be embedded in template engines and
other host applications. It is the language of the [Texl](../README.md)
template engine.

This crate contains the lexer, the parser, the evaluator, the runtime values
and the built-in library. It has no dependencies.

```rust
use swan_core::builtins::root_scope;
use swan_core::evaluator::eval;
use swan_core::lexer::Lexer;
use swan_core::parser::Parser;
use swan_core::value::Value;

let scope = root_scope();
let ast = Parser::new(Lexer::new("double = x -> x * 2, double 21")).parse();
assert_eq!(eval(&ast, &scope), Value::Numb(42.0));
```

Evaluation never fails and never panics: errors are `Value::Undefined` values.

## Documentation

- [The Swan language manual](https://onlabsorg.github.io/texl/book/swan/index.html)
- [The built-in library](https://onlabsorg.github.io/texl/book/swan/builtins/index.html)
- [API reference](https://onlabsorg.github.io/texl/api/swan_core/index.html)
  (or `cargo doc -p swan-core --open`)

## License

[MIT](../LICENSE)
