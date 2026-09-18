//! Syntax-only abstract syntax tree for filter expressions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Predicate(Predicate),
    And(Vec<Spanned<Expr>>),
    Or(Vec<Spanned<Expr>>),
    Not(Box<Spanned<Expr>>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    Compare {
        field: Spanned<FieldName>,
        op: CompareOp,
        value: Spanned<ValueExpr>,
    },
    In {
        field: Spanned<FieldName>,
        values: InValues,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InValues {
    Values(Vec<Spanned<ValueExpr>>),
    Parameter(Spanned<ParameterName>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueExpr {
    String(String),
    Number(NumericLiteral),
    Boolean(bool),
    Parameter(ParameterName),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericLiteral {
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldName(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterName(pub String);
