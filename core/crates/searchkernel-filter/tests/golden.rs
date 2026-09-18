use searchkernel_filter::{
    parse_filter, CompareOp, ComplexityLimit, ErrorKind, Expr, InValues, ParseError, Predicate,
    Span, Spanned, SyntaxReason, ValueExpr,
};
use serde_json::{json, Value};

const CORPUS: &str = include_str!("fixtures/filter-parser-validation.json");

#[test]
fn frozen_validation_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("validation corpus must be JSON");
    let cases = corpus["cases"].as_array().expect("cases must be an array");
    for case in cases {
        let id = case["id"].as_str().expect("case id must be a string");
        let source = case_source(case).unwrap_or_else(|message| panic!("{id}: {message}"));
        let expected = &case["expected"];
        match parse_filter(&source) {
            Ok(ast) => {
                assert_eq!(
                    expected["ok"],
                    Value::Bool(true),
                    "{id}: expected error, got AST for {source:?}"
                );
                assert_eq!(
                    render_expr(&ast),
                    expected["ast"],
                    "{id}: AST mismatch for {source:?}"
                );
            }
            Err(error) => {
                assert_eq!(
                    expected["ok"],
                    Value::Bool(false),
                    "{id}: expected AST, got {error}"
                );
                assert_error(
                    id,
                    &error,
                    expected
                        .get("error")
                        .expect("error case needs error object"),
                );
            }
        }
    }
}

fn case_source(case: &Value) -> Result<String, String> {
    if let Some(source) = case.get("source").and_then(Value::as_str) {
        return Ok(source.to_owned());
    }
    let input = case.get("input").ok_or("case needs source or input")?;
    let generator = input["generator"]
        .as_str()
        .ok_or("generated input needs a string generator")?;
    match generator {
        "repeat" => {
            let text = input["text"].as_str().ok_or("repeat needs text")?;
            let count = input["count"].as_u64().ok_or("repeat needs count")? as usize;
            Ok(text.repeat(count))
        }
        "nested_parentheses" => {
            let depth = input["depth"]
                .as_u64()
                .ok_or("nested_parentheses needs depth")? as usize;
            let predicate = input["predicate"]
                .as_str()
                .ok_or("nested_parentheses needs predicate")?;
            Ok(format!(
                "{}{}{}",
                "(".repeat(depth),
                predicate,
                ")".repeat(depth)
            ))
        }
        "or_chain" => {
            let count = input["count"].as_u64().ok_or("or_chain needs count")? as usize;
            Ok((0..count)
                .map(|index| format!("f{index} = 1"))
                .collect::<Vec<_>>()
                .join(" OR "))
        }
        "in_list" => {
            let count = input["count"].as_u64().ok_or("in_list needs count")? as usize;
            Ok(format!("f IN ({})", vec!["1"; count].join(", ")))
        }
        "parameter_chain" => {
            let count = input["count"]
                .as_u64()
                .ok_or("parameter_chain needs count")? as usize;
            Ok((0..count)
                .map(|index| format!("f{index} = :p{index}"))
                .collect::<Vec<_>>()
                .join(" OR "))
        }
        other => Err(format!("unknown generator {other:?}")),
    }
}

fn assert_error(id: &str, actual: &ParseError, expected: &Value) {
    assert_eq!(
        kind_name(actual.kind),
        expected["kind"].as_str().expect("error kind"),
        "{id}: kind"
    );
    if let Some(span) = expected.get("span") {
        assert_eq!(render_span(actual.span), *span, "{id}: span");
    }
    if let Some(reason) = expected.get("reason") {
        assert_eq!(
            actual.reason.map(reason_name),
            Some(reason.as_str().expect("reason string")),
            "{id}: reason"
        );
    }
    if let Some(limit) = expected.get("limit") {
        assert_eq!(
            actual.limit.map(limit_name),
            Some(limit.as_str().expect("limit string")),
            "{id}: limit"
        );
    }
}

fn render_expr(expression: &Spanned<Expr>) -> Value {
    match &expression.node {
        Expr::Predicate(Predicate::Compare { field, op, value }) => json!({
            "kind": "compare", "span": render_span(expression.span),
            "field": { "name": field.node.0, "span": render_span(field.span) },
            "op": op_name(*op), "value": render_value(value),
        }),
        Expr::Predicate(Predicate::In { field, values }) => json!({
            "kind": "in", "span": render_span(expression.span),
            "field": { "name": field.node.0, "span": render_span(field.span) },
            "values": render_in_values(values),
        }),
        Expr::And(items) => {
            json!({ "kind": "and", "span": render_span(expression.span), "items": items.iter().map(render_expr).collect::<Vec<_>>() })
        }
        Expr::Or(items) => {
            json!({ "kind": "or", "span": render_span(expression.span), "items": items.iter().map(render_expr).collect::<Vec<_>>() })
        }
        Expr::Not(child) => {
            json!({ "kind": "not", "span": render_span(expression.span), "expr": render_expr(child) })
        }
    }
}

fn render_in_values(values: &InValues) -> Value {
    match values {
        InValues::Values(values) => {
            json!({ "kind": "values", "items": values.iter().map(render_value).collect::<Vec<_>>() })
        }
        InValues::Parameter(parameter) => {
            json!({ "kind": "parameter", "name": parameter.node.0, "span": render_span(parameter.span) })
        }
    }
}

fn render_value(value: &Spanned<ValueExpr>) -> Value {
    match &value.node {
        ValueExpr::String(text) => {
            json!({ "kind": "string", "value": text, "span": render_span(value.span) })
        }
        ValueExpr::Number(number) => {
            json!({ "kind": "number", "source": number.source, "span": render_span(value.span) })
        }
        ValueExpr::Boolean(boolean) => {
            json!({ "kind": "boolean", "value": boolean, "span": render_span(value.span) })
        }
        ValueExpr::Parameter(parameter) => {
            json!({ "kind": "parameter", "name": parameter.0, "span": render_span(value.span) })
        }
    }
}

fn render_span(span: Span) -> Value {
    json!([span.start, span.end])
}

fn op_name(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "eq",
        CompareOp::Lt => "lt",
        CompareOp::Lte => "lte",
        CompareOp::Gt => "gt",
        CompareOp::Gte => "gte",
    }
}

fn kind_name(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::SyntaxError => "syntax_error",
        ErrorKind::EmptyIn => "empty_in",
        ErrorKind::ExpressionTooComplex => "expression_too_complex",
    }
}

fn reason_name(reason: SyntaxReason) -> &'static str {
    match reason {
        SyntaxReason::ExpectedExpression => "expected_expression",
        SyntaxReason::InvalidOperator => "invalid_operator",
        SyntaxReason::UnclosedString => "unclosed_string",
        SyntaxReason::ExpectedRightParen => "expected_right_paren",
        SyntaxReason::ExpectedLeftParen => "expected_left_paren",
        SyntaxReason::ExpectedValue => "expected_value",
        SyntaxReason::ExpectedOperator => "expected_operator",
        SyntaxReason::UnexpectedToken => "unexpected_token",
        SyntaxReason::InvalidParameter => "invalid_parameter",
        SyntaxReason::ExpectedInValue => "expected_in_value",
    }
}

fn limit_name(limit: ComplexityLimit) -> &'static str {
    match limit {
        ComplexityLimit::ExpressionBytes => "expression_bytes",
        ComplexityLimit::Tokens => "tokens",
        ComplexityLimit::NestingDepth => "nesting_depth",
        ComplexityLimit::AstNodes => "ast_nodes",
        ComplexityLimit::InValues => "in_values",
        ComplexityLimit::ParameterReferences => "parameter_references",
    }
}

#[test]
fn fixture_has_only_known_constructs() {
    let corpus: Value = serde_json::from_str(CORPUS).unwrap();
    assert_allowed_keys(
        corpus.as_object().unwrap(),
        &[
            "format_version",
            "contract",
            "limits",
            "canonical_ast",
            "generator_contract",
            "cases",
        ],
        "corpus",
    );
    for case in corpus["cases"].as_array().unwrap() {
        assert_allowed_keys(
            case.as_object().unwrap(),
            &["id", "source", "input", "expected"],
            "case",
        );
        assert!(case.get("source").is_some() || case.get("input").is_some());
        if let Some(input) = case.get("input") {
            let generator = input["generator"].as_str().unwrap();
            assert!(matches!(
                generator,
                "repeat" | "nested_parentheses" | "or_chain" | "in_list" | "parameter_chain"
            ));
            let keys: &[&str] = match generator {
                "repeat" => &["generator", "text", "count"],
                "nested_parentheses" => &["generator", "depth", "predicate"],
                "or_chain" | "in_list" | "parameter_chain" => &["generator", "count"],
                _ => unreachable!("unknown generator was rejected above"),
            };
            assert_allowed_keys(input.as_object().unwrap(), keys, "generated input");
        }
        let expected = case["expected"].as_object().unwrap();
        assert_allowed_keys(expected, &["ok", "ast", "error"], "expected result");
        let error = expected.get("error");
        if let Some(error) = error {
            for key in error.as_object().unwrap().keys() {
                assert!(
                    matches!(key.as_str(), "kind" | "span" | "reason" | "limit"),
                    "unknown error field {key}"
                );
            }
        }
    }
}

fn assert_allowed_keys(object: &serde_json::Map<String, Value>, allowed: &[&str], context: &str) {
    for key in object.keys() {
        assert!(
            allowed.contains(&key.as_str()),
            "unknown {context} field {key}"
        );
    }
}
