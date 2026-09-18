//! UTF-8 byte-span lexer for the standalone filter grammar.

use crate::ast::Span;
use crate::error::{ComplexityLimit, ParseError, SyntaxReason};
use crate::limits::{MAX_PARAMETER_REFERENCES, MAX_TOKENS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier(String),
    StringLiteral(String),
    NumberLiteral(String),
    Parameter(String),
    Eq,
    Lt,
    Lte,
    Gt,
    Gte,
    And,
    Or,
    Not,
    In,
    True,
    False,
    LeftParen,
    RightParen,
    Comma,
    InvalidOperator,
    InvalidParameter,
    Invalid,
    End,
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token>, ParseError> {
    let mut lexer = Lexer {
        source,
        offset: 0,
        token_count: 0,
        parameter_references: 0,
        tokens: Vec::new(),
    };
    while lexer.offset < source.len() {
        lexer.skip_whitespace();
        if lexer.offset == source.len() {
            break;
        }
        lexer.lex_one()?;
    }
    lexer.tokens.push(Token {
        kind: TokenKind::End,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
    });
    Ok(lexer.tokens)
}

struct Lexer<'a> {
    source: &'a str,
    offset: usize,
    token_count: usize,
    parameter_references: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn skip_whitespace(&mut self) {
        while let Some(character) = self.source[self.offset..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            self.offset += character.len_utf8();
            if self.offset == self.source.len() {
                break;
            }
        }
    }

    fn lex_one(&mut self) -> Result<(), ParseError> {
        let start = self.offset;
        let rest = &self.source[start..];
        let character = rest
            .chars()
            .next()
            .expect("offset is within a UTF-8 string");
        match character {
            '(' => self.single(TokenKind::LeftParen),
            ')' => self.single(TokenKind::RightParen),
            ',' => self.single(TokenKind::Comma),
            '=' => {
                if rest.starts_with("==") {
                    self.consume(TokenKind::InvalidOperator, 2)?;
                } else {
                    self.single(TokenKind::Eq)?;
                }
                Ok(())
            }
            '!' => {
                if rest.starts_with("!=") {
                    self.consume(TokenKind::InvalidOperator, 2)?;
                } else {
                    self.single(TokenKind::Invalid)?;
                }
                Ok(())
            }
            '<' => {
                if rest.starts_with("<=") {
                    self.consume(TokenKind::Lte, 2)?;
                } else {
                    self.single(TokenKind::Lt)?;
                }
                Ok(())
            }
            '>' => {
                if rest.starts_with(">=") {
                    self.consume(TokenKind::Gte, 2)?;
                } else {
                    self.single(TokenKind::Gt)?;
                }
                Ok(())
            }
            '\'' => self.string(),
            ':' => self.parameter(),
            '-' if rest.as_bytes().get(1).is_some_and(u8::is_ascii_digit) => self.number(),
            '0'..='9' => self.number(),
            'A'..='Z' | 'a'..='z' | '_' => self.identifier(),
            _ => {
                self.offset += character.len_utf8();
                self.push(TokenKind::Invalid, start, self.offset)
            }
        }
    }

    fn single(&mut self, kind: TokenKind) -> Result<(), ParseError> {
        let start = self.offset;
        self.offset += 1;
        self.push(kind, start, self.offset)
    }

    fn consume(&mut self, kind: TokenKind, bytes: usize) -> Result<(), ParseError> {
        let start = self.offset;
        self.offset += bytes;
        self.push(kind, start, self.offset)
    }

    fn identifier(&mut self) -> Result<(), ParseError> {
        let start = self.offset;
        self.offset += 1;
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_alphanumeric)
            || self.source.as_bytes().get(self.offset) == Some(&b'_')
        {
            self.offset += 1;
        }
        let word = &self.source[start..self.offset];
        let kind = if word.eq_ignore_ascii_case("and") {
            TokenKind::And
        } else if word.eq_ignore_ascii_case("or") {
            TokenKind::Or
        } else if word.eq_ignore_ascii_case("not") {
            TokenKind::Not
        } else if word.eq_ignore_ascii_case("in") {
            TokenKind::In
        } else if word.eq_ignore_ascii_case("true") {
            TokenKind::True
        } else if word.eq_ignore_ascii_case("false") {
            TokenKind::False
        } else {
            TokenKind::Identifier(word.to_owned())
        };
        self.push(kind, start, self.offset)
    }

    fn parameter(&mut self) -> Result<(), ParseError> {
        let start = self.offset;
        self.offset += 1;
        let Some(next) = self.source.as_bytes().get(self.offset).copied() else {
            return self.push(TokenKind::InvalidParameter, start, self.offset);
        };
        if !(next.is_ascii_alphabetic() || next == b'_') {
            return self.push(TokenKind::InvalidParameter, start, self.offset);
        }
        let name_start = self.offset;
        self.offset += 1;
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_alphanumeric)
            || self.source.as_bytes().get(self.offset) == Some(&b'_')
        {
            self.offset += 1;
        }
        self.parameter_references += 1;
        if self.parameter_references > MAX_PARAMETER_REFERENCES {
            return Err(ParseError::too_complex(
                Span {
                    start,
                    end: self.offset,
                },
                ComplexityLimit::ParameterReferences,
            ));
        }
        self.push(
            TokenKind::Parameter(self.source[name_start..self.offset].to_owned()),
            start,
            self.offset,
        )
    }

    fn number(&mut self) -> Result<(), ParseError> {
        let start = self.offset;
        if self.source.as_bytes()[self.offset] == b'-' {
            self.offset += 1;
        }
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_digit)
        {
            self.offset += 1;
        }
        if self.source.as_bytes().get(self.offset) == Some(&b'.')
            && self
                .source
                .as_bytes()
                .get(self.offset + 1)
                .is_some_and(u8::is_ascii_digit)
        {
            self.offset += 2;
            while self
                .source
                .as_bytes()
                .get(self.offset)
                .is_some_and(u8::is_ascii_digit)
            {
                self.offset += 1;
            }
        }
        if self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(|byte| *byte == b'e' || *byte == b'E')
        {
            let exponent = self.offset;
            self.offset += 1;
            if self
                .source
                .as_bytes()
                .get(self.offset)
                .is_some_and(|byte| *byte == b'+' || *byte == b'-')
            {
                self.offset += 1;
            }
            let digits = self.offset;
            while self
                .source
                .as_bytes()
                .get(self.offset)
                .is_some_and(u8::is_ascii_digit)
            {
                self.offset += 1;
            }
            if digits == self.offset {
                self.offset = exponent;
            }
        }
        self.push(
            TokenKind::NumberLiteral(self.source[start..self.offset].to_owned()),
            start,
            self.offset,
        )
    }

    fn string(&mut self) -> Result<(), ParseError> {
        let start = self.offset;
        self.offset += 1;
        let mut decoded = String::new();
        loop {
            if self.offset == self.source.len() {
                return Err(ParseError::syntax(
                    Span {
                        start,
                        end: self.offset,
                    },
                    SyntaxReason::UnclosedString,
                ));
            }
            let character = self.source[self.offset..]
                .chars()
                .next()
                .expect("offset remains at a UTF-8 boundary");
            self.offset += character.len_utf8();
            if character == '\'' {
                if self.source.as_bytes().get(self.offset) == Some(&b'\'') {
                    decoded.push('\'');
                    self.offset += 1;
                } else {
                    return self.push(TokenKind::StringLiteral(decoded), start, self.offset);
                }
            } else {
                decoded.push(character);
            }
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) -> Result<(), ParseError> {
        self.token_count += 1;
        if self.token_count > MAX_TOKENS {
            return Err(ParseError::too_complex(
                Span { start, end },
                ComplexityLimit::Tokens,
            ));
        }
        self.tokens.push(Token {
            kind,
            span: Span { start, end },
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_every_public_grammar_token() {
        let tokens = lex("name 's' 1 :p = < <= > >= AND OR NOT IN TRUE FALSE ( ) ,").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::Identifier(_)));
        assert!(matches!(tokens[1].kind, TokenKind::StringLiteral(_)));
        assert!(matches!(tokens[2].kind, TokenKind::NumberLiteral(_)));
        assert!(matches!(tokens[3].kind, TokenKind::Parameter(_)));
        assert!(matches!(tokens[4].kind, TokenKind::Eq));
        assert!(matches!(tokens[5].kind, TokenKind::Lt));
        assert!(matches!(tokens[6].kind, TokenKind::Lte));
        assert!(matches!(tokens[7].kind, TokenKind::Gt));
        assert!(matches!(tokens[8].kind, TokenKind::Gte));
        assert!(matches!(tokens[9].kind, TokenKind::And));
        assert!(matches!(tokens[10].kind, TokenKind::Or));
        assert!(matches!(tokens[11].kind, TokenKind::Not));
        assert!(matches!(tokens[12].kind, TokenKind::In));
        assert!(matches!(tokens[13].kind, TokenKind::True));
        assert!(matches!(tokens[14].kind, TokenKind::False));
        assert!(matches!(tokens[15].kind, TokenKind::LeftParen));
        assert!(matches!(tokens[16].kind, TokenKind::RightParen));
        assert!(matches!(tokens[17].kind, TokenKind::Comma));
        assert!(matches!(tokens[18].kind, TokenKind::End));
    }

    #[test]
    fn strings_use_byte_spans_and_doubled_quotes() {
        let tokens = lex("x = 'Café O''Reilly'").unwrap();
        assert_eq!(tokens[2].span, Span { start: 4, end: 21 });
        assert_eq!(
            tokens[2].kind,
            TokenKind::StringLiteral("Café O'Reilly".into())
        );
    }

    #[test]
    fn token_limit_accepts_exactly_the_limit() {
        assert_eq!(lex(&"(".repeat(MAX_TOKENS)).unwrap().len(), MAX_TOKENS + 1);
        let error = lex(&"(".repeat(MAX_TOKENS + 1)).unwrap_err();
        assert_eq!(error.limit, Some(ComplexityLimit::Tokens));
    }
}
