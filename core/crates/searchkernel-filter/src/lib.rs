//! A standalone, schema-independent parser for Search Kernel filter syntax.
//!
//! This crate deliberately stops at the syntax AST. Schema lookup, type checking,
//! parameter binding, and backend lowering belong to a later integration layer.

mod ast;
mod error;
mod lexer;
pub mod limits;
mod parser;

pub use ast::{
    CompareOp, Expr, FieldName, InValues, NumericLiteral, ParameterName, Predicate, Span, Spanned,
    ValueExpr,
};
pub use error::{ComplexityLimit, ErrorKind, ParseError, SyntaxReason};

/// Parses a complete filter expression into a syntax-only AST.
pub fn parse_filter(source: &str) -> Result<Spanned<Expr>, ParseError> {
    parser::parse(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{
        MAX_AST_NODES, MAX_EXPRESSION_BYTES, MAX_IN_VALUES, MAX_NESTING_DEPTH,
        MAX_PARAMETER_REFERENCES,
    };

    #[test]
    fn logical_chains_are_n_ary_and_left_to_right() {
        let parsed = parse_filter("a = 1 AND b = 2 AND c = 3").unwrap();
        let Expr::And(items) = parsed.node else {
            panic!("expected And");
        };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].span, Span { start: 0, end: 5 });
        assert_eq!(items[2].span, Span { start: 20, end: 25 });
    }

    #[test]
    fn repeated_not_has_checked_syntax() {
        let parsed = parse_filter("NOT NOT a = true").unwrap();
        assert!(matches!(parsed.node, Expr::Not(_)));
    }

    #[test]
    fn parenthesized_expression_widens_only_its_outer_span() {
        let parsed = parse_filter("(a = 1 OR b = 2)").unwrap();
        assert_eq!(parsed.span, Span { start: 0, end: 16 });
        let Expr::Or(items) = parsed.node else {
            panic!("expected Or");
        };
        assert_eq!(items[0].span, Span { start: 1, end: 6 });
    }

    #[test]
    fn malformed_numbers_are_not_silently_consumed() {
        for source in ["a = +1", "a = 1.", "a = 1e", "a = --1"] {
            assert!(parse_filter(source).is_err(), "{source}");
        }
    }

    #[test]
    fn keyword_prefixes_are_identifiers() {
        assert!(parse_filter("android = 1 AND origin = 2 AND inside = 3").is_ok());
    }

    #[test]
    fn arbitrary_utf8_input_never_panics() {
        let corpus = [
            "",
            "☃",
            "\u{2003}",
            "'",
            "😀 = :",
            "x = '\\'",
            "a IN (,,)",
            "\0",
            "日本語",
        ];
        for source in corpus {
            let outcome = std::panic::catch_unwind(|| parse_filter(source));
            assert!(outcome.is_ok(), "parser panicked for {source:?}");
        }
    }

    #[test]
    fn complexity_boundaries_are_enforced() {
        assert!(parse_filter(&" ".repeat(MAX_EXPRESSION_BYTES)).is_err());
        let too_long = parse_filter(&" ".repeat(MAX_EXPRESSION_BYTES + 1)).unwrap_err();
        assert_eq!(too_long.limit, Some(ComplexityLimit::ExpressionBytes));

        let nested = format!(
            "{}a = 1{}",
            "(".repeat(MAX_NESTING_DEPTH),
            ")".repeat(MAX_NESTING_DEPTH)
        );
        assert!(parse_filter(&nested).is_ok());
        let too_nested = format!(
            "{}a = 1{}",
            "(".repeat(MAX_NESTING_DEPTH + 1),
            ")".repeat(MAX_NESTING_DEPTH + 1)
        );
        assert_eq!(
            parse_filter(&too_nested).unwrap_err().limit,
            Some(ComplexityLimit::NestingDepth)
        );
        assert!(parse_filter(&format!("{}a = 1", "NOT ".repeat(MAX_NESTING_DEPTH))).is_ok());
        assert_eq!(
            parse_filter(&format!("{}a = 1", "NOT ".repeat(MAX_NESTING_DEPTH + 1)))
                .unwrap_err()
                .limit,
            Some(ComplexityLimit::NestingDepth)
        );

        let in_at_limit = format!("f IN ({})", vec!["1"; MAX_IN_VALUES].join(","));
        assert!(parse_filter(&in_at_limit).is_ok());
        let too_many_in = format!("f IN ({})", vec!["1"; MAX_IN_VALUES + 1].join(","));
        assert_eq!(
            parse_filter(&too_many_in).unwrap_err().limit,
            Some(ComplexityLimit::InValues)
        );

        let parameter_chain = |count: usize| {
            (0..count)
                .map(|index| format!("f{index}=:p{index}"))
                .collect::<Vec<_>>()
                .join(" OR ")
        };
        assert!(parse_filter(&parameter_chain(MAX_PARAMETER_REFERENCES)).is_ok());
        assert_eq!(
            parse_filter(&parameter_chain(MAX_PARAMETER_REFERENCES + 1))
                .unwrap_err()
                .limit,
            Some(ComplexityLimit::ParameterReferences)
        );

        let or_chain = |count: usize| {
            (0..count)
                .map(|index| format!("f{index}=1"))
                .collect::<Vec<_>>()
                .join(" OR ")
        };
        assert!(parse_filter(&or_chain(MAX_AST_NODES - 1)).is_ok());
        assert_eq!(
            parse_filter(&or_chain(MAX_AST_NODES)).unwrap_err().limit,
            Some(ComplexityLimit::AstNodes)
        );
    }
}
