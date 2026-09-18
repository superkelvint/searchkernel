//! Recursive-descent parser for the syntax-only filter language.

use crate::ast::{
    CompareOp, Expr, FieldName, InValues, NumericLiteral, ParameterName, Predicate, Span, Spanned,
    ValueExpr,
};
use crate::error::{ComplexityLimit, ParseError, SyntaxReason};
use crate::lexer::{lex, Token, TokenKind};
use crate::limits::{MAX_AST_NODES, MAX_EXPRESSION_BYTES, MAX_IN_VALUES, MAX_NESTING_DEPTH};

pub(crate) fn parse(source: &str) -> Result<Spanned<Expr>, ParseError> {
    if source.len() > MAX_EXPRESSION_BYTES {
        return Err(ParseError::too_complex(
            Span {
                start: 0,
                end: source.len(),
            },
            ComplexityLimit::ExpressionBytes,
        ));
    }
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens,
        position: 0,
        nesting_depth: 0,
        ast_nodes: 0,
    };
    let expression = parser.expression()?;
    if !matches!(parser.current().kind, TokenKind::End) {
        return Err(ParseError::syntax(
            parser.current().span,
            SyntaxReason::UnexpectedToken,
        ));
    }
    Ok(expression)
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    nesting_depth: usize,
    ast_nodes: usize,
}

impl Parser {
    fn expression(&mut self) -> Result<Spanned<Expr>, ParseError> {
        self.or_expression()
    }

    fn or_expression(&mut self) -> Result<Spanned<Expr>, ParseError> {
        let first = self.and_expression()?;
        let mut items = vec![first];
        while matches!(self.current().kind, TokenKind::Or) {
            self.advance();
            items.push(self.and_expression()?);
        }
        self.collect_logical(items, false)
    }

    fn and_expression(&mut self) -> Result<Spanned<Expr>, ParseError> {
        let first = self.not_expression()?;
        let mut items = vec![first];
        while matches!(self.current().kind, TokenKind::And) {
            self.advance();
            items.push(self.not_expression()?);
        }
        self.collect_logical(items, true)
    }

    fn collect_logical(
        &mut self,
        mut items: Vec<Spanned<Expr>>,
        is_and: bool,
    ) -> Result<Spanned<Expr>, ParseError> {
        if items.len() == 1 {
            return Ok(items.pop().expect("one item was checked"));
        }
        let span = Span {
            start: items[0].span.start,
            end: items.last().expect("nonempty").span.end,
        };
        self.add_ast_node(span)?;
        let node = if is_and {
            Expr::And(items)
        } else {
            Expr::Or(items)
        };
        Ok(Spanned { node, span })
    }

    fn not_expression(&mut self) -> Result<Spanned<Expr>, ParseError> {
        if !matches!(self.current().kind, TokenKind::Not) {
            return self.primary();
        }
        let not = self.advance();
        self.enter_nesting(not.span)?;
        let child = self.not_expression();
        self.nesting_depth -= 1;
        let child = child?;
        let span = Span {
            start: not.span.start,
            end: child.span.end,
        };
        self.add_ast_node(span)?;
        Ok(Spanned {
            node: Expr::Not(Box::new(child)),
            span,
        })
    }

    fn primary(&mut self) -> Result<Spanned<Expr>, ParseError> {
        if matches!(self.current().kind, TokenKind::LeftParen) {
            let opening = self.advance();
            self.enter_nesting(opening.span)?;
            let expression = self.expression();
            self.nesting_depth -= 1;
            let mut expression = expression?;
            if !matches!(self.current().kind, TokenKind::RightParen) {
                return Err(ParseError::syntax(
                    self.current().span,
                    SyntaxReason::ExpectedRightParen,
                ));
            }
            let closing = self.advance();
            expression.span = Span {
                start: opening.span.start,
                end: closing.span.end,
            };
            return Ok(expression);
        }
        match self.current().kind {
            TokenKind::End
            | TokenKind::And
            | TokenKind::Or
            | TokenKind::RightParen
            | TokenKind::Not => Err(ParseError::syntax(
                self.current().span,
                SyntaxReason::ExpectedExpression,
            )),
            _ => self.predicate(),
        }
    }

    fn predicate(&mut self) -> Result<Spanned<Expr>, ParseError> {
        let field_token = self.current().clone();
        let TokenKind::Identifier(name) = field_token.kind else {
            return Err(ParseError::syntax(
                field_token.span,
                SyntaxReason::ExpectedExpression,
            ));
        };
        self.advance();
        let field = Spanned {
            node: FieldName(name),
            span: field_token.span,
        };
        match self.current().kind {
            TokenKind::Eq | TokenKind::Lt | TokenKind::Lte | TokenKind::Gt | TokenKind::Gte => {
                self.comparison(field)
            }
            TokenKind::In => self.in_predicate(field),
            TokenKind::InvalidOperator => Err(ParseError::syntax(
                self.current().span,
                SyntaxReason::InvalidOperator,
            )),
            _ => Err(ParseError::syntax(
                self.current().span,
                SyntaxReason::ExpectedOperator,
            )),
        }
    }

    fn comparison(&mut self, field: Spanned<FieldName>) -> Result<Spanned<Expr>, ParseError> {
        let operator = self.advance();
        let op = match operator.kind {
            TokenKind::Eq => CompareOp::Eq,
            TokenKind::Lt => CompareOp::Lt,
            TokenKind::Lte => CompareOp::Lte,
            TokenKind::Gt => CompareOp::Gt,
            TokenKind::Gte => CompareOp::Gte,
            _ => unreachable!("comparison caller only passes comparison tokens"),
        };
        let value = self.value(SyntaxReason::ExpectedValue)?;
        let span = Span {
            start: field.span.start,
            end: value.span.end,
        };
        self.add_ast_node(span)?;
        Ok(Spanned {
            node: Expr::Predicate(Predicate::Compare { field, op, value }),
            span,
        })
    }

    fn in_predicate(&mut self, field: Spanned<FieldName>) -> Result<Spanned<Expr>, ParseError> {
        self.advance();
        if !matches!(self.current().kind, TokenKind::LeftParen) {
            return Err(ParseError::syntax(
                self.current().span,
                SyntaxReason::ExpectedLeftParen,
            ));
        }
        let opening = self.advance();
        if matches!(self.current().kind, TokenKind::RightParen) {
            let closing = self.advance();
            return Err(ParseError::empty_in(Span {
                start: opening.span.start,
                end: closing.span.end,
            }));
        }
        let first = self.value(SyntaxReason::ExpectedInValue)?;
        if let ValueExpr::Parameter(name) = &first.node {
            if matches!(self.current().kind, TokenKind::RightParen) {
                let closing = self.advance();
                let parameter = Spanned {
                    node: name.clone(),
                    span: first.span,
                };
                return self.finish_in(field, InValues::Parameter(parameter), closing.span.end);
            }
        }
        let mut values = vec![first];
        while matches!(self.current().kind, TokenKind::Comma) {
            self.advance();
            values.push(self.value(SyntaxReason::ExpectedInValue)?);
            if values.len() > MAX_IN_VALUES {
                return Err(ParseError::too_complex(
                    values.last().expect("just pushed").span,
                    ComplexityLimit::InValues,
                ));
            }
        }
        if !matches!(self.current().kind, TokenKind::RightParen) {
            return Err(ParseError::syntax(
                self.current().span,
                SyntaxReason::UnexpectedToken,
            ));
        }
        let closing = self.advance();
        self.finish_in(field, InValues::Values(values), closing.span.end)
    }

    fn finish_in(
        &mut self,
        field: Spanned<FieldName>,
        values: InValues,
        end: usize,
    ) -> Result<Spanned<Expr>, ParseError> {
        let span = Span {
            start: field.span.start,
            end,
        };
        self.add_ast_node(span)?;
        Ok(Spanned {
            node: Expr::Predicate(Predicate::In { field, values }),
            span,
        })
    }

    fn value(&mut self, missing_reason: SyntaxReason) -> Result<Spanned<ValueExpr>, ParseError> {
        let token = self.current().clone();
        let node = match token.kind {
            TokenKind::StringLiteral(value) => ValueExpr::String(value),
            TokenKind::NumberLiteral(source) => ValueExpr::Number(NumericLiteral { source }),
            TokenKind::True => ValueExpr::Boolean(true),
            TokenKind::False => ValueExpr::Boolean(false),
            TokenKind::Parameter(name) => ValueExpr::Parameter(ParameterName(name)),
            TokenKind::InvalidParameter => {
                return Err(ParseError::syntax(
                    token.span,
                    SyntaxReason::InvalidParameter,
                ))
            }
            _ => return Err(ParseError::syntax(token.span, missing_reason)),
        };
        self.advance();
        Ok(Spanned {
            node,
            span: token.span,
        })
    }

    fn enter_nesting(&mut self, span: Span) -> Result<(), ParseError> {
        self.nesting_depth += 1;
        if self.nesting_depth > MAX_NESTING_DEPTH {
            self.nesting_depth -= 1;
            return Err(ParseError::too_complex(span, ComplexityLimit::NestingDepth));
        }
        Ok(())
    }

    fn add_ast_node(&mut self, span: Span) -> Result<(), ParseError> {
        self.ast_nodes += 1;
        if self.ast_nodes > MAX_AST_NODES {
            return Err(ParseError::too_complex(span, ComplexityLimit::AstNodes));
        }
        Ok(())
    }

    fn current(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn advance(&mut self) -> Token {
        let token = self.tokens[self.position].clone();
        self.position += 1;
        token
    }
}
