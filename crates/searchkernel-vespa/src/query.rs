//! The low-level Vespa query model and the deliberately small YQL parser.

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
/// A bound of an integer attribute range.
pub enum RangeBound {
    Unbounded,
    Included(i32),
    Excluded(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A low-level query which is lowered directly to Vespa query nodes.
pub enum Query {
    Term {
        field: String,
        text: String,
    },
    Phrase {
        field: String,
        text: String,
    },
    And(Vec<Query>),
    Or(Vec<Query>),
    Not(Box<Query>),
    Equals {
        field: String,
        value: i32,
    },
    Range {
        field: String,
        lower: RangeBound,
        upper: RangeBound,
    },
    Exists {
        field: String,
    },
}

impl Query {
    /// Parse the supported YQL subset into the same model used by typed callers.
    pub fn from_yql(yql: &str) -> Result<Self> {
        Ok(yql::parse(yql)?.query)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedYql {
    pub source: String,
    pub query: Query,
}

pub(crate) fn parse_yql(yql: &str) -> Result<ParsedYql> {
    yql::parse(yql)
}

pub(crate) fn validate_query(query: &Query) -> Result<()> {
    match query {
        Query::Term { field, text } | Query::Phrase { field, text } => {
            if field.is_empty() || text.is_empty() {
                return Err(invalid_query("text query field and text must not be empty"));
            }
        }
        Query::And(children) | Query::Or(children) => {
            if children.is_empty() {
                return Err(invalid_query(
                    "boolean query must contain at least one child",
                ));
            }
            for child in children {
                validate_query(child)?;
            }
        }
        Query::Not(child) => validate_query(child)?,
        Query::Equals { field, .. } | Query::Exists { field } => {
            if field.is_empty() {
                return Err(invalid_query("numeric query field must not be empty"));
            }
        }
        Query::Range { field, .. } => {
            if field.is_empty() {
                return Err(invalid_query("range query field must not be empty"));
            }
        }
    }
    Ok(())
}

pub(crate) fn invalid_query(message: impl Into<String>) -> Error {
    Error::message(format!("invalid_query: {}", message.into()))
}

mod yql {
    use super::{Query, RangeBound};
    use crate::{Error, Result};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Token {
        Identifier(String),
        String(String),
        Integer(u64),
        Star,
        LParen,
        RParen,
        LBrace,
        RBrace,
        Comma,
        Colon,
        Equals,
        Bang,
        Minus,
        End,
    }

    pub(super) fn parse(input: &str) -> Result<super::ParsedYql> {
        let mut parser = Parser::new(input)?;
        parser.parse_statement()
    }

    struct Parser {
        tokens: Vec<Token>,
        position: usize,
    }

    impl Parser {
        fn new(input: &str) -> Result<Self> {
            Ok(Self {
                tokens: lex(input)?,
                position: 0,
            })
        }

        fn parse_statement(&mut self) -> Result<super::ParsedYql> {
            self.expect_keyword("select")?;
            if !self.take_simple(Token::Star) {
                return Err(unsupported("only `select *` is supported"));
            }
            self.expect_keyword("from")?;
            let source = if self.take_keyword("sources") {
                if !self.take_simple(Token::Star) {
                    return Err(unsupported("only `from sources *` is supported"));
                }
                "*".to_owned()
            } else {
                self.take_identifier()
                    .ok_or_else(|| invalid_yql_error("expected a query source"))?
            };
            self.expect_keyword("where")?;
            let query = self.parse_or()?;
            match self.peek() {
                Token::End => {}
                Token::Identifier(name) if is_unsupported_clause(name) => {
                    return Err(unsupported(format!("unsupported YQL clause `{name}`")))
                }
                _ => {
                    return Err(invalid_yql_error(
                        "unexpected tokens after query expression",
                    ))
                }
            }
            Ok(super::ParsedYql { source, query })
        }

        fn parse_or(&mut self) -> Result<Query> {
            let first = self.parse_and()?;
            let mut children = vec![first];
            while self.take_keyword("or") {
                children.push(self.parse_and()?);
            }
            if children.len() == 1 {
                Ok(children.pop().expect("one child was inserted"))
            } else {
                Ok(Query::Or(children))
            }
        }

        fn parse_and(&mut self) -> Result<Query> {
            let first = self.parse_unary()?;
            let mut children = vec![first];
            while self.take_keyword("and") {
                children.push(self.parse_unary()?);
            }
            if children.len() == 1 {
                Ok(children.pop().expect("one child was inserted"))
            } else {
                Ok(Query::And(children))
            }
        }

        fn parse_unary(&mut self) -> Result<Query> {
            if self.take_simple(Token::Bang) {
                return Ok(Query::Not(Box::new(self.parse_unary()?)));
            }
            self.parse_primary()
        }

        fn parse_primary(&mut self) -> Result<Query> {
            if self.peek() == &Token::LParen && self.peek_n(1) == &Token::LBrace {
                return self.parse_annotated_range();
            }
            if self.take_simple(Token::LParen) {
                let query = self.parse_or()?;
                self.expect_simple(Token::RParen, "expected `)`")?;
                return Ok(query);
            }
            if self.peek_keyword("range") && self.peek_n(1) == &Token::LParen {
                self.expect_keyword("range")?;
                return self.parse_range(None);
            }
            let field = self
                .take_identifier()
                .ok_or_else(|| invalid_yql_error("expected query expression"))?;
            if !self.take_keyword("contains") {
                if self.take_simple(Token::Equals) {
                    let value = self.take_signed_integer()?.ok_or_else(|| {
                        invalid_yql_error("numeric equality requires an integer literal")
                    })?;
                    return Ok(Query::Equals { field, value });
                }
                return Err(unsupported(
                    "only `contains` and `=` field expressions are supported",
                ));
            }
            if self.take_keyword("phrase") {
                let terms = self.parse_phrase_terms()?;
                return Ok(Query::Phrase {
                    field,
                    text: terms.join(" "),
                });
            }
            let text = self.take_string().ok_or_else(|| {
                invalid_yql_error("contains requires a quoted term or phrase expression")
            })?;
            require_one_query_position(&text, "term")?;
            Ok(Query::Term { field, text })
        }

        fn parse_annotated_range(&mut self) -> Result<Query> {
            self.expect_simple(Token::LParen, "expected bounds annotation")?;
            self.expect_simple(Token::LBrace, "expected `{` in bounds annotation")?;
            self.expect_keyword("bounds")?;
            self.expect_simple(Token::Colon, "expected `:` in bounds annotation")?;
            let bounds = self
                .take_string()
                .ok_or_else(|| invalid_yql_error("bounds annotation requires a string value"))?;
            self.expect_simple(Token::RBrace, "expected `}` in bounds annotation")?;
            if !self.take_keyword("range") {
                return Err(unsupported(
                    "only bounds annotations on range are supported",
                ));
            }
            let query = self.parse_range(Some(bounds))?;
            self.expect_simple(Token::RParen, "expected `)` after annotated range")?;
            Ok(query)
        }

        fn parse_range(&mut self, bounds: Option<String>) -> Result<Query> {
            self.expect_simple(Token::LParen, "expected `(` after range")?;
            let field = self
                .take_identifier()
                .ok_or_else(|| invalid_yql_error("range requires a field name"))?;
            self.expect_simple(Token::Comma, "expected `,` after range field")?;
            let lower = self.parse_endpoint(true)?;
            self.expect_simple(Token::Comma, "expected `,` after range lower bound")?;
            let upper = self.parse_endpoint(false)?;
            self.expect_simple(Token::RParen, "expected `)` after range bounds")?;
            let (lower, upper) = apply_bounds(lower, upper, bounds.as_deref())?;
            if matches!(
                (&lower, &upper),
                (RangeBound::Unbounded, RangeBound::Unbounded)
            ) {
                return Ok(Query::Exists { field });
            }
            let (lower, upper) = normalize_unannotated_range(lower, upper, bounds.is_none());
            if let (RangeBound::Included(low), RangeBound::Included(high)) = (&lower, &upper) {
                if low == high {
                    return Ok(Query::Equals { field, value: *low });
                }
            }
            Ok(Query::Range {
                field,
                lower,
                upper,
            })
        }

        fn parse_phrase_terms(&mut self) -> Result<Vec<String>> {
            self.expect_simple(Token::LParen, "expected `(` after phrase")?;
            let mut terms = Vec::new();
            loop {
                terms.push(self.take_string().ok_or_else(|| {
                    invalid_yql_error("phrase requires one or more quoted terms")
                })?);
                if !self.take_simple(Token::Comma) {
                    break;
                }
            }
            self.expect_simple(Token::RParen, "expected `)` after phrase terms")?;
            for term in &terms {
                require_one_query_position(term, "phrase")?;
            }
            Ok(terms)
        }

        fn parse_endpoint(&mut self, lower: bool) -> Result<Endpoint> {
            if self.take_simple(Token::Minus) {
                if self.take_keyword("infinity") {
                    if lower {
                        return Ok(Endpoint::Unbounded);
                    }
                    return Err(invalid_yql_error("negative Infinity is only a lower bound"));
                }
                let magnitude = self.take_integer_magnitude().ok_or_else(|| {
                    invalid_yql_error("expected an integer or Infinity range bound")
                })?;
                return signed_integer(magnitude, true).map(Endpoint::Included);
            }
            if self.take_keyword("infinity") {
                if !lower {
                    return Ok(Endpoint::Unbounded);
                }
                return Err(invalid_yql_error(
                    "positive Infinity is only an upper bound",
                ));
            }
            let magnitude = self
                .take_integer_magnitude()
                .ok_or_else(|| invalid_yql_error("expected an integer or Infinity range bound"))?;
            signed_integer(magnitude, false).map(Endpoint::Included)
        }

        fn peek(&self) -> &Token {
            &self.tokens[self.position]
        }

        fn peek_n(&self, offset: usize) -> &Token {
            self.tokens
                .get(self.position + offset)
                .unwrap_or_else(|| self.tokens.last().expect("lexer always emits End"))
        }

        fn take_simple(&mut self, expected: Token) -> bool {
            if *self.peek() == expected {
                self.position += 1;
                true
            } else {
                false
            }
        }

        fn expect_simple(&mut self, expected: Token, message: &str) -> Result<()> {
            if self.take_simple(expected) {
                Ok(())
            } else {
                Err(invalid_yql_error(message))
            }
        }

        fn take_keyword(&mut self, expected: &str) -> bool {
            match self.peek() {
                Token::Identifier(value) if value.eq_ignore_ascii_case(expected) => {
                    self.position += 1;
                    true
                }
                _ => false,
            }
        }

        fn peek_keyword(&self, expected: &str) -> bool {
            matches!(
                self.peek(),
                Token::Identifier(value) if value.eq_ignore_ascii_case(expected)
            )
        }

        fn expect_keyword(&mut self, expected: &str) -> Result<()> {
            if self.take_keyword(expected) {
                Ok(())
            } else {
                Err(invalid_yql_error(format!("expected `{expected}`")))
            }
        }

        fn take_identifier(&mut self) -> Option<String> {
            match self.peek() {
                Token::Identifier(value) => {
                    let value = value.clone();
                    self.position += 1;
                    Some(value)
                }
                _ => None,
            }
        }

        fn take_string(&mut self) -> Option<String> {
            match self.peek() {
                Token::String(value) => {
                    let value = value.clone();
                    self.position += 1;
                    Some(value)
                }
                _ => None,
            }
        }

        fn take_integer_magnitude(&mut self) -> Option<u64> {
            match self.peek() {
                Token::Integer(value) => {
                    let value = *value;
                    self.position += 1;
                    Some(value)
                }
                _ => None,
            }
        }

        fn take_signed_integer(&mut self) -> Result<Option<i32>> {
            let negative = self.take_simple(Token::Minus);
            let Some(magnitude) = self.take_integer_magnitude() else {
                if negative {
                    self.position -= 1;
                }
                return Ok(None);
            };
            signed_integer(magnitude, negative).map(Some)
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Endpoint {
        Unbounded,
        Included(i32),
    }

    fn apply_bounds(
        lower: Endpoint,
        upper: Endpoint,
        bounds: Option<&str>,
    ) -> Result<(RangeBound, RangeBound)> {
        let (lower_inclusive, upper_inclusive) = match bounds {
            None => (true, true),
            Some("leftOpen") => (false, true),
            Some("rightOpen") => (true, false),
            Some("open") => (false, false),
            Some(value) => return Err(unsupported(format!("unsupported range bounds `{value}`"))),
        };
        let lower = match lower {
            Endpoint::Unbounded => RangeBound::Unbounded,
            Endpoint::Included(value) if lower_inclusive => RangeBound::Included(value),
            Endpoint::Included(value) => RangeBound::Excluded(value),
        };
        let upper = match upper {
            Endpoint::Unbounded => RangeBound::Unbounded,
            Endpoint::Included(value) if upper_inclusive => RangeBound::Included(value),
            Endpoint::Included(value) => RangeBound::Excluded(value),
        };
        Ok((lower, upper))
    }

    fn normalize_unannotated_range(
        lower: RangeBound,
        upper: RangeBound,
        unannotated: bool,
    ) -> (RangeBound, RangeBound) {
        if unannotated {
            if let (RangeBound::Included(low), RangeBound::Included(high)) = (&lower, &upper) {
                if low > high {
                    return (RangeBound::Included(*high), RangeBound::Included(*low));
                }
            }
        }
        (lower, upper)
    }

    fn require_one_query_position(text: &str, kind: &str) -> Result<()> {
        let analysis = crate::Linguistics.analyze_query(text, Default::default());
        if analysis.terms.len() != 1 {
            return Err(unsupported(format!(
                "{kind} literal must analyze to exactly one query position; use phrase(...)"
            )));
        }
        Ok(())
    }

    fn lex(input: &str) -> Result<Vec<Token>> {
        let bytes = input.as_bytes();
        let mut index = 0;
        let mut tokens = Vec::new();
        while index < bytes.len() {
            match bytes[index] {
                b' ' | b'\t' | b'\r' | b'\n' => index += 1,
                b'*' => {
                    tokens.push(Token::Star);
                    index += 1;
                }
                b'(' => {
                    tokens.push(Token::LParen);
                    index += 1;
                }
                b')' => {
                    tokens.push(Token::RParen);
                    index += 1;
                }
                b'{' => {
                    tokens.push(Token::LBrace);
                    index += 1;
                }
                b'}' => {
                    tokens.push(Token::RBrace);
                    index += 1;
                }
                b',' => {
                    tokens.push(Token::Comma);
                    index += 1;
                }
                b':' => {
                    tokens.push(Token::Colon);
                    index += 1;
                }
                b'=' => {
                    tokens.push(Token::Equals);
                    index += 1;
                }
                b'!' => {
                    tokens.push(Token::Bang);
                    index += 1;
                }
                b'-' => {
                    tokens.push(Token::Minus);
                    index += 1;
                }
                b'"' => {
                    let (value, next) = lex_string(bytes, index + 1)?;
                    tokens.push(Token::String(value));
                    index = next;
                }
                byte if byte.is_ascii_digit() => {
                    let start = index;
                    while index < bytes.len() && bytes[index].is_ascii_digit() {
                        index += 1;
                    }
                    let value = std::str::from_utf8(&bytes[start..index])
                        .expect("ASCII digits are valid UTF-8")
                        .parse::<u64>()
                        .map_err(|_| invalid_yql_error("integer literal is outside i32 range"))?;
                    tokens.push(Token::Integer(value));
                }
                byte if is_identifier_start(byte) => {
                    let start = index;
                    index += 1;
                    while index < bytes.len() && is_identifier_continue(bytes[index]) {
                        index += 1;
                    }
                    tokens.push(Token::Identifier(
                        std::str::from_utf8(&bytes[start..index])
                            .expect("identifier bytes are ASCII")
                            .to_owned(),
                    ));
                }
                _ => return Err(invalid_yql_error("unexpected character in YQL")),
            }
        }
        tokens.push(Token::End);
        Ok(tokens)
    }

    fn signed_integer(magnitude: u64, negative: bool) -> Result<i32> {
        const I32_MAX: u64 = i32::MAX as u64;
        const I32_MIN_MAGNITUDE: u64 = I32_MAX + 1;
        if negative {
            if magnitude == I32_MIN_MAGNITUDE {
                Ok(i32::MIN)
            } else if magnitude <= I32_MAX {
                Ok(-(magnitude as i32))
            } else {
                Err(invalid_yql_error("integer literal is outside i32 range"))
            }
        } else if magnitude <= I32_MAX {
            Ok(magnitude as i32)
        } else {
            Err(invalid_yql_error("integer literal is outside i32 range"))
        }
    }

    fn lex_string(bytes: &[u8], mut index: usize) -> Result<(String, usize)> {
        let mut value = String::new();
        while index < bytes.len() {
            match bytes[index] {
                b'"' => return Ok((value, index + 1)),
                b'\\' => {
                    index += 1;
                    let escaped = *bytes
                        .get(index)
                        .ok_or_else(|| invalid_yql_error("unterminated string escape"))?;
                    match escaped {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        b'u' => {
                            let end = index + 5;
                            let hex = bytes
                                .get(index + 1..end)
                                .ok_or_else(|| invalid_yql_error("invalid Unicode escape"))?;
                            let hex = std::str::from_utf8(hex)
                                .map_err(|_| invalid_yql_error("invalid Unicode escape"))?;
                            let code = u16::from_str_radix(hex, 16)
                                .map_err(|_| invalid_yql_error("invalid Unicode escape"))?;
                            let character = char::from_u32(code as u32)
                                .ok_or_else(|| invalid_yql_error("invalid Unicode escape"))?;
                            value.push(character);
                            index = end - 1;
                        }
                        _ => return Err(invalid_yql_error("unsupported string escape")),
                    }
                    index += 1;
                }
                byte if byte < 0x20 => {
                    return Err(invalid_yql_error("control character in string"))
                }
                _ => {
                    let character = input_char_at(bytes, index)?;
                    value.push(character.0);
                    index = character.1;
                }
            }
        }
        Err(invalid_yql_error("unterminated string literal"))
    }

    fn input_char_at(bytes: &[u8], index: usize) -> Result<(char, usize)> {
        let value = std::str::from_utf8(&bytes[index..])
            .map_err(|_| invalid_yql_error("YQL must be valid UTF-8"))?;
        let character = value
            .chars()
            .next()
            .ok_or_else(|| invalid_yql_error("unexpected end of string"))?;
        Ok((character, index + character.len_utf8()))
    }

    fn is_identifier_start(byte: u8) -> bool {
        byte.is_ascii_alphabetic() || byte == b'_'
    }

    fn is_identifier_continue(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    fn is_unsupported_clause(name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "order"
                | "limit"
                | "offset"
                | "ranking"
                | "summary"
                | "group"
                | "userinput"
                | "userquery"
                | "fuzzy"
                | "prefix"
                | "regexp"
                | "near"
                | "onear"
                | "nearestneighbor"
        )
    }

    fn invalid_yql_error(message: impl Into<String>) -> Error {
        Error::message(format!("invalid_yql: {}", message.into()))
    }

    fn unsupported(message: impl Into<String>) -> Error {
        Error::message(format!("unsupported_yql: {}", message.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_yql, validate_query, Query, RangeBound};

    #[test]
    fn yql_respects_boolean_precedence() {
        let query = Query::from_yql(
            "select * from sources * where title contains \"brown\" or title contains \"red\" and title contains \"fox\"",
        )
        .unwrap();
        assert_eq!(
            query,
            Query::Or(vec![
                Query::Term {
                    field: "title".into(),
                    text: "brown".into()
                },
                Query::And(vec![
                    Query::Term {
                        field: "title".into(),
                        text: "red".into()
                    },
                    Query::Term {
                        field: "title".into(),
                        text: "fox".into()
                    },
                ]),
            ])
        );
    }

    #[test]
    fn yql_maps_phrase_and_open_bounds() {
        let phrase = Query::from_yql(
            "select * from sources * where title contains phrase(\"running\", \"foxes\")",
        )
        .unwrap();
        assert_eq!(
            phrase,
            Query::Phrase {
                field: "title".into(),
                text: "running foxes".into()
            }
        );
        let range = Query::from_yql(
            "select * from sources * where ({bounds:\"rightOpen\"}range(qty, 10, 20))",
        )
        .unwrap();
        assert_eq!(
            range,
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Included(10),
                upper: RangeBound::Excluded(20),
            }
        );
    }

    #[test]
    fn yql_maps_full_range_to_exists() {
        let query =
            Query::from_yql("select * from typea where range(qty, -Infinity, Infinity)").unwrap();
        assert_eq!(
            query,
            Query::Exists {
                field: "qty".into()
            }
        );
    }

    #[test]
    fn yql_normalizes_only_unannotated_reversed_ranges() {
        assert_eq!(
            Query::from_yql("select * from sources * where range(qty, 20, 10)").unwrap(),
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Included(10),
                upper: RangeBound::Included(20),
            }
        );
        assert_eq!(
            Query::from_yql(
                "select * from sources * where ({bounds:\"rightOpen\"}range(qty, 20, 10))"
            )
            .unwrap(),
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Included(20),
                upper: RangeBound::Excluded(10),
            }
        );
        assert_eq!(
            Query::from_yql("select * from sources * where ({bounds:\"open\"}range(qty, 10, 10))")
                .unwrap(),
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Excluded(10),
                upper: RangeBound::Excluded(10),
            }
        );
    }

    #[test]
    fn explicit_closed_bounds_and_multi_position_literals_are_unsupported() {
        let closed = Query::from_yql(
            "select * from sources * where ({bounds:\"closed\"}range(qty, 10, 20))",
        )
        .unwrap_err()
        .to_string();
        assert!(closed.starts_with("unsupported_yql:"));

        let term = Query::from_yql("select * from sources * where title contains \"brown fox\"")
            .unwrap_err()
            .to_string();
        assert!(term.starts_with("unsupported_yql:"));

        let phrase = Query::from_yql(
            "select * from sources * where title contains phrase(\"new york\", \"hotel\")",
        )
        .unwrap_err()
        .to_string();
        assert!(phrase.starts_with("unsupported_yql:"));
    }

    #[test]
    fn range_is_available_as_a_literal_field_name() {
        assert_eq!(
            Query::from_yql("select * from sources * where range contains \"foo\"").unwrap(),
            Query::Term {
                field: "range".into(),
                text: "foo".into(),
            }
        );
    }

    #[test]
    fn malformed_and_unsupported_yql_are_classified() {
        let malformed = Query::from_yql("select * from sources * where (title contains \"fox\"")
            .unwrap_err()
            .to_string();
        assert!(malformed.starts_with("invalid_yql:"));
        let unsupported =
            Query::from_yql("select * from sources * where title contains \"fox\" order by title")
                .unwrap_err()
                .to_string();
        assert!(unsupported.starts_with("unsupported_yql:"));
    }

    #[test]
    fn typed_boolean_and_ranges_are_validated() {
        assert!(validate_query(&Query::And(Vec::new())).is_err());
        assert!(validate_query(&Query::Or(Vec::new())).is_err());
        assert!(validate_query(&Query::Range {
            field: "qty".into(),
            lower: RangeBound::Excluded(10),
            upper: RangeBound::Included(10),
        })
        .is_ok());
    }

    #[test]
    fn parser_accepts_named_source_without_changing_query() {
        let parsed = Query::from_yql("select * from typea where title contains \"fox\"").unwrap();
        assert_eq!(
            parsed,
            Query::Term {
                field: "title".into(),
                text: "fox".into()
            }
        );
    }

    #[test]
    fn internal_parser_preserves_named_query_source() {
        let parsed = parse_yql("select * from typea where title contains \"fox\"").unwrap();
        assert_eq!(parsed.source, "typea");
        assert_eq!(
            parsed.query,
            Query::Term {
                field: "title".into(),
                text: "fox".into(),
            }
        );

        let wildcard = parse_yql("select * from sources * where title contains \"fox\"").unwrap();
        assert_eq!(wildcard.source, "*");
    }

    #[test]
    fn yql_accepts_negative_i32_values() {
        assert_eq!(
            Query::from_yql("select * from sources * where qty = -10").unwrap(),
            Query::Equals {
                field: "qty".into(),
                value: -10
            }
        );
        assert_eq!(
            Query::from_yql("select * from sources * where range(qty, -2147483648, -1)").unwrap(),
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Included(i32::MIN),
                upper: RangeBound::Included(-1)
            }
        );
    }
}
