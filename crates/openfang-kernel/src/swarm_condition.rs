//! Condition expression evaluator for swarm workflow steps.
//!
//! ## Supported Syntax (by precedence, low to high):
//! 1. `||` - logical or
//! 2. `&&` - logical and
//! 3. `!`  - logical not
//! 4. `>`, `<`, `>=`, `<=`, `==`, `!=` - comparison
//! 5. `has(path)` - existence check
//! 6. `input.key`, `steps.step_id.key` - value reference
//! 7. Literals: numbers, strings (single/double quotes), `true`/`false`
//! 8. `(expr)` - grouping

use std::collections::HashMap;

use serde_json::Value;

/// Condition evaluation context providing access to input and step outputs.
pub struct ConditionContext<'a> {
    /// Workflow/swarm input parameters.
    pub input: &'a HashMap<String, Value>,
    /// Outputs from completed steps, keyed by step_id.
    pub step_outputs: &'a HashMap<String, Value>,
}

/// Condition expression evaluator.
pub struct ConditionEvaluator;

/// Token type for lexical analysis.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    String(String),
    Bool(bool),
    Ident(String),
    Op(String),
    LParen,
    RParen,
    Comma,
}

/// Tokenizer: converts expression string to token stream.
fn tokenize(expr: &str) -> Result<Vec<(Token, usize)>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let pos = i;

        match c {
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            '(' => {
                tokens.push((Token::LParen, pos));
                i += 1;
            }
            ')' => {
                tokens.push((Token::RParen, pos));
                i += 1;
            }
            ',' => {
                tokens.push((Token::Comma, pos));
                i += 1;
            }
            '\'' | '"' => {
                let quote = c;
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err(format!("Unterminated string at position {}", pos));
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push((Token::String(s), pos));
                i += 1;
            }
            '0'..='9' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                match num_str.parse::<f64>() {
                    Ok(n) => tokens.push((Token::Number(n), pos)),
                    Err(_) => return Err(format!("Invalid number '{}' at position {}", num_str, pos)),
                }
            }
            '!' | '<' | '>' | '=' => {
                // Handle multi-char operators: !=, ==, <=, >=, <, >, !
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push((Token::Op(format!("{}=", chars[i])), pos));
                    i += 2;
                } else {
                    tokens.push((Token::Op(c.to_string()), pos));
                    i += 1;
                }
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    tokens.push((Token::Op("&&".to_string()), pos));
                    i += 2;
                } else {
                    return Err(format!("Expected '&&' at position {}", pos));
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    tokens.push((Token::Op("||".to_string()), pos));
                    i += 2;
                } else {
                    return Err(format!("Expected '||' at position {}", pos));
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                match ident.as_str() {
                    "true" => tokens.push((Token::Bool(true), pos)),
                    "false" => tokens.push((Token::Bool(false), pos)),
                    _ => tokens.push((Token::Ident(ident), pos)),
                }
            }
            _ => return Err(format!("Unexpected character '{}' at position {}", c, pos)),
        }
    }

    Ok(tokens)
}

/// Parser state.
struct Parser {
    tokens: Vec<(Token, usize)>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<(Token, usize)>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&(Token, usize)> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<(Token, usize)> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn error_at(&self, msg: &str) -> String {
        match self.peek() {
            Some((_, pos)) => format!("{} at position {}", msg, pos),
            None => format!("{} at end of expression", msg),
        }
    }

    /// Parse or expression (lowest precedence).
    fn parse_or(&mut self, ctx: &ConditionContext) -> Result<Value, String> {
        let mut left = self.parse_and(ctx)?;

        while let Some((Token::Op(op), _)) = self.peek() {
            if op != "||" {
                break;
            }
            self.advance();
            let right = self.parse_and(ctx)?;
            let lv = left.as_bool().unwrap_or(false);
            let rv = right.as_bool().unwrap_or(false);
            left = Value::Bool(lv || rv);
        }

        Ok(left)
    }

    /// Parse and expression.
    fn parse_and(&mut self, ctx: &ConditionContext) -> Result<Value, String> {
        let mut left = self.parse_not(ctx)?;

        while let Some((Token::Op(op), _)) = self.peek() {
            if op != "&&" {
                break;
            }
            self.advance();
            let right = self.parse_not(ctx)?;
            let lv = left.as_bool().unwrap_or(false);
            let rv = right.as_bool().unwrap_or(false);
            left = Value::Bool(lv && rv);
        }

        Ok(left)
    }

    /// Parse not expression.
    fn parse_not(&mut self, ctx: &ConditionContext) -> Result<Value, String> {
        if let Some((Token::Op(op), _)) = self.peek() {
            if op == "!" {
                self.advance();
                let val = self.parse_not(ctx)?;
                let b = val.as_bool().unwrap_or(false);
                return Ok(Value::Bool(!b));
            }
        }
        self.parse_comparison(ctx)
    }

    /// Parse comparison expression.
    fn parse_comparison(&mut self, ctx: &ConditionContext) -> Result<Value, String> {
        let left = self.parse_primary(ctx)?;

        if let Some((Token::Op(op), _)) = self.peek().cloned() {
            if ["==", "!=", ">", "<", ">=", "<="].contains(&op.as_str()) {
                self.advance();
                let right = self.parse_primary(ctx)?;
                return compare(&left, &op, &right);
            }
        }

        Ok(left)
    }

    /// Parse primary: literal, has(), value reference, or parenthesized expr.
    fn parse_primary(&mut self, ctx: &ConditionContext) -> Result<Value, String> {
        match self.peek().cloned() {
            Some((Token::Number(n), _)) => {
                self.advance();
                Ok(Value::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0))))
            }
            Some((Token::String(s), _)) => {
                self.advance();
                Ok(Value::String(s))
            }
            Some((Token::Bool(b), _)) => {
                self.advance();
                Ok(Value::Bool(b))
            }
            Some((Token::LParen, _)) => {
                self.advance();
                let val = self.parse_or(ctx)?;
                match self.advance() {
                    Some((Token::RParen, _)) => Ok(val),
                    Some((_, pos)) => Err(format!("Expected ')' at position {}", pos)),
                    None => Err("Expected ')' at end of expression".to_string()),
                }
            }
            Some((Token::Ident(name), pos)) => {
                self.advance();

                // Check for function call: has(path)
                if name == "has" {
                    match self.advance() {
                        Some((Token::LParen, _)) => {}
                        _ => return Err(format!("Expected '(' after 'has' at position {}", pos)),
                    }
                    let arg = match self.advance() {
                        Some((Token::String(s), _)) => s,
                        Some((Token::Ident(s), _)) => s,
                        Some((_, p)) => return Err(format!("Expected string argument for has() at position {}", p)),
                        None => return Err("Expected argument for has()".to_string()),
                    };
                    match self.advance() {
                        Some((Token::RParen, _)) => {}
                        Some((_, p)) => return Err(format!("Expected ')' at position {}", p)),
                        None => return Err("Expected ')' after has() argument".to_string()),
                    }
                    return Ok(Value::Bool(resolve_path_exists(&arg, ctx)));
                }

                // Value reference: input.key or steps.step_id.key
                resolve_value(&name, ctx, pos)
            }
            _ => Err(self.error_at("Expected expression")),
        }
    }
}

/// Compare two values with the given operator.
fn compare(left: &Value, op: &str, right: &Value) -> Result<Value, String> {
    let result = match (left, right) {
        (Value::Number(a), Value::Number(b)) => {
            let a = a.as_f64().unwrap_or(0.0);
            let b = b.as_f64().unwrap_or(0.0);
            match op {
                "==" => a == b,
                "!=" => a != b,
                ">" => a > b,
                "<" => a < b,
                ">=" => a >= b,
                "<=" => a <= b,
                _ => return Err(format!("Unknown operator: {}", op)),
            }
        }
        (Value::String(a), Value::String(b)) => {
            match op {
                "==" => a == b,
                "!=" => a != b,
                ">" => a > b,
                "<" => a < b,
                ">=" => a >= b,
                "<=" => a <= b,
                _ => return Err(format!("Unknown operator: {}", op)),
            }
        }
        (Value::Bool(a), Value::Bool(b)) => {
            match op {
                "==" => a == b,
                "!=" => a != b,
                _ => return Err(format!("Cannot use '{}' on boolean values", op)),
            }
        }
        _ => {
            match op {
                "==" => left == right,
                "!=" => left != right,
                _ => return Err(format!("Type mismatch: cannot compare {:?} {} {:?}", left, op, right)),
            }
        }
    };
    Ok(Value::Bool(result))
}

/// Check if a path exists in the context.
fn resolve_path_exists(path: &str, ctx: &ConditionContext) -> bool {
    resolve_value(path, ctx, 0).is_ok()
}

/// Resolve a value path like "input.key" or "steps.step_id.key".
fn resolve_value(path: &str, ctx: &ConditionContext, pos: usize) -> Result<Value, String> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return Err(format!("Empty path at position {}", pos));
    }

    match parts[0] {
        "input" => {
            if parts.len() < 2 {
                return Err(format!("Expected key after 'input.' at position {}", pos));
            }
            ctx.input
                .get(parts[1])
                .cloned()
                .ok_or_else(|| format!("Input key '{}' not found at position {}", parts[1], pos))
        }
        "steps" => {
            if parts.len() < 3 {
                return Err(format!("Expected 'steps.step_id.key' at position {}", pos));
            }
            let step_output = ctx
                .step_outputs
                .get(parts[1])
                .ok_or_else(|| format!("Step '{}' output not found at position {}", parts[1], pos))?;
            get_nested_value(step_output, &parts[2..], pos)
        }
        _ => Err(format!("Unknown path prefix '{}' at position {}", parts[0], pos)),
    }
}

/// Get a nested value from a JSON value.
fn get_nested_value(value: &Value, path_parts: &[&str], pos: usize) -> Result<Value, String> {
    let mut current = value;
    for part in path_parts {
        match current {
            Value::Object(map) => {
                current = map
                    .get(*part)
                    .ok_or_else(|| format!("Key '{}' not found at position {}", part, pos))?;
            }
            _ => return Err(format!("Cannot access key '{}' on non-object at position {}", part, pos)),
        }
    }
    Ok(current.clone())
}

impl ConditionEvaluator {
    /// Evaluate a condition expression string, returning a boolean result.
    pub fn evaluate(expr: &str, ctx: &ConditionContext) -> Result<bool, String> {
        let tokens = tokenize(expr)?;
        let mut parser = Parser::new(tokens);
        let result = parser.parse_or(ctx)?;
        result
            .as_bool()
            .ok_or_else(|| "Expression did not evaluate to a boolean".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> ConditionContext<'static> {
        let mut input = HashMap::new();
        input.insert("count".to_string(), Value::Number(serde_json::Number::from(42)));
        input.insert("name".to_string(), Value::String("test".to_string()));
        input.insert("flag".to_string(), Value::Bool(true));

        let mut step_outputs = HashMap::new();
        let mut step1 = serde_json::Map::new();
        step1.insert("result".to_string(), Value::String("success".to_string()));
        step_outputs.insert("step1".to_string(), Value::Object(step1));

        // Use static storage for test
        static INPUT: std::sync::OnceLock<HashMap<String, Value>> = std::sync::OnceLock::new();
        static STEPS: std::sync::OnceLock<HashMap<String, Value>> = std::sync::OnceLock::new();

        ConditionContext {
            input: INPUT.get_or_init(|| input),
            step_outputs: STEPS.get_or_init(|| step_outputs),
        }
    }

    #[test]
    fn test_literal_values() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("true", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("false", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("42 > 10", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("'hello' == 'hello'", &ctx).unwrap());
    }

    #[test]
    fn test_logical_ops() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("true && true", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("true && false", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("false || true", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("!false", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("!true", &ctx).unwrap());
    }

    #[test]
    fn test_value_reference() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("input.count == 42", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.name == 'test'", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.flag", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("steps.step1.result == 'success'", &ctx).unwrap());
    }

    #[test]
    fn test_has_function() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("has(input.count)", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("has(input.missing)", &ctx).unwrap());
    }

    #[test]
    fn test_parentheses() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("(true || false) && true", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("true || (false && false)", &ctx).unwrap());
    }

    #[test]
    fn test_nested_parentheses() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("((true))", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("((true || false) && (true && true))", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("((true && false) || (false && true))", &ctx).unwrap());
    }

    #[test]
    fn test_complex_nested_logic() {
        let ctx = make_ctx();

        // Complex nested expressions
        assert!(ConditionEvaluator::evaluate("(input.count > 10) && (input.count < 100)", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("(input.count == 42) || (input.count == 0)", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("!(input.count == 0) && input.flag", &ctx).unwrap());
    }

    #[test]
    fn test_has_function_with_steps() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("has(steps.step1.result)", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("has(steps.step1.nonexistent)", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("has(steps.nonexistent.field)", &ctx).unwrap());
    }

    #[test]
    fn test_comparison_operators() {
        let ctx = make_ctx();

        // Test all comparison operators
        assert!(ConditionEvaluator::evaluate("input.count == 42", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.count != 0", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.count > 10", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.count < 100", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.count >= 42", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.count <= 42", &ctx).unwrap());
    }

    #[test]
    fn test_string_comparison() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("input.name > 'aaa'", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.name < 'zzz'", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("'hello' != 'world'", &ctx).unwrap());
    }

    #[test]
    fn test_operator_precedence() {
        let ctx = make_ctx();

        // && has higher precedence than ||
        assert!(ConditionEvaluator::evaluate("true || false && false", &ctx).unwrap());
        assert!(!ConditionEvaluator::evaluate("false && true || false", &ctx).unwrap());
    }

    #[test]
    fn test_error_cases() {
        let ctx = make_ctx();

        // Invalid expressions should return errors
        assert!(ConditionEvaluator::evaluate("input.nonexistent", &ctx).is_err());
        assert!(ConditionEvaluator::evaluate("", &ctx).is_err());
        assert!(ConditionEvaluator::evaluate("(", &ctx).is_err());
        assert!(ConditionEvaluator::evaluate(")", &ctx).is_err());
        assert!(ConditionEvaluator::evaluate("true &&", &ctx).is_err());
    }

    #[test]
    fn test_double_quotes() {
        let ctx = make_ctx();

        assert!(ConditionEvaluator::evaluate("\"hello\" == 'hello'", &ctx).unwrap());
        assert!(ConditionEvaluator::evaluate("input.name == \"test\"", &ctx).unwrap());
    }
}
