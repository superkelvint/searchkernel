//! The low-level Vespa query model and the deliberately small YQL parser.

use crate::error::{check, cstring};
use crate::linguistics::{Linguistics, LinguisticsOptions};
use crate::native::{
    length_u32, text_query_terms, validate_term_query, NativeError, NativeQuery, NativeQueryTerms,
};
use crate::{Error, Result};
use searchkernel_vespa_bindings as sys;

pub use crate::document::ScalarValue as Value;

pub(crate) const MAX_QUERY_DEPTH: usize = 256;
pub(crate) const MAX_QUERY_NODES: usize = 65_536;
pub(crate) const MAX_YQL_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A bound of an integer attribute range.
pub enum RangeBound {
    Unbounded,
    Included(i64),
    Excluded(i64),
}

#[derive(Debug, Clone, PartialEq)]
/// A bound of an attribute range.
pub enum AttributeBound {
    Unbounded,
    Included(Value),
    Excluded(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// A key/value pair used by Vespa weighted-set and WAND query nodes.
pub enum WeightedQueryItem {
    String { key: String, weight: i64 },
    Integer { key: i64, weight: i64 },
}

#[derive(Debug, Clone, PartialEq)]
/// A low-level query which is lowered directly to Vespa query nodes.
pub enum Query {
    True,
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
    Near {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    ONear {
        field: String,
        children: Vec<Query>,
        distance: u32,
    },
    Equiv(Vec<Query>),
    WeakAnd {
        field: String,
        children: Vec<Query>,
        target_num_hits: u32,
    },
    Rank {
        matching: Box<Query>,
        ranking: Vec<Query>,
    },
    Fuzzy {
        field: String,
        text: String,
        max_edit_distance: u32,
        prefix_lock_length: u32,
        prefix_match: bool,
    },
    WeightedSet {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    Wand {
        field: String,
        items: Vec<WeightedQueryItem>,
    },
    Equals {
        field: String,
        value: i64,
    },
    Range {
        field: String,
        lower: RangeBound,
        upper: RangeBound,
    },
    Exists {
        field: String,
    },
    AttributeEquals {
        field: String,
        value: Value,
    },
    AttributeRange {
        field: String,
        lower: AttributeBound,
        upper: AttributeBound,
    },
    NearestNeighbor {
        document_field: String,
        query_tensor: String,
        target_num_hits: u32,
        approximate: bool,
    },
}

impl Query {
    pub fn term(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Term {
            field: field.into(),
            text: text.into(),
        }
    }

    pub fn phrase(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Phrase {
            field: field.into(),
            text: text.into(),
        }
    }

    pub fn equals(field: impl Into<String>, value: i64) -> Self {
        Self::Equals {
            field: field.into(),
            value,
        }
    }

    pub fn range(field: impl Into<String>, lower: RangeBound, upper: RangeBound) -> Self {
        Self::Range {
            field: field.into(),
            lower,
            upper,
        }
    }

    pub fn exists(field: impl Into<String>) -> Self {
        Self::Exists {
            field: field.into(),
        }
    }

    pub fn nearest_neighbor(
        document_field: impl Into<String>,
        query_tensor: impl Into<String>,
    ) -> Self {
        Self::NearestNeighbor {
            document_field: document_field.into(),
            query_tensor: query_tensor.into(),
            target_num_hits: 100,
            approximate: true,
        }
    }

    /// Parse the supported YQL subset into the same model used by typed callers.
    ///
    /// This intentionally returns only the expression. Use
    /// [`crate::SearchRequest::from_yql`] when the YQL source selector must be
    /// retained for engine-side validation.
    pub fn from_yql(yql: &str) -> Result<Self> {
        let query = parse_yql(yql)?.query;
        validate_query(&query)?;
        Ok(query)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedYql {
    pub source: String,
    pub query: Query,
}

pub(crate) fn parse_yql(yql: &str) -> Result<ParsedYql> {
    yql::parse(yql)
}

pub(crate) fn validate_query(query: &Query) -> Result<()> {
    let mut nodes = 0;
    validate_query_at(query, 0, &mut nodes)
}

fn validate_query_at(query: &Query, depth: usize, nodes: &mut usize) -> Result<()> {
    if depth > MAX_QUERY_DEPTH {
        return Err(invalid_query(format!(
            "query nesting exceeds maximum depth {MAX_QUERY_DEPTH}"
        )));
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or_else(|| invalid_query("query node count overflowed"))?;
    if *nodes > MAX_QUERY_NODES {
        return Err(invalid_query(format!(
            "query contains more than {MAX_QUERY_NODES} nodes"
        )));
    }
    match query {
        Query::True => {}
        Query::Term { field, text } | Query::Phrase { field, text } => {
            if field.is_empty() || text.is_empty() {
                return Err(invalid_query("text query field and text must not be empty"));
            }
        }
        Query::And(children)
        | Query::Or(children)
        | Query::WeakAnd { children, .. }
        | Query::Equiv(children) => {
            if children.is_empty() {
                return Err(invalid_query(
                    "boolean query must contain at least one child",
                ));
            }
            if children.len() > MAX_QUERY_NODES {
                return Err(invalid_query(format!(
                    "boolean query contains more than {MAX_QUERY_NODES} children"
                )));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Near {
            field,
            children,
            distance,
        }
        | Query::ONear {
            field,
            children,
            distance,
        } => {
            if field.is_empty() {
                return Err(invalid_query("proximity query field must not be empty"));
            }
            if *distance == 0 {
                return Err(invalid_query("proximity query distance must be positive"));
            }
            if children.len() < 2 {
                return Err(invalid_query(
                    "proximity query requires at least two children",
                ));
            }
            for child in children {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Not(child) => validate_query_at(child, depth + 1, nodes)?,
        Query::Rank { matching, ranking } => {
            validate_query_at(matching, depth + 1, nodes)?;
            for child in ranking {
                validate_query_at(child, depth + 1, nodes)?;
            }
        }
        Query::Fuzzy {
            field,
            text,
            max_edit_distance,
            prefix_lock_length,
            ..
        } => {
            if field.is_empty() || text.is_empty() {
                return Err(invalid_query(
                    "fuzzy query field and text must not be empty",
                ));
            }
            if *max_edit_distance == 0 && *prefix_lock_length > 0 {
                return Err(invalid_query(
                    "fuzzy prefix lock length requires a positive edit distance",
                ));
            }
        }
        Query::WeightedSet { field, items } | Query::Wand { field, items } => {
            if field.is_empty() {
                return Err(invalid_query("weighted query field must not be empty"));
            }
            if items.is_empty() {
                return Err(invalid_query("weighted query requires at least one item"));
            }
            let first_kind = matches!(items.first(), Some(WeightedQueryItem::String { .. }));
            if items
                .iter()
                .any(|item| matches!(item, WeightedQueryItem::String { .. }) != first_kind)
            {
                return Err(invalid_query(
                    "weighted query cannot mix string and integer keys",
                ));
            }
        }
        Query::Equals { field, .. } | Query::Exists { field } => {
            if field.is_empty() {
                return Err(invalid_query("numeric query field must not be empty"));
            }
        }
        Query::Range { field, .. } | Query::AttributeRange { field, .. } => {
            if field.is_empty() {
                return Err(invalid_query("range query field must not be empty"));
            }
        }
        Query::AttributeEquals { field, value } => {
            if field.is_empty() {
                return Err(invalid_query("attribute query field must not be empty"));
            }
            validate_value(value)?;
        }
        Query::NearestNeighbor {
            document_field,
            query_tensor,
            target_num_hits,
            ..
        } => {
            if document_field.is_empty() || query_tensor.is_empty() {
                return Err(invalid_query(
                    "nearest-neighbor field and query tensor must not be empty",
                ));
            }
            if *target_num_hits == 0 {
                return Err(invalid_query(
                    "nearest-neighbor target hits must be positive",
                ));
            }
        }
    }
    Ok(())
}

fn validate_value(value: &Value) -> Result<()> {
    if let Value::Float(value) = value {
        if !value.is_finite() {
            return Err(invalid_query("attribute float value must be finite"));
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
        if input.len() > super::MAX_YQL_BYTES {
            return Err(invalid_yql_error(format!(
                "YQL exceeds maximum size {} bytes",
                super::MAX_YQL_BYTES
            )));
        }
        let mut parser = Parser::new(input)?;
        parser.parse_statement()
    }

    struct Parser {
        tokens: Vec<Token>,
        position: usize,
        depth: usize,
    }

    impl Parser {
        fn new(input: &str) -> Result<Self> {
            Ok(Self {
                tokens: lex(input)?,
                position: 0,
                depth: 0,
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
                    return Err(unsupported(format!("unsupported YQL clause `{name}`")));
                }
                _ => {
                    return Err(invalid_yql_error(
                        "unexpected tokens after query expression",
                    ));
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
                let mut positives = Vec::with_capacity(children.len());
                let mut negatives = Vec::new();
                for child in children {
                    if matches!(child, Query::Not(_)) {
                        negatives.push(child);
                    } else {
                        positives.push(child);
                    }
                }
                positives.extend(negatives);
                Ok(Query::And(positives))
            }
        }

        fn parse_unary(&mut self) -> Result<Query> {
            if self.take_simple(Token::Bang) {
                let child = self.with_depth(|parser| parser.parse_unary())?;
                return Ok(Query::Not(Box::new(child)));
            }
            self.parse_primary()
        }

        fn parse_primary(&mut self) -> Result<Query> {
            if self.peek() == &Token::LParen && self.peek_n(1) == &Token::LBrace {
                return self.parse_annotated_range();
            }
            if self.take_simple(Token::LParen) {
                return self.with_depth(|parser| {
                    let query = parser.parse_or()?;
                    parser.expect_simple(Token::RParen, "expected `)`")?;
                    Ok(query)
                });
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

        fn with_depth<T>(&mut self, operation: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
            self.depth += 1;
            if self.depth > super::MAX_QUERY_DEPTH {
                self.depth -= 1;
                return Err(invalid_yql_error(format!(
                    "query nesting exceeds maximum depth {}",
                    super::MAX_QUERY_DEPTH
                )));
            }
            let result = operation(self);
            self.depth -= 1;
            result
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

        fn take_signed_integer(&mut self) -> Result<Option<i64>> {
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
        Included(i64),
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
                        .map_err(|_| invalid_yql_error("integer literal is outside i64 range"))?;
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

    fn signed_integer(magnitude: u64, negative: bool) -> Result<i64> {
        const I64_MAX: u64 = i64::MAX as u64;
        const I64_MIN_MAGNITUDE: u64 = I64_MAX + 1;
        if negative {
            if magnitude == I64_MIN_MAGNITUDE {
                Ok(i64::MIN)
            } else if magnitude <= I64_MAX {
                Ok(-(magnitude as i64))
            } else {
                Err(invalid_yql_error("integer literal is outside i64 range"))
            }
        } else if magnitude <= I64_MAX {
            Ok(magnitude as i64)
        } else {
            Err(invalid_yql_error("integer literal is outside i64 range"))
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
                    return Err(invalid_yql_error("control character in string"));
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
    use super::{
        parse_yql, validate_query, Query, RangeBound, Value, MAX_QUERY_DEPTH, MAX_QUERY_NODES,
        MAX_YQL_BYTES,
    };

    fn nested_not(depth: usize) -> Query {
        let mut query = Query::Term {
            field: "title".to_owned(),
            text: "fox".to_owned(),
        };
        for _ in 0..depth {
            query = Query::Not(Box::new(query));
        }
        query
    }

    #[test]
    fn query_depth_is_bounded_before_lowering() {
        assert!(validate_query(&nested_not(MAX_QUERY_DEPTH + 1)).is_err());
        assert!(validate_query(&nested_not(MAX_QUERY_DEPTH)).is_ok());
    }

    #[test]
    fn query_node_count_is_bounded() {
        let query = Query::And(
            (0..MAX_QUERY_NODES)
                .map(|_| Query::Term {
                    field: "title".to_owned(),
                    text: "fox".to_owned(),
                })
                .collect(),
        );
        assert!(validate_query(&query).is_err());
    }

    #[test]
    fn yql_input_size_is_bounded() {
        let yql = format!(
            "select * from sources * where title contains \"{}\"",
            "x".repeat(MAX_YQL_BYTES)
        );
        assert!(Query::from_yql(&yql).is_err());
    }

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
    fn explicit_closed_bounds_are_unsupported_but_text_is_preserved_raw() {
        let closed = Query::from_yql(
            "select * from sources * where ({bounds:\"closed\"}range(qty, 10, 20))",
        )
        .unwrap_err()
        .to_string();
        assert!(closed.starts_with("unsupported_yql:"));

        let term =
            Query::from_yql("select * from sources * where title contains \"brown fox\"").unwrap();
        assert_eq!(
            term,
            Query::Term {
                field: "title".into(),
                text: "brown fox".into(),
            }
        );

        let phrase = Query::from_yql(
            "select * from sources * where title contains phrase(\"new york\", \"hotel\")",
        )
        .unwrap();
        assert_eq!(
            phrase,
            Query::Phrase {
                field: "title".into(),
                text: "new york hotel".into(),
            }
        );
    }

    #[test]
    fn yql_does_not_analyze_text_before_field_validation() {
        let query = Query::from_yql("select * from sources * where missing contains \"brown fox\"")
            .unwrap();
        assert_eq!(
            query,
            Query::Term {
                field: "missing".into(),
                text: "brown fox".into(),
            }
        );
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
    fn yql_normalizes_and_negatives_after_positive_children() {
        let parsed = Query::from_yql(
            "select * from sources * where !(title contains \"scarlet\") and title contains \"quartz\" and !(title contains \"quiet\")",
        )
        .unwrap();
        assert_eq!(
            parsed,
            Query::And(vec![
                Query::Term {
                    field: "title".into(),
                    text: "quartz".into(),
                },
                Query::Not(Box::new(Query::Term {
                    field: "title".into(),
                    text: "scarlet".into(),
                })),
                Query::Not(Box::new(Query::Term {
                    field: "title".into(),
                    text: "quiet".into(),
                })),
            ])
        );
    }

    #[test]
    fn yql_accepts_negative_i64_values() {
        assert_eq!(
            Query::from_yql("select * from sources * where qty = -10").unwrap(),
            Query::Equals {
                field: "qty".into(),
                value: -10
            }
        );
        assert_eq!(
            Query::from_yql("select * from sources * where range(qty, -9223372036854775808, -1)")
                .unwrap(),
            Query::Range {
                field: "qty".into(),
                lower: RangeBound::Included(i64::MIN),
                upper: RangeBound::Included(-1)
            }
        );
    }

    #[test]
    fn typed_query_validation_rejects_empty_fields_and_nonfinite_values() {
        let invalid_queries = [
            Query::Term {
                field: String::new(),
                text: "fox".into(),
            },
            Query::Phrase {
                field: "title".into(),
                text: String::new(),
            },
            Query::Equals {
                field: String::new(),
                value: 1,
            },
            Query::Exists {
                field: String::new(),
            },
            Query::Range {
                field: String::new(),
                lower: RangeBound::Unbounded,
                upper: RangeBound::Unbounded,
            },
            Query::AttributeEquals {
                field: String::new(),
                value: Value::String("fox".into()),
            },
            Query::AttributeEquals {
                field: "price".into(),
                value: Value::Float(f64::NAN),
            },
            Query::AttributeRange {
                field: String::new(),
                lower: super::AttributeBound::Unbounded,
                upper: super::AttributeBound::Unbounded,
            },
        ];
        for query in invalid_queries {
            assert!(
                validate_query(&query).is_err(),
                "accepted invalid query: {query:?}"
            );
        }

        assert!(validate_query(&Query::AttributeEquals {
            field: "price".into(),
            value: Value::Float(1.5),
        })
        .is_ok());
    }
}

pub(crate) fn lower_query(
    native: &NativeQuery,
    query: &Query,
    language: LinguisticsOptions,
) -> Result<(), Error> {
    match query {
        Query::True => {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_true(native.0.as_ptr(), error.as_mut_ptr(), error.capacity())
            };
            check(status, &error)
        }
        Query::Term { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            validate_term_query(&analysis)
                .map_err(|error| invalid_query(format!("term analysis failed: {error}")))?;
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_TERM, field, &terms)
        }
        Query::Phrase { field, text } => {
            let analysis = Linguistics.analyze_query(text, language);
            if analysis.terms.is_empty() {
                return Err(invalid_query("phrase must analyze to at least one term"));
            }
            let terms = text_query_terms(&analysis)?;
            add_native_text(native, sys::SK_QUERY_TEXT_PHRASE, field, &terms)
        }
        Query::And(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_AND, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Or(children) => {
            add_native_operator(native, sys::SK_QUERY_NODE_OR, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Not(child) => {
            add_native_operator(native, sys::SK_QUERY_NODE_NOT, 1)?;
            lower_query(native, child, language)
        }
        Query::Near {
            field,
            children,
            distance,
        } => {
            add_native_proximity(native, false, field, children.len(), *distance)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::ONear {
            field,
            children,
            distance,
        } => {
            add_native_proximity(native, true, field, children.len(), *distance)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Equiv(children) => {
            add_native_equiv(native, children.len())?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::WeakAnd {
            field,
            children,
            target_num_hits,
        } => {
            add_native_weak_and(native, field, children.len(), *target_num_hits)?;
            for child in children {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Rank { matching, ranking } => {
            add_native_rank(native, 1usize.saturating_add(ranking.len()))?;
            lower_query(native, matching, language)?;
            for child in ranking {
                lower_query(native, child, language)?;
            }
            Ok(())
        }
        Query::Fuzzy {
            field,
            text,
            max_edit_distance,
            prefix_lock_length,
            prefix_match,
        } => add_native_fuzzy(
            native,
            field,
            text,
            *max_edit_distance,
            *prefix_lock_length,
            *prefix_match,
        ),
        Query::WeightedSet { field, items } => add_native_weighted(native, false, field, items),
        Query::Wand { field, items } => add_native_weighted(native, true, field, items),
        Query::Equals { field, value } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    *value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Range {
            field,
            lower,
            upper,
        } => {
            let field = cstring(field)?;
            let (lower_kind, lower_value) = native_bound(lower);
            let (upper_kind, upper_value) = native_bound(upper);
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    lower_kind,
                    lower_value,
                    upper_kind,
                    upper_value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::Exists { field } => {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_range(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    sys::SK_QUERY_BOUND_UNBOUNDED,
                    0,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
        Query::AttributeEquals { field, value } => {
            add_native_attribute_equals(native, field, value)
        }
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let value_kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            match value_kind {
                None => {
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_exists(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Integer) => {
                    let (lower_kind, lower_value) = integer_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = integer_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_integer_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::Float) => {
                    let (lower_kind, lower_value) = float_attribute_bound(lower)?;
                    let (upper_kind, upper_value) = float_attribute_bound(upper)?;
                    let field = cstring(field)?;
                    let mut error = NativeError::new();
                    let status = unsafe {
                        sys::sk_query_add_attribute_float_range(
                            native.0.as_ptr(),
                            field.as_ptr(),
                            lower_kind,
                            lower_value,
                            upper_kind,
                            upper_value,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    };
                    check(status, &error)
                }
                Some(AttributeKind::String | AttributeKind::Boolean) => {
                    Err(Error::message("unsupported attribute range field type"))
                }
            }
        }
        Query::NearestNeighbor {
            document_field,
            query_tensor,
            target_num_hits,
            approximate,
        } => {
            let document_field = cstring(document_field)?;
            let query_tensor = cstring(query_tensor)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_query_add_nearest_neighbor(
                    native.0.as_ptr(),
                    document_field.as_ptr(),
                    query_tensor.as_ptr(),
                    *target_num_hits,
                    u8::from(*approximate),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)
        }
    }
}

pub(crate) fn validate_query_fields(native: &NativeQuery, query: &Query) -> Result<(), Error> {
    match query {
        Query::True => Ok(()),
        Query::Term { field, .. } | Query::Phrase { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)
        }
        Query::And(children) | Query::Or(children) => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Not(child) => validate_query_fields(native, child),
        Query::Near {
            field, children, ..
        }
        | Query::ONear {
            field, children, ..
        } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)?;
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Equiv(children) => {
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::WeakAnd {
            field, children, ..
        } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)?;
            for child in children {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Rank { matching, ranking } => {
            validate_query_fields(native, matching)?;
            for child in ranking {
                validate_query_fields(native, child)?;
            }
            Ok(())
        }
        Query::Fuzzy { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field)
                .or_else(|_| validate_native_field(native, sys::SK_QUERY_FIELD_STRING, field))
        }
        Query::WeightedSet { field, .. } | Query::Wand { field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
                .or_else(|_| validate_native_field(native, sys::SK_QUERY_FIELD_TEXT, field))
        }
        Query::Equals { field, .. } | Query::Range { field, .. } | Query::Exists { field } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_INTEGER, field)
        }
        Query::AttributeEquals { field, value } => validate_native_field(
            native,
            match value {
                Value::String(_) => sys::SK_QUERY_FIELD_STRING,
                Value::Integer(_) => sys::SK_QUERY_FIELD_INTEGER,
                Value::Float(_) => sys::SK_QUERY_FIELD_FLOAT,
                Value::Boolean(_) => sys::SK_QUERY_FIELD_BOOLEAN,
            },
            field,
        ),
        Query::AttributeRange {
            field,
            lower,
            upper,
        } => {
            let kind = attribute_range_kind(lower).or_else(|| attribute_range_kind(upper));
            if let Some(kind) = kind {
                validate_native_field(
                    native,
                    match kind {
                        AttributeKind::Integer => sys::SK_QUERY_FIELD_INTEGER,
                        AttributeKind::Float => sys::SK_QUERY_FIELD_FLOAT,
                        AttributeKind::String => sys::SK_QUERY_FIELD_STRING,
                        AttributeKind::Boolean => sys::SK_QUERY_FIELD_BOOLEAN,
                    },
                    field,
                )
            } else {
                validate_native_field(native, sys::SK_QUERY_FIELD_ATTRIBUTE, field)
            }
        }
        Query::NearestNeighbor { document_field, .. } => {
            validate_native_field(native, sys::SK_QUERY_FIELD_TENSOR, document_field)
        }
    }
}

#[derive(Clone, Copy)]
enum AttributeKind {
    String,
    Integer,
    Float,
    Boolean,
}

fn attribute_range_kind(bound: &AttributeBound) -> Option<AttributeKind> {
    match bound {
        AttributeBound::Unbounded => None,
        AttributeBound::Included(Value::String(_)) | AttributeBound::Excluded(Value::String(_)) => {
            Some(AttributeKind::String)
        }
        AttributeBound::Included(Value::Integer(_))
        | AttributeBound::Excluded(Value::Integer(_)) => Some(AttributeKind::Integer),
        AttributeBound::Included(Value::Float(_)) | AttributeBound::Excluded(Value::Float(_)) => {
            Some(AttributeKind::Float)
        }
        AttributeBound::Included(Value::Boolean(_))
        | AttributeBound::Excluded(Value::Boolean(_)) => Some(AttributeKind::Boolean),
    }
}

fn integer_attribute_bound(bound: &AttributeBound) -> Result<(u32, i64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0)),
        AttributeBound::Included(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Integer(value)) => {
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching integer types",
        )),
    }
}

fn float_attribute_bound(bound: &AttributeBound) -> Result<(u32, f64), Error> {
    match bound {
        AttributeBound::Unbounded => Ok((sys::SK_QUERY_BOUND_UNBOUNDED, 0.0)),
        AttributeBound::Included(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_INCLUDED, *value))
        }
        AttributeBound::Excluded(Value::Float(value)) => {
            if !value.is_finite() {
                return Err(Error::message("attribute float bound must be finite"));
            }
            Ok((sys::SK_QUERY_BOUND_EXCLUDED, *value))
        }
        _ => Err(Error::message(
            "wrong value type: attribute range bounds must have matching float types",
        )),
    }
}

fn add_native_attribute_equals(
    native: &NativeQuery,
    field: &str,
    value: &Value,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        match value {
            Value::String(value) => {
                let value = cstring(value)?;
                sys::sk_query_add_attribute_string_equals(
                    native.0.as_ptr(),
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
            Value::Integer(value) => sys::sk_query_add_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Float(value) => sys::sk_query_add_attribute_float_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            ),
            Value::Boolean(value) => sys::sk_query_add_attribute_boolean_equals(
                native.0.as_ptr(),
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            ),
        }
    };
    check(status, &error)
}

fn validate_native_field(native: &NativeQuery, kind: u32, field: &str) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_validate_field(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_operator(native: &NativeQuery, kind: u32, count: usize) -> Result<(), Error> {
    let count = length_u32(count, "query child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_operator(
            native.0.as_ptr(),
            kind,
            count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_proximity(
    native: &NativeQuery,
    ordered: bool,
    field: &str,
    child_count: usize,
    distance: u32,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let child_count = length_u32(child_count, "proximity child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_proximity(
            native.0.as_ptr(),
            u8::from(ordered),
            field.as_ptr(),
            child_count,
            distance,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_equiv(native: &NativeQuery, child_count: usize) -> Result<(), Error> {
    let child_count = length_u32(child_count, "equiv child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_equiv(
            native.0.as_ptr(),
            child_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_weak_and(
    native: &NativeQuery,
    field: &str,
    child_count: usize,
    target_num_hits: u32,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let child_count = length_u32(child_count, "weak-and child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_weak_and(
            native.0.as_ptr(),
            field.as_ptr(),
            child_count,
            target_num_hits,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_rank(native: &NativeQuery, child_count: usize) -> Result<(), Error> {
    let child_count = length_u32(child_count, "rank child")?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_rank(
            native.0.as_ptr(),
            child_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_fuzzy(
    native: &NativeQuery,
    field: &str,
    text: &str,
    max_edit_distance: u32,
    prefix_lock_length: u32,
    prefix_match: bool,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let text = cstring(text)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_fuzzy(
            native.0.as_ptr(),
            field.as_ptr(),
            text.as_ptr(),
            max_edit_distance,
            prefix_lock_length,
            u8::from(prefix_match),
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn add_native_weighted(
    native: &NativeQuery,
    wand: bool,
    field: &str,
    items: &[WeightedQueryItem],
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match items.first() {
        Some(WeightedQueryItem::String { .. }) => {
            let mut key_bytes = Vec::new();
            let mut key_lengths = Vec::with_capacity(items.len());
            let mut weights = Vec::with_capacity(items.len());
            for item in items {
                let WeightedQueryItem::String { key, weight } = item else {
                    return Err(invalid_query(
                        "weighted query cannot mix string and integer keys",
                    ));
                };
                key_lengths.push(length_u32(key.len(), "weighted query key")?);
                key_bytes.extend_from_slice(key.as_bytes());
                weights.push(*weight);
            }
            let item_count = length_u32(items.len(), "weighted query item")?;
            unsafe {
                if wand {
                    sys::sk_query_add_wand_strings(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        key_bytes.as_ptr(),
                        length_u32(key_bytes.len(), "weighted query key bytes")?,
                        key_lengths.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                } else {
                    sys::sk_query_add_weighted_set_strings(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        key_bytes.as_ptr(),
                        length_u32(key_bytes.len(), "weighted query key bytes")?,
                        key_lengths.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                }
            }
        }
        Some(WeightedQueryItem::Integer { .. }) => {
            let mut keys = Vec::with_capacity(items.len());
            let mut weights = Vec::with_capacity(items.len());
            for item in items {
                let WeightedQueryItem::Integer { key, weight } = item else {
                    return Err(invalid_query(
                        "weighted query cannot mix string and integer keys",
                    ));
                };
                keys.push(*key);
                weights.push(*weight);
            }
            let item_count = length_u32(items.len(), "weighted query item")?;
            unsafe {
                if wand {
                    sys::sk_query_add_wand_integers(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        keys.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                } else {
                    sys::sk_query_add_weighted_set_integers(
                        native.0.as_ptr(),
                        field.as_ptr(),
                        keys.as_ptr(),
                        weights.as_ptr(),
                        item_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                }
            }
        }
        None => return Err(invalid_query("weighted query requires at least one item")),
    };
    check(status, &error)
}

fn add_native_text(
    native: &NativeQuery,
    kind: u32,
    field: &str,
    terms: &NativeQueryTerms,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_query_add_text(
            native.0.as_ptr(),
            kind,
            field.as_ptr(),
            terms.term_bytes.as_ptr(),
            length_u32(terms.term_bytes.len(), "query text term bytes")?,
            terms.term_lengths.as_ptr(),
            length_u32(terms.term_lengths.len(), "query text term")?,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)
}

fn native_bound(bound: &RangeBound) -> (u32, i64) {
    match bound {
        RangeBound::Unbounded => (sys::SK_QUERY_BOUND_UNBOUNDED, 0),
        RangeBound::Included(value) => (sys::SK_QUERY_BOUND_INCLUDED, *value),
        RangeBound::Excluded(value) => (sys::SK_QUERY_BOUND_EXCLUDED, *value),
    }
}
