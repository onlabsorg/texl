use swan_core::evaluator::eval;
use swan_core::lexer::Lexer;
use swan_core::parser::Parser;
use swan_core::value::{Scope, Value};



/// Evaluates all Swan expressions enclosed between `<%` and `%>` in `template`,
/// using the given `scope`, and returns the interpolated output string.
pub fn render(template: &str, scope: &Scope) -> String {
    let mut result = String::with_capacity(template.len());
    let mut cursor = 0;

    while let Some(start_idx) = template[cursor..].find("<%") {
        let tag_start = cursor + start_idx;
        let expr_start = tag_start + 2; // skip "<%"

        if let Some(end_idx) = template[expr_start..].find("%>") {
            let expr_end = expr_start + end_idx;
            let tag_end = expr_end + 2; // skip "%>"

            // Append literal text prior to the opening tag
            result.push_str(&template[cursor..tag_start]);

            // Extract expression snippet between delimiters
            let expr_src = &template[expr_start..expr_end];

            // Parse and evaluate expression
            let mut parser = Parser::new(Lexer::new(expr_src));
            let ast = parser.parse();
            let evaluated_val = eval(&ast, scope);

            // Convert evaluated Value into its text representation
            result.push_str(&value_to_string(evaluated_val));

            // Move cursor past the closing delimiter tag
            cursor = tag_end;
        } else {
            // Unclosed `<%` delimiter encountered; append remaining text and finish
            break;
        }
    }

    // Append any trailing string fragment after the final tag
    result.push_str(&template[cursor..]);
    result
}


/// Converts a Swan `Value` into its rendered text format.
fn value_to_string(val: Value) -> String {
    match val {
        Value::Text(s) => s.to_string(),
        Value::Numb(n) => n.to_string(),
        Value::Bool(b) => if b { "TRUE".into() } else { "FALSE".into() },
        Value::Nothing => String::new(),
        Value::List(l) => format!("[[List of {}]]", l.len()),
        Value::Namespace(s) => format!("[[Namespace of {}]]", s.0.borrow().bindings.len()),
        Value::Func { .. } | Value::Builtin(_) => "[[Func]]".to_string(),
        Value::Undefined(_) => "[[Undefined]]".to_string(),
        Value::Tuple(items) => {
            let item_strs: Vec<String> = items
                .iter()
                .cloned()
                .map(value_to_string)
                .collect();
            item_strs.join(", ")
        }
        Value::Name(n) => n.to_string(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use swan_core::value::{Scope, Value};

    /// Helper to create a test scope pre-populated with standard variables and functions.
    fn create_test_scope() -> Scope {
        let scope = Scope::new();
        scope.set("name", Value::Text("World".into()));
        scope.set("a", Value::Numb(10.0));
        scope.set("b", Value::Numb(20.0));
        scope.set("is_valid", Value::Bool(true));
        scope.set("items", Value::list(vec![Value::Numb(1.0), Value::Numb(2.0)]));
        scope.new_child()
    }

    #[test]
    fn test_plain_text_without_expressions() {
        let scope = create_test_scope();
        let input = "Hello, world! No template tags here.";
        assert_eq!(render(input, &scope), "Hello, world! No template tags here.");
    }

    #[test]
    fn test_single_expression_interpolation() {
        let scope = create_test_scope();
        let input = "Hello <% name %>!";
        assert_eq!(render(input, &scope), "Hello World!");
    }

    #[test]
    fn test_multiple_expressions_and_arithmetic() {
        let scope = create_test_scope();
        let input = "Sum: <% a + b %>, Product: <% a * b %>";
        assert_eq!(render(input, &scope), "Sum: 30, Product: 200");
    }

    #[test]
    fn test_boolean_and_nothing_formatting() {
        let scope = create_test_scope();
        let input = "Valid: <% is_valid %>, Empty: <% () %>";
        assert_eq!(render(input, &scope), "Valid: TRUE, Empty: ");
    }

    #[test]
    fn test_structured_types_formatting() {
        let scope = create_test_scope();
        let input = "List: <% items %>, Namespace: <% { x: 1, y: 2 } %>";
        assert_eq!(render(input, &scope), "List: [[List of 2]], Namespace: [[Namespace of 2]]");
    }

    #[test]
    fn test_tuple_formatting() {
        let scope = create_test_scope();
        let input = "Tuple: <% (a, b, name) %>";
        assert_eq!(render(input, &scope), "Tuple: 10, 20, World");
    }

    #[test]
    fn test_unclosed_delimiter_tag() {
        let scope = create_test_scope();
        let input = "Hello <% name %, remaining text without closing tag <% a";
        assert_eq!(
            render(input, &scope),
            "Hello <% name %, remaining text without closing tag <% a"
        );
    }

    #[test]
    fn test_state_mutation_across_tags() {
        let scope = create_test_scope();
        let input = "<% count = 1 %>Count is <% count %>, next is <% count = count + 1, count %>";
        assert_eq!(render(input, &scope), "Count is 1, next is 2");
    }
}
