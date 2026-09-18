use crate::ast::Span;
use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    SyntaxError,
    EmptyIn,
    ExpressionTooComplex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxReason {
    ExpectedExpression,
    InvalidOperator,
    UnclosedString,
    ExpectedRightParen,
    ExpectedLeftParen,
    ExpectedValue,
    ExpectedOperator,
    UnexpectedToken,
    InvalidParameter,
    ExpectedInValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLimit {
    ExpressionBytes,
    Tokens,
    NestingDepth,
    AstNodes,
    InValues,
    ParameterReferences,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ErrorKind,
    pub span: Span,
    pub reason: Option<SyntaxReason>,
    pub limit: Option<ComplexityLimit>,
}

impl ParseError {
    pub(crate) fn syntax(span: Span, reason: SyntaxReason) -> Self {
        Self {
            kind: ErrorKind::SyntaxError,
            span,
            reason: Some(reason),
            limit: None,
        }
    }

    pub(crate) fn empty_in(span: Span) -> Self {
        Self {
            kind: ErrorKind::EmptyIn,
            span,
            reason: None,
            limit: None,
        }
    }

    pub(crate) fn too_complex(span: Span, limit: ComplexityLimit) -> Self {
        Self {
            kind: ErrorKind::ExpressionTooComplex,
            span,
            reason: None,
            limit: Some(limit),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind, self.reason, self.limit) {
            (ErrorKind::SyntaxError, Some(reason), _) => write!(
                formatter,
                "syntax error ({reason:?}) at {}..{}",
                self.span.start, self.span.end
            ),
            (ErrorKind::EmptyIn, _, _) => write!(
                formatter,
                "IN list cannot be empty at {}..{}",
                self.span.start, self.span.end
            ),
            (ErrorKind::ExpressionTooComplex, _, Some(limit)) => write!(
                formatter,
                "expression exceeds {limit:?} limit at {}..{}",
                self.span.start, self.span.end
            ),
            _ => write!(
                formatter,
                "filter parse error at {}..{}",
                self.span.start, self.span.end
            ),
        }
    }
}

impl std::error::Error for ParseError {}
