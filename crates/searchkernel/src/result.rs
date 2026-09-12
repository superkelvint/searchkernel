use crate::document::DocumentId;
use crate::error::{Error, Result};
use searchkernel_core::{self as core, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotalHitsRelation {
    Exact,
    LowerBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult<T> {
    pub hits: Vec<Hit<T>>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    pub document: T,
    pub highlights: Vec<HighlightResult>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacetResult {
    pub field: String,
    pub values: Vec<FacetValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FacetValue {
    pub value: serde_json::Value,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

pub(crate) fn search_result<T: serde::de::DeserializeOwned>(
    result: core::SearchResult,
) -> Result<SearchResult<T>> {
    let hits = result
        .hits
        .into_iter()
        .map(hit)
        .collect::<Result<Vec<_>>>()?;
    let facets = result
        .facets
        .into_iter()
        .map(facet)
        .collect::<Result<Vec<_>>>()?;
    Ok(SearchResult {
        hits,
        total_hits: total_hits(result.total_hits),
        facets,
    })
}

pub(crate) fn document<T: serde::de::DeserializeOwned>(
    value: core::DocumentResult,
) -> Result<(DocumentId, T)> {
    let id = DocumentId::from(value.id);
    let object = fields_to_json(value.fields)?;
    let document = serde_json::from_value(object)
        .map_err(|error| Error::Deserialization(error.to_string()))?;
    Ok((id, document))
}

fn hit<T: serde::de::DeserializeOwned>(value: core::Hit) -> Result<Hit<T>> {
    let document = serde_json::from_value(fields_to_json(value.fields)?)
        .map_err(|error| Error::Deserialization(error.to_string()))?;
    Ok(Hit {
        id: DocumentId::from(value.id),
        score: value.score,
        document,
        highlights: value.highlights.into_iter().map(highlight).collect(),
    })
}

fn total_hits(value: core::TotalHits) -> TotalHits {
    TotalHits {
        value: value.value,
        relation: match value.relation {
            core::TotalHitsRelation::Exact => TotalHitsRelation::Exact,
            core::TotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
        },
    }
}

fn facet(value: core::FacetResult) -> Result<FacetResult> {
    Ok(FacetResult {
        field: value.field,
        values: value
            .values
            .into_iter()
            .map(|value| {
                Ok(FacetValue {
                    value: json_value(value.value)?,
                    count: value.count,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn highlight(value: core::HighlightResult) -> HighlightResult {
    HighlightResult {
        field: value.field,
        fragments: value
            .fragments
            .into_iter()
            .map(|fragment| HighlightFragment {
                text: fragment.text,
                matches: fragment
                    .matches
                    .into_iter()
                    .map(|span| HighlightSpan {
                        start_byte: span.start_byte,
                        end_byte: span.end_byte,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn fields_to_json(fields: std::collections::BTreeMap<String, Value>) -> Result<serde_json::Value> {
    let mut object = serde_json::Map::new();
    for (name, value) in fields {
        object.insert(name, json_value(value)?);
    }
    Ok(serde_json::Value::Object(object))
}

fn json_value(value: Value) -> Result<serde_json::Value> {
    Ok(match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(value) => serde_json::Value::Bool(value),
        Value::Int64(value) => serde_json::Value::Number(value.into()),
        Value::UInt64(value) => serde_json::Value::Number(value.into()),
        Value::Float64(value) => serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .ok_or_else(|| {
                Error::infrastructure("backend returned a non-finite floating-point value")
            })?,
        Value::String(value) => serde_json::Value::String(value),
        Value::Bytes(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(|value| serde_json::Value::Number(value.into()))
                .collect(),
        ),
        Value::List(values) => serde_json::Value::Array(
            values
                .into_iter()
                .map(json_value)
                .collect::<Result<Vec<_>>>()?,
        ),
        Value::Object(values) => {
            let mut object = serde_json::Map::new();
            for (name, value) in values {
                object.insert(name, json_value(value)?);
            }
            serde_json::Value::Object(object)
        }
    })
}
