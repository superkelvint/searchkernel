//! Primitive, request, and backend value representations.

use crate::error::{PortableError, Result};
use std::fmt;

macro_rules! validated_string_type {
    ($name:ident, $detail:literal, $description:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                if value.is_empty() {
                    return Err(PortableError::invalid(
                        $detail,
                        None::<String>,
                        concat!($description, " must not be empty"),
                    ));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldName(String);

impl FieldName {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let mut chars = value.chars();
        let valid = chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|character| character.is_ascii_alphanumeric() || character == '_');
        if !valid {
            return Err(PortableError::invalid(
                "invalid_field_name",
                None::<String>,
                "field name must match [A-Za-z][A-Za-z0-9_]*",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse_query_path(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if !value.is_empty()
            && value.split('.').all(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .is_some_and(|first| first.is_ascii_alphabetic())
                    && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
            })
        {
            Ok(Self(value))
        } else {
            Err(PortableError::invalid(
                "invalid_field_path",
                None::<String>,
                "field paths must contain identifier components separated by dots",
            ))
        }
    }
}

impl AsRef<str> for FieldName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

validated_string_type!(DocumentId, "invalid_document_id", "document ID");
validated_string_type!(IndexName, "invalid_index_name", "index name");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageHint(String);

impl LanguageHint {
    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.split('-').enumerate().all(|(index, part)| {
                let length = part.len();
                let length_valid = if index == 0 {
                    (2..=8).contains(&length)
                } else {
                    (1..=8).contains(&length)
                };
                length_valid
                    && part.is_ascii()
                    && if index == 0 {
                        part.bytes().all(|byte| byte.is_ascii_alphabetic())
                    } else {
                        part.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    }
            });
        if !valid {
            return Err(PortableError::invalid(
                "invalid_language",
                None::<String>,
                "language must be a basic BCP-47-style ASCII tag",
            ));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for LanguageHint {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarValueIr {
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldValueIr {
    Single(ScalarValueIr),
    Multi(Vec<ScalarValueIr>),
    /// An array of Vespa struct values represented by owned scalar members.
    Structured(Vec<std::collections::BTreeMap<String, ScalarValueIr>>),
    Position {
        latitude: f64,
        longitude: f64,
    },
    Predicate {
        features: std::collections::BTreeMap<String, String>,
    },
    /// Canonical Vespa predicate Slime JSON. The native adapter parses this
    /// representation; Rust does not evaluate predicate expressions.
    PredicateExpression {
        expression: String,
    },
    Tensor(RequestTensorIr),
}

/// The complete portable value domain used by request controls.  Document values
/// remain deliberately narrower while the native document ABI has that limit;
/// request properties and query inputs must not lose valid portable values for that
/// reason.
#[derive(Clone, Debug, PartialEq)]
pub enum RequestValueIr {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    NonFiniteFloat(RequestNonFiniteFloatIr),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<RequestValueIr>),
    Object(std::collections::BTreeMap<String, RequestValueIr>),
    Tensor(RequestTensorIr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestNonFiniteFloatIr {
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorIr {
    pub type_spec: String,
    pub payload: RequestTensorPayloadIr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RequestTensorPayloadIr {
    Dense(Vec<f64>),
    Sparse(Vec<RequestTensorCellIr>),
    Mixed(Vec<RequestTensorBlockIr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorCellIr {
    pub address: std::collections::BTreeMap<String, String>,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestTensorBlockIr {
    pub address: std::collections::BTreeMap<String, String>,
    pub values: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryInputIr {
    Literal(RequestValueIr),
    Expression(String),
}

/// Complete value representation used by owned backend results.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Object(std::collections::BTreeMap<String, Value>),
    Tensor(RequestTensorIr),
}

impl fmt::Display for FieldName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for IndexName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
