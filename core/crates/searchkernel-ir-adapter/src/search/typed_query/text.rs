//! Text and user-query lowering.

use super::typed_query_field;
use searchkernel_ir::{
    PortableError, QueryExprIr, QueryExprKindIr, QueryFieldRefIr, RequestValueIr, Result,
};
use searchkernel_vespa_engine::Query;

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserQueryAtom {
    field: Option<String>,
    text: String,
    phrase: bool,
    negated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum UserQueryToken {
    Atom(UserQueryAtom),
    And,
    Or,
    Open,
    Close,
}

fn user_query_field(
    selected: Option<&str>,
    field_context: Option<&QueryFieldRefIr>,
    default_index: Option<&str>,
    path: &str,
) -> Result<String> {
    let field = selected
        .or_else(|| field_context.map(|field| field.path.as_str()))
        .or(default_index)
        .filter(|field| !field.is_empty())
        .ok_or_else(|| {
            PortableError::invalid(
                "user_query_default_index_missing",
                Some(path),
                "userInput/userQuery requires a containing field or defaultIndex",
            )
        })?;
    Ok(field.to_owned())
}

fn tokenize_user_query(value: &str, path: &str) -> Result<Vec<UserQueryToken>> {
    // This is deliberately a small, explicit grammar.  Do not let syntax from
    // Vespa's much larger userInput/userQuery parser fall through as a plain
    // term: that would make an unsupported query look successful with changed
    // semantics.
    for (index, character) in value.char_indices() {
        if matches!(
            character,
            '\\' | '^' | '*' | '?' | '[' | ']' | '{' | '}' | '~' | '+' | '|' | '&' | '!' | '='
        ) {
            return Err(PortableError::unsupported(
                "user_query_syntax_not_supported",
                Some(format!("{path}.value[{index}]")),
                "userInput/userQuery supports terms, quoted phrases, field selectors, parentheses, AND/OR, and leading '-' only; escaping, weights, ranges, prefixes, suffixes, and parser operators are not supported",
            ));
        }
        if character == '(' && index > 0 {
            let previous = value[..index].chars().next_back();
            if previous.is_some_and(|previous| {
                previous.is_ascii_alphanumeric() || previous == '_' || previous == '.'
            }) {
                return Err(PortableError::unsupported(
                    "user_query_composite_syntax_not_supported",
                    Some(format!("{path}.value[{index}]")),
                    "function-style and composite user-query grammar is not supported",
                ));
            }
        }
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut output = Vec::new();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        match bytes[index] {
            b'(' => {
                output.push(UserQueryToken::Open);
                index += 1;
                continue;
            }
            b')' => {
                output.push(UserQueryToken::Close);
                index += 1;
                continue;
            }
            _ => {}
        }

        let mut negated = false;
        if bytes[index] == b'-' {
            negated = true;
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index == bytes.len() {
                return Err(PortableError::invalid(
                    "invalid_user_query",
                    Some(path),
                    "a negative user-query term requires a value",
                ));
            }
        }

        let start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && bytes[index] != b'('
            && bytes[index] != b')'
            && bytes[index] != b'"'
        {
            index += 1;
        }
        let mut prefix = &value[start..index];
        let mut phrase = false;
        let text;
        if index < bytes.len() && bytes[index] == b'"' {
            if !prefix.is_empty() && !prefix.ends_with(':') {
                return Err(PortableError::invalid(
                    "invalid_user_query",
                    Some(path),
                    "a quoted term must begin a token or follow a field selector",
                ));
            }
            if prefix.is_empty() {
                prefix = "";
            }
            index += 1;
            let quoted_start = index;
            while index < bytes.len() && bytes[index] != b'"' {
                index += 1;
            }
            if index == bytes.len() {
                return Err(PortableError::invalid(
                    "invalid_user_query",
                    Some(path),
                    "unterminated quoted user-query phrase",
                ));
            }
            text = value[quoted_start..index].to_owned();
            index += 1;
            phrase = true;
        } else {
            text = prefix.to_owned();
        }
        if text.is_empty() {
            return Err(PortableError::invalid(
                "invalid_user_query",
                Some(path),
                "user-query terms must not be empty",
            ));
        }
        let (field, text) = if let Some(separator) = prefix.find(':') {
            let selected = &prefix[..separator];
            if selected.is_empty()
                || !selected
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
            {
                return Err(PortableError::invalid(
                    "invalid_user_query",
                    Some(path),
                    "user-query field selectors must contain a valid field name",
                ));
            }
            if phrase {
                (Some(selected.to_owned()), text)
            } else {
                let selected_text = &prefix[separator + 1..];
                if selected_text.is_empty() {
                    return Err(PortableError::invalid(
                        "invalid_user_query",
                        Some(path),
                        "user-query field selectors require a value",
                    ));
                }
                (Some(selected.to_owned()), selected_text.to_owned())
            }
        } else {
            (None, text)
        };
        let token = match text.as_str() {
            "AND" if !phrase && !negated && field.is_none() => UserQueryToken::And,
            "OR" if !phrase && !negated && field.is_none() => UserQueryToken::Or,
            _ => UserQueryToken::Atom(UserQueryAtom {
                field,
                text,
                phrase,
                negated,
            }),
        };
        output.push(token);
    }
    if output.is_empty() {
        return Err(PortableError::invalid(
            "empty_user_query",
            Some(path),
            "userInput/userQuery must contain a query",
        ));
    }
    Ok(output)
}

struct UserQueryParser<'a> {
    tokens: &'a [UserQueryToken],
    index: usize,
    field_context: Option<&'a QueryFieldRefIr>,
    default_index: Option<&'a str>,
    target_hits: u32,
    path: &'a str,
}

impl<'a> UserQueryParser<'a> {
    fn parse(mut self) -> Result<Query> {
        let query = self.parse_or()?;
        if self.index != self.tokens.len() {
            return Err(PortableError::invalid(
                "invalid_user_query",
                Some(self.path),
                "unexpected token in user query",
            ));
        }
        Ok(query)
    }

    fn parse_or(&mut self) -> Result<Query> {
        let mut children = vec![self.parse_and()?];
        while matches!(self.tokens.get(self.index), Some(UserQueryToken::Or)) {
            self.index += 1;
            children.push(self.parse_and()?);
        }
        if children.len() == 1 {
            Ok(children.remove(0))
        } else {
            Ok(Query::Or(children))
        }
    }

    fn parse_and(&mut self) -> Result<Query> {
        let mut children = vec![self.parse_primary()?];
        let mut explicit = false;
        loop {
            if matches!(self.tokens.get(self.index), Some(UserQueryToken::And)) {
                explicit = true;
                self.index += 1;
                children.push(self.parse_primary()?);
            } else if matches!(
                self.tokens.get(self.index),
                Some(UserQueryToken::Atom(_)) | Some(UserQueryToken::Open)
            ) {
                children.push(self.parse_primary()?);
            } else {
                break;
            }
        }
        if children.len() == 1 {
            Ok(children.remove(0))
        } else if explicit {
            Ok(Query::And(children))
        } else {
            Ok(Query::WeakAnd {
                children,
                target_num_hits: self.target_hits,
            })
        }
    }

    fn parse_primary(&mut self) -> Result<Query> {
        match self.tokens.get(self.index) {
            Some(UserQueryToken::Atom(atom)) => {
                self.index += 1;
                let field = user_query_field(
                    atom.field.as_deref(),
                    self.field_context,
                    self.default_index,
                    self.path,
                )?;
                let query = if atom.phrase {
                    Query::Phrase {
                        field,
                        text: atom.text.clone(),
                    }
                } else {
                    Query::Term {
                        field,
                        text: atom.text.clone(),
                    }
                };
                if atom.negated {
                    Ok(Query::Not(Box::new(query)))
                } else {
                    Ok(query)
                }
            }
            Some(UserQueryToken::Open) => {
                self.index += 1;
                let query = self.parse_or()?;
                if !matches!(self.tokens.get(self.index), Some(UserQueryToken::Close)) {
                    return Err(PortableError::invalid(
                        "invalid_user_query",
                        Some(self.path),
                        "unterminated user-query group",
                    ));
                }
                self.index += 1;
                Ok(query)
            }
            Some(UserQueryToken::Close) => Err(PortableError::invalid(
                "invalid_user_query",
                Some(self.path),
                "unexpected closing parenthesis in user query",
            )),
            Some(UserQueryToken::And | UserQueryToken::Or) | None => Err(PortableError::invalid(
                "invalid_user_query",
                Some(self.path),
                "user-query operator requires a term",
            )),
        }
    }
}

pub(super) fn lower_user_query(
    value: &str,
    field_context: Option<&QueryFieldRefIr>,
    default_index: Option<&str>,
    target_hits: u32,
    path: &str,
) -> Result<Query> {
    let tokens = tokenize_user_query(value, path)?;
    UserQueryParser {
        tokens: &tokens,
        index: 0,
        field_context,
        default_index,
        target_hits,
        path,
    }
    .parse()
}

pub(super) fn typed_term_text(value: &RequestValueIr, path: &str) -> Result<String> {
    match value {
        RequestValueIr::String(value) if !value.is_empty() => Ok(value.clone()),
        _ => Err(PortableError::unsupported(
            "typed_query_value_not_supported",
            Some(format!("{path}.term.value")),
            "typed term values must be non-empty strings",
        )),
    }
}

pub(super) fn lower_typed_phrase(
    children: &[QueryExprIr],
    field_context: Option<&QueryFieldRefIr>,
    language: searchkernel_vespa_engine::LinguisticsOptions,
    path: &str,
) -> Result<Query> {
    let field = typed_query_field(field_context, path)?;
    let mut terms = Vec::new();
    for (index, child) in children.iter().enumerate() {
        collect_typed_phrase_terms(
            child,
            field_context,
            &mut terms,
            &format!("{path}.phrase[{index}]"),
        )?;
    }
    if terms.is_empty() {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(format!("{path}.phrase")),
            "phrase query requires at least one term",
        ));
    }
    let text = terms.join(" ");
    let analyzed = searchkernel_vespa_engine::Linguistics
        .analyze_query(&text, language)
        .terms
        .len();
    if analyzed == 0 {
        return Err(PortableError::invalid(
            "empty_typed_query",
            Some(format!("{path}.phrase")),
            "phrase query must analyze to at least one term",
        ));
    }
    Ok(Query::phrase(field, text))
}

pub(super) fn collect_typed_phrase_terms(
    expression: &QueryExprIr,
    field_context: Option<&QueryFieldRefIr>,
    terms: &mut Vec<String>,
    path: &str,
) -> Result<()> {
    match &expression.kind {
        QueryExprKindIr::Term { value } => terms.push(typed_term_text(value, path)?),
        QueryExprKindIr::Contains { field, item } => {
            typed_query_field(Some(field), &format!("{path}.contains.field"))?;
            if field_context.is_some_and(|context| context.path != field.path) {
                return Err(PortableError::unsupported(
                    "typed_query_phrase_field_mismatch",
                    Some(path),
                    "all phrase terms must target the same field",
                ));
            }
            collect_typed_phrase_terms(item, Some(field), terms, &format!("{path}.contains.item"))?;
        }
        QueryExprKindIr::Phrase(children) => {
            for (index, child) in children.iter().enumerate() {
                collect_typed_phrase_terms(
                    child,
                    field_context,
                    terms,
                    &format!("{path}.phrase[{index}]"),
                )?;
            }
        }
        _ => {
            return Err(PortableError::unsupported(
                "typed_query_phrase_child_not_supported",
                Some(path),
                "phrase children must be term, contains, or phrase nodes",
            ));
        }
    }
    Ok(())
}
