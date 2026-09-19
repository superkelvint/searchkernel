use crate::document::DocumentId;
use crate::error::{Error, Result};
use searchkernel_ir::{self as core, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
/// Describes whether a hit count is exact or a lower bound.
pub enum TotalHitsRelation {
    Unspecified,
    Exact,
    LowerBound,
    Approximate,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// The number of matching documents and its accuracy.
#[non_exhaustive]
pub struct TotalHits {
    pub value: u64,
    pub relation: TotalHitsRelation,
}

#[derive(Clone, Debug, PartialEq)]
/// Search hits, total-hit information, and facet values.
#[non_exhaustive]
pub struct SearchResult<T> {
    pub hits: Vec<Hit<T>>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
}

#[derive(Clone, Debug, PartialEq)]
/// One matching document.
#[non_exhaustive]
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    pub document: T,
    pub highlights: Vec<HighlightResult>,
}

#[derive(Clone, Debug, PartialEq)]
/// Facet values for one field.
#[non_exhaustive]
pub struct FacetResult {
    pub field: String,
    pub values: Vec<FacetValue>,
}

#[derive(Clone, Debug, PartialEq)]
/// One distinct facet value and its document count.
#[non_exhaustive]
pub struct FacetValue {
    pub value: serde_json::Value,
    pub count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Highlighted fragments for one field.
#[non_exhaustive]
pub struct HighlightResult {
    pub field: String,
    pub fragments: Vec<HighlightFragment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// One piece of highlighted text and its matching spans.
#[non_exhaustive]
pub struct HighlightFragment {
    pub text: String,
    pub matches: Vec<HighlightSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A byte range inside a highlighted fragment.
#[non_exhaustive]
pub struct HighlightSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

pub(crate) fn search_result<T: serde::de::DeserializeOwned>(
    result: core::SearchResult,
    requested_facets: &[&str],
) -> Result<SearchResult<T>> {
    let mut hits = Vec::new();
    let mut facets = Vec::new();
    project_root(result.root, requested_facets, &mut hits, &mut facets)?;
    Ok(SearchResult {
        hits,
        total_hits: total_hits(result.total_hits),
        facets,
    })
}

/// Project only the immediate children of the canonical search root into the
/// legacy public result shape. Nested hits belong to their result groups and
/// must not be promoted into `SearchResult.hits`.
fn project_root<T: serde::de::DeserializeOwned>(
    root: core::ResultNode,
    requested_facets: &[&str],
    hits: &mut Vec<Hit<T>>,
    facets: &mut Vec<FacetResult>,
) -> Result<()> {
    match root {
        core::ResultNode::Hit(hit) => hits.push(convert_hit(hit)?),
        core::ResultNode::Group(group) => {
            for child in group.children {
                match child {
                    core::ResultNode::Hit(hit) => hits.push(convert_hit(hit)?),
                    core::ResultNode::Group(group) => {
                        if let Some(facet) = convert_facet_group(&group, requested_facets)? {
                            facets.push(facet);
                        }
                    }
                }
            }
        }
    }
    Ok(())
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
            core::TotalHitsRelation::Unspecified => TotalHitsRelation::Unspecified,
            core::TotalHitsRelation::Exact => TotalHitsRelation::Exact,
            core::TotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
            core::TotalHitsRelation::Approximate => TotalHitsRelation::Approximate,
            core::TotalHitsRelation::Unknown => TotalHitsRelation::Unknown,
        },
    }
}

fn convert_facet_group(
    group: &core::ResultGroup,
    requested_facets: &[&str],
) -> Result<Option<FacetResult>> {
    let Some(field) = group.id.as_ref() else {
        return Ok(None);
    };
    // Facets are a compatibility projection, not a general interpretation of
    // arbitrary result groups. Only a group corresponding to a facet the
    // caller actually requested may enter this legacy shape.
    if !requested_facets.iter().any(|requested| *requested == field) {
        return Ok(None);
    }
    if group.label.as_deref() != Some(field)
        || group.value.is_some()
        || !group.fields.is_empty()
        || !group.continuations.is_empty()
        || group.source.is_some()
        || !group.types.is_empty()
        || group.score.is_some()
        || group.limits.is_some()
        || !group.metadata.is_empty()
    {
        return Ok(None);
    }
    let mut values = Vec::with_capacity(group.children.len());
    for child in &group.children {
        let core::ResultNode::Group(child) = child else {
            return Ok(None);
        };
        if child.id.is_some()
            || child.label.is_some()
            || !child.children.is_empty()
            || !child.continuations.is_empty()
            || child.source.is_some()
            || !child.types.is_empty()
            || child.score.is_some()
            || child.limits.is_some()
            || !child.metadata.is_empty()
        {
            return Ok(None);
        }
        let Some(value) = child.value.clone() else {
            return Ok(None);
        };
        let Some(core::Value::UInt64(count)) = child.fields.get("count") else {
            return Ok(None);
        };
        values.push(FacetValue {
            value: json_value(value)?,
            count: *count,
        });
    }
    Ok(Some(FacetResult {
        field: field.clone(),
        values,
    }))
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
        Value::Tensor(_) => {
            return Err(Error::infrastructure(
                "backend returned a tensor value unsupported by the public JSON result projection",
            ))
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
            root: core::ResultNode::Group(core::ResultGroup {
                children: vec![
                    core::ResultNode::Hit(core::Hit {
                        id: "doc-1".into(),
                        score: Some(1.5),
                        fields: BTreeMap::from([
                            ("title".into(), Value::String("hello".into())),
                            ("bytes".into(), Value::Bytes(vec![1, 255])),
                            (
                                "nested".into(),
                                Value::Object(BTreeMap::from([(
                                    "enabled".into(),
                                    Value::Bool(true),
                                )])),
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
                        ..Default::default()
                    }),
                    core::ResultNode::Group(core::ResultGroup {
                        id: Some("brand".into()),
                        label: Some("brand".into()),
                        children: vec![core::ResultNode::Group(core::ResultGroup {
                            value: Some(Value::UInt64(2)),
                            fields: BTreeMap::from([("count".into(), Value::UInt64(3))]),
                            ..core::ResultGroup::root(vec![])
                        })],
                        ..core::ResultGroup::root(vec![])
                    }),
                ],
                ..core::ResultGroup::root(vec![])
            }),
            total_hits: core::TotalHits {
                value: 7,
                relation: core::TotalHitsRelation::LowerBound,
            },
            coverage: None,
            timing: None,
            errors: vec![],
            trace: None,
            metadata: BTreeMap::new(),
        };

        let converted: SearchResult<serde_json::Value> = search_result(result, &["brand"]).unwrap();
        assert_eq!(converted.total_hits.value, 7);
        assert_eq!(converted.total_hits.relation, TotalHitsRelation::LowerBound);
        assert_eq!(converted.hits.len(), 1);
        assert_eq!(converted.hits[0].id.as_str(), "doc-1");
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
    fn result_conversion_does_not_promote_nested_group_hits() {
        let result = core::SearchResult {
            root: core::ResultNode::Group(core::ResultGroup {
                children: vec![core::ResultNode::Group(core::ResultGroup {
                    id: Some("group".into()),
                    children: vec![core::ResultNode::Hit(core::Hit {
                        id: "nested-hit".into(),
                        ..Default::default()
                    })],
                    ..core::ResultGroup::root(vec![])
                })],
                ..core::ResultGroup::root(vec![])
            }),
            total_hits: core::TotalHits {
                value: 1,
                relation: core::TotalHitsRelation::Exact,
            },
            coverage: None,
            timing: None,
            errors: Vec::new(),
            trace: None,
            metadata: BTreeMap::new(),
        };

        let converted: SearchResult<serde_json::Value> = search_result(result, &[]).unwrap();
        assert!(converted.hits.is_empty());
        assert!(converted.facets.is_empty());
    }

    #[test]
    fn result_conversion_rejects_nonfinite_values_and_bad_documents() {
        let nonfinite = core::SearchResult {
            root: core::ResultNode::Group(core::ResultGroup {
                children: vec![core::ResultNode::Group(core::ResultGroup {
                    id: Some("score".into()),
                    label: Some("score".into()),
                    children: vec![core::ResultNode::Group(core::ResultGroup {
                        value: Some(Value::Float64(f64::INFINITY)),
                        fields: BTreeMap::from([("count".into(), Value::UInt64(1))]),
                        ..core::ResultGroup::root(vec![])
                    })],
                    ..core::ResultGroup::root(vec![])
                })],
                ..core::ResultGroup::root(vec![])
            }),
            total_hits: core::TotalHits {
                value: 0,
                relation: core::TotalHitsRelation::Exact,
            },
            coverage: None,
            timing: None,
            errors: vec![],
            trace: None,
            metadata: BTreeMap::new(),
        };
        assert!(matches!(
            search_result::<serde_json::Value>(nonfinite, &["score"]),
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
