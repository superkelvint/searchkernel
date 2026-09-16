use crate::document::DocumentId;
use crate::error::{Error, Result};
use searchkernel_ir::{self as core, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
/// Describes whether a hit count is exact or a lower bound.
pub enum TotalHitsRelation {
    Exact,
    LowerBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The number of matching documents and its accuracy.
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

#[derive(Clone, Debug, PartialEq)]
/// Search hits, total-hit information, and facet values.
pub struct SearchResult<T> {
    pub hits: Vec<Hit<T>>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
}

#[derive(Clone, Debug, PartialEq)]
/// One matching document.
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    pub document: T,
    pub highlights: Vec<HighlightResult>,
}

#[derive(Clone, Debug, PartialEq)]
/// Facet values for one field.
pub struct FacetResult {
    pub field: String,
    pub values: Vec<FacetValue>,
}

#[derive(Clone, Debug, PartialEq)]
/// One distinct facet value and its document count.
pub struct FacetValue {
    pub value: serde_json::Value,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Highlighted fragments for one field.
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One piece of highlighted text and its matching spans.
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A byte range inside a highlighted fragment.
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
        .map(convert_hit)
        .collect::<Result<Vec<_>>>()?;
    let facets = result
        .facets
        .into_iter()
        .map(convert_facet)
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

fn convert_hit<T: serde::de::DeserializeOwned>(hit: core::Hit) -> Result<Hit<T>> {
    let document = serde_json::from_value(fields_to_json(hit.fields)?)
        .map_err(|error| Error::Deserialization(error.to_string()))?;
    Ok(Hit {
        id: DocumentId::from(hit.id),
        score: hit.score,
        document,
        highlights: hit.highlights.into_iter().map(convert_highlight).collect(),
    })
}

fn total_hits(total_hits: core::TotalHits) -> TotalHits {
    TotalHits {
        value: total_hits.value,
        relation: match total_hits.relation {
            core::TotalHitsRelation::Exact => TotalHitsRelation::Exact,
            core::TotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
        },
    }
}

fn convert_facet(facet: core::FacetResult) -> Result<FacetResult> {
    Ok(FacetResult {
        field: facet.field,
        values: facet
            .values
            .into_iter()
            .map(|facet_value| {
                Ok(FacetValue {
                    value: json_value(facet_value.value)?,
                    count: facet_value.count,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn convert_highlight(highlight: core::HighlightResult) -> HighlightResult {
    HighlightResult {
        field: highlight.field,
        fragments: highlight
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
        Value::Bool(boolean) => serde_json::Value::Bool(boolean),
        Value::Int64(integer) => serde_json::Value::Number(integer.into()),
        Value::UInt64(integer) => serde_json::Value::Number(integer.into()),
        Value::Float64(number) => serde_json::Number::from_f64(number)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn result_conversion_preserves_totals_facets_highlights_and_nested_values() {
        let result = core::SearchResult {
            hits: vec![core::Hit {
                id: "doc-1".into(),
                score: Some(1.5),
                fields: BTreeMap::from([
                    ("title".into(), Value::String("hello".into())),
                    ("bytes".into(), Value::Bytes(vec![1, 255])),
                    (
                        "nested".into(),
                        Value::Object(BTreeMap::from([("enabled".into(), Value::Bool(true))])),
                    ),
                ]),
                highlights: vec![core::HighlightResult {
                    field: "title".into(),
                    fragments: vec![core::HighlightFragment {
                        text: "<em>hello</em>".into(),
                        matches: vec![core::HighlightSpan {
                            start_byte: 0,
                            end_byte: 5,
                        }],
                    }],
                }],
            }],
            total_hits: core::TotalHits {
                value: 7,
                relation: core::TotalHitsRelation::LowerBound,
            },
            facets: vec![core::FacetResult {
                field: "brand".into(),
                values: vec![core::FacetValue {
                    value: Value::UInt64(2),
                    count: 3,
                }],
            }],
        };

        let converted: SearchResult<serde_json::Value> = search_result(result).unwrap();
        assert_eq!(converted.total_hits.value, 7);
        assert_eq!(converted.total_hits.relation, TotalHitsRelation::LowerBound);
        assert_eq!(converted.hits[0].document["nested"]["enabled"], true);
        assert_eq!(
            converted.hits[0].document["bytes"],
            serde_json::json!([1, 255])
        );
        assert_eq!(
            converted.hits[0].highlights[0].fragments[0].matches[0].end_byte,
            5
        );
        assert_eq!(converted.facets[0].values[0].value, serde_json::json!(2));
    }

    #[test]
    fn result_conversion_rejects_nonfinite_values_and_bad_documents() {
        let nonfinite = core::SearchResult {
            hits: vec![],
            total_hits: core::TotalHits {
                value: 0,
                relation: core::TotalHitsRelation::Exact,
            },
            facets: vec![core::FacetResult {
                field: "score".into(),
                values: vec![core::FacetValue {
                    value: Value::Float64(f64::INFINITY),
                    count: 1,
                }],
            }],
        };
        assert!(matches!(
            search_result::<serde_json::Value>(nonfinite),
            Err(Error::Infrastructure(message)) if message.contains("non-finite")
        ));

        let bad_document = core::DocumentResult {
            id: "doc-2".into(),
            fields: BTreeMap::from([(
                String::from("count"),
                Value::String("not an integer".into()),
            )]),
        };
        assert!(matches!(
            document::<i64>(bad_document),
            Err(Error::Deserialization(_))
        ));
    }
}
