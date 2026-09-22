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

/// The canonical recursive result tree exposed by Search Kernel.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResultNode {
    Hit(ResultHit),
    Group(ResultGroup),
}

/// A hit stored in the canonical result tree.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultHit {
    pub id: DocumentId,
    pub relevance: Option<f64>,
    pub score: Option<f64>,
    pub fields: std::collections::BTreeMap<String, serde_json::Value>,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub match_features: std::collections::BTreeMap<String, serde_json::Value>,
    pub summary_features: std::collections::BTreeMap<String, serde_json::Value>,
    pub rank_features: std::collections::BTreeMap<String, serde_json::Value>,
    pub sort_values: Vec<serde_json::Value>,
}

/// Limits attached to a canonical result group.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultLimits {
    pub from: Option<serde_json::Value>,
    pub to: Option<serde_json::Value>,
}

/// A canonical recursive result group.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultGroup {
    pub id: Option<String>,
    pub label: Option<String>,
    pub value: Option<serde_json::Value>,
    pub fields: std::collections::BTreeMap<String, serde_json::Value>,
    pub children: Vec<ResultNode>,
    pub continuations: std::collections::BTreeMap<String, String>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub relevance: Option<f64>,
    pub limits: Option<ResultLimits>,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
/// Search hits, total-hit information, and facet values.
#[non_exhaustive]
pub struct SearchResult<T> {
    pub root: ResultNode,
    pub hits: Vec<Hit<T>>,
    pub total_hits: TotalHits,
    pub facets: Vec<FacetResult>,
    pub groups: Vec<GroupingResult<T>>,
    pub aggregations: std::collections::BTreeMap<String, serde_json::Value>,
    pub coverage: Option<Coverage>,
    pub timing: Option<SearchTiming>,
    pub issues: Vec<ResultIssue>,
    pub trace: Option<serde_json::Value>,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
/// One matching document.
#[non_exhaustive]
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    pub document: T,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    /// Backward-compatible alias of `match_features`.
    pub features: std::collections::BTreeMap<String, FeatureSet>,
    pub match_features: std::collections::BTreeMap<String, FeatureValue>,
    pub summary_features: std::collections::BTreeMap<String, FeatureValue>,
    pub rank_features: std::collections::BTreeMap<String, FeatureValue>,
    pub sort_values: Vec<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureSet {
    Value(FeatureValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureValue {
    Double(f64),
    Data(Vec<u8>),
    /// Canonical non-binary feature values represented without debug-string coercion.
    Json(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingResult<T = serde_json::Value> {
    pub key: Option<serde_json::Value>,
    pub aggregations: std::collections::BTreeMap<String, serde_json::Value>,
    pub groups: Vec<GroupingResult<T>>,
    pub hits: Vec<Hit<T>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoverageDegraded {
    pub match_phase: Option<bool>,
    pub timeout: Option<bool>,
    pub adaptive_timeout: Option<bool>,
    pub ann_timeout: Option<bool>,
    pub non_ideal_state: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Coverage {
    pub coverage_percent: Option<f64>,
    pub documents: Option<u64>,
    pub full: Option<bool>,
    pub nodes: Option<u32>,
    pub results: Option<u64>,
    pub results_full: Option<u64>,
    pub degraded: Option<CoverageDegraded>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultIssue {
    pub code: i64,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
    pub stack_trace: Option<String>,
    pub transient: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
/// Backend timing values, expressed in portable seconds.
pub struct SearchTiming {
    pub query_time_seconds: Option<f64>,
    pub search_time_seconds: Option<f64>,
    pub summary_fetch_time_seconds: Option<f64>,
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
/// Facet values for one field.
#[non_exhaustive]
pub struct FacetResult {
    pub field: String,
    pub name: String,
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
    requested_facets: &[(&str, &str)],
) -> Result<SearchResult<T>> {
    let root = convert_result_node(result.root.clone())?;
    let mut hits = Vec::new();
    let mut facets = Vec::new();
    let mut groups = Vec::new();
    project_root(
        result.root,
        requested_facets,
        &mut hits,
        &mut facets,
        &mut groups,
    )?;
    let aggregations = match &root {
        ResultNode::Group(group) => group.fields.clone(),
        ResultNode::Hit(_) => std::collections::BTreeMap::new(),
    };
    Ok(SearchResult {
        root,
        hits,
        total_hits: total_hits(result.total_hits),
        facets,
        groups,
        aggregations,
        coverage: result.coverage.map(coverage),
        timing: result.timing.map(search_timing).transpose()?,
        issues: result
            .errors
            .into_iter()
            .map(result_issue)
            .collect::<Result<Vec<_>>>()?,
        trace: result.trace.map(json_value).transpose()?,
        metadata: fields_to_json(result.metadata)?
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    })
}

/// Project only the immediate children of the canonical search root into the
/// legacy public result shape. Nested hits belong to their result groups and
/// must not be promoted into `SearchResult.hits`.
fn project_root<T: serde::de::DeserializeOwned>(
    root: core::ResultNode,
    requested_facets: &[(&str, &str)],
    hits: &mut Vec<Hit<T>>,
    facets: &mut Vec<FacetResult>,
    groups: &mut Vec<GroupingResult<T>>,
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
                        } else {
                            groups.push(convert_group(group)?);
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
    let match_features = convert_feature_map(hit.match_features)?;
    let features = match_features
        .iter()
        .map(|(name, value)| (name.clone(), FeatureSet::Value(value.clone())))
        .collect();
    Ok(Hit {
        id: DocumentId::from(hit.id),
        score: hit.score,
        document,
        highlights: hit.highlights.into_iter().map(convert_highlight).collect(),
        source: hit.source,
        types: hit.types,
        features,
        match_features,
        summary_features: convert_feature_map(hit.summary_features)?,
        rank_features: convert_feature_map(hit.rank_features)?,
        sort_values: hit
            .sort_values
            .into_iter()
            .map(json_value)
            .collect::<Result<Vec<_>>>()?,
    })
}

fn convert_feature_map(
    values: std::collections::BTreeMap<String, core::Value>,
) -> Result<std::collections::BTreeMap<String, FeatureValue>> {
    values
        .into_iter()
        .map(|(name, value)| convert_feature_value(value).map(|value| (name, value)))
        .collect()
}

fn convert_feature_value(value: core::Value) -> Result<FeatureValue> {
    Ok(match value {
        core::Value::Float64(value) => FeatureValue::Double(value),
        core::Value::Bytes(value) => FeatureValue::Data(value),
        other => FeatureValue::Json(json_value(other)?),
    })
}

fn convert_result_node(node: core::ResultNode) -> Result<ResultNode> {
    match node {
        core::ResultNode::Hit(hit) => Ok(ResultNode::Hit(convert_result_hit(hit)?)),
        core::ResultNode::Group(group) => Ok(ResultNode::Group(convert_result_group(group)?)),
    }
}

fn convert_result_hit(hit: core::Hit) -> Result<ResultHit> {
    Ok(ResultHit {
        id: DocumentId::from(hit.id),
        relevance: hit.score,
        score: hit.score,
        fields: json_map(hit.fields)?,
        highlights: hit.highlights.into_iter().map(convert_highlight).collect(),
        source: hit.source,
        types: hit.types,
        match_features: json_map(hit.match_features)?,
        summary_features: json_map(hit.summary_features)?,
        rank_features: json_map(hit.rank_features)?,
        sort_values: hit.sort_values.into_iter().map(json_value).collect::<Result<Vec<_>>>()?,
    })
}

fn convert_result_group(group: core::ResultGroup) -> Result<ResultGroup> {
    Ok(ResultGroup {
        id: group.id,
        label: group.label,
        value: group.value.map(json_value).transpose()?,
        fields: json_map(group.fields)?,
        children: group.children.into_iter().map(convert_result_node).collect::<Result<Vec<_>>>()?,
        continuations: group.continuations,
        source: group.source,
        types: group.types,
        relevance: group.score,
        limits: group.limits.map(|limits| {
            Ok(ResultLimits {
                from: limits.from.map(json_value).transpose()?,
                to: limits.to.map(json_value).transpose()?,
            })
        }).transpose()?,
        metadata: json_map(group.metadata)?,
    })
}

fn convert_group<T: serde::de::DeserializeOwned>(
    group: core::ResultGroup,
) -> Result<GroupingResult<T>> {
    let mut nested = Vec::new();
    let mut hits = Vec::new();
    for child in group.children {
        match child {
            core::ResultNode::Group(child) => nested.push(convert_group(child)?),
            core::ResultNode::Hit(hit) => hits.push(convert_hit(hit)?),
        }
    }
    let aggregations = group
        .fields
        .into_iter()
        .map(|(name, value)| json_value(value).map(|value| (name, value)))
        .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
    Ok(GroupingResult {
        key: group.value.map(json_value).transpose()?,
        aggregations,
        groups: nested,
        hits,
    })
}

fn coverage(value: core::Coverage) -> Coverage {
    Coverage {
        coverage_percent: value.coverage_percent,
        documents: value.documents,
        full: value.full,
        nodes: value.nodes,
        results: value.results,
        results_full: value.results_full,
        degraded: value.degraded.map(|value| CoverageDegraded {
            match_phase: value.match_phase,
            timeout: value.timeout,
            adaptive_timeout: value.adaptive_timeout,
            ann_timeout: value.ann_timeout,
            non_ideal_state: value.non_ideal_state,
        }),
    }
}

fn result_issue(value: core::SearchResultError) -> Result<ResultIssue> {
    Ok(ResultIssue {
        code: value.code,
        summary: value.summary,
        message: value.message,
        source: value.source,
        stack_trace: value.stack_trace,
        transient: value.transient,
        metadata: fields_to_json(value.metadata)?
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
    })
}

fn search_timing(value: core::SearchTiming) -> Result<SearchTiming> {
    Ok(SearchTiming {
        query_time_seconds: value.query_time_seconds,
        search_time_seconds: value.search_time_seconds,
        summary_fetch_time_seconds: value.summary_fetch_time_seconds,
        metadata: fields_to_json(value.metadata)?
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
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
    requested_facets: &[(&str, &str)],
) -> Result<Option<FacetResult>> {
    let Some(label) = group.label.as_deref() else {
        return Ok(None);
    };
    // Facets are a request-derived compatibility projection. Match the stable
    // canonical facet label rather than guessing arbitrary grouping trees.
    let Some((field, name)) = requested_facets
        .iter()
        .copied()
        .find(|(_, name)| *name == label)
    else {
        return Ok(None);
    };
    let optimized = group.id.as_deref() == Some(field);
    let generic = group.id.is_none();
    if (!optimized && !generic)
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
        let optimized_child = child.id.is_none() && child.label.is_none();
        let generic_child = child.id.as_deref() == Some("grouping-0")
            && child.label.as_deref() == Some(name);
        if (!optimized_child && !generic_child)
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
        field: field.to_owned(),
        name: name.to_owned(),
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

fn json_map(
    values: std::collections::BTreeMap<String, Value>,
) -> Result<std::collections::BTreeMap<String, serde_json::Value>> {
    values
        .into_iter()
        .map(|(name, value)| json_value(value).map(|value| (name, value)))
        .collect()
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
        Value::Tensor(tensor) => serde_json::json!({
            "$searchkernel_tensor": {
                "type_spec": tensor.type_spec,
                "dense": match tensor.payload {
                    searchkernel_ir::RequestTensorPayloadIr::Dense(values) => values,
                    _ => return Err(Error::search(crate::ErrorCode::Unsupported, "sparse and mixed tensors are not public JSON values")),
                }
            }
        }),
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

        let converted: SearchResult<serde_json::Value> = search_result(result, &[("brand", "brand")]).unwrap();
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
    fn result_conversion_preserves_named_generic_facets() {
        let result = core::SearchResult {
            root: core::ResultNode::Group(core::ResultGroup {
                children: vec![core::ResultNode::Group(core::ResultGroup {
                    id: None,
                    label: Some("brands".into()),
                    children: vec![core::ResultNode::Group(core::ResultGroup {
                        id: Some("grouping-0".into()),
                        label: Some("brands".into()),
                        value: Some(Value::String("acme".into())),
                        fields: BTreeMap::from([("count".into(), Value::UInt64(4))]),
                        ..core::ResultGroup::root(vec![])
                    })],
                    ..core::ResultGroup::root(vec![])
                })],
                ..core::ResultGroup::root(vec![])
            }),
            total_hits: core::TotalHits {
                value: 4,
                relation: core::TotalHitsRelation::Exact,
            },
            coverage: None,
            timing: None,
            errors: Vec::new(),
            trace: None,
            metadata: BTreeMap::new(),
        };

        let converted: SearchResult<serde_json::Value> =
            search_result(result, &[("brand", "brands")]).unwrap();
        assert_eq!(converted.facets.len(), 1);
        assert_eq!(converted.facets[0].field, "brand");
        assert_eq!(converted.facets[0].name, "brands");
        assert_eq!(converted.facets[0].values[0].value, serde_json::json!("acme"));
        assert_eq!(converted.facets[0].values[0].count, 4);
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
    fn result_conversion_preserves_envelope_timing_errors_trace_and_metadata() {
        let result = core::SearchResult {
            root: core::ResultNode::Group(core::ResultGroup::root(vec![])),
            total_hits: core::TotalHits {
                value: 0,
                relation: core::TotalHitsRelation::Exact,
            },
            coverage: None,
            timing: Some(core::SearchTiming {
                query_time_seconds: Some(0.25),
                search_time_seconds: Some(0.5),
                summary_fetch_time_seconds: None,
                metadata: BTreeMap::from([("timing".into(), Value::String("yes".into()))]),
            }),
            errors: vec![core::SearchResultError {
                code: 7,
                summary: Some("summary".into()),
                message: Some("message".into()),
                source: Some("source".into()),
                stack_trace: Some("stack".into()),
                transient: Some(true),
                metadata: BTreeMap::from([("error".into(), Value::String("yes".into()))]),
            }],
            trace: Some(Value::String("trace".into())),
            metadata: BTreeMap::from([("result".into(), Value::String("yes".into()))]),
        };

        let converted: SearchResult<serde_json::Value> = search_result(result, &[]).unwrap();
        assert_eq!(
            converted.timing.as_ref().unwrap().query_time_seconds,
            Some(0.25)
        );
        assert_eq!(converted.timing.as_ref().unwrap().metadata["timing"], "yes");
        assert_eq!(converted.issues[0].code, 7);
        assert_eq!(converted.issues[0].summary.as_deref(), Some("summary"));
        assert_eq!(converted.issues[0].message.as_deref(), Some("message"));
        assert_eq!(converted.issues[0].source.as_deref(), Some("source"));
        assert_eq!(converted.issues[0].stack_trace.as_deref(), Some("stack"));
        assert_eq!(converted.issues[0].transient, Some(true));
        assert_eq!(converted.issues[0].metadata["error"], "yes");
        assert_eq!(converted.trace, Some(serde_json::json!("trace")));
        assert_eq!(converted.metadata["result"], "yes");
    }

    #[test]
    fn result_conversion_preserves_complete_hit_metadata_and_feature_maps() {
        let hit = core::Hit {
            id: "doc-1".into(),
            score: Some(2.5),
            fields: BTreeMap::from([("title".into(), Value::String("hello".into()))]),
            source: Some("catalog".into()),
            types: vec!["product".into(), "featured".into()],
            match_features: BTreeMap::from([
                ("scalar".into(), Value::Float64(1.5)),
                ("bytes".into(), Value::Bytes(vec![1, 2])),
                (
                    "list".into(),
                    Value::List(vec![Value::Int64(1), Value::Bool(true)]),
                ),
            ]),
            summary_features: BTreeMap::from([(
                "summary".into(),
                Value::Object(BTreeMap::from([("ready".into(), Value::Bool(true))])),
            )]),
            rank_features: BTreeMap::from([("rank".into(), Value::UInt64(7))]),
            sort_values: vec![Value::String("a".into()), Value::UInt64(9)],
            highlights: vec![core::HighlightResult {
                field: "title".into(),
                fragments: vec![core::HighlightFragment {
                    text: "hello".into(),
                    matches: vec![core::HighlightSpan {
                        start_byte: 0,
                        end_byte: 5,
                    }],
                }],
            }],
        };
        let converted: Hit<serde_json::Value> = convert_hit(hit).unwrap();

        assert_eq!(converted.source.as_deref(), Some("catalog"));
        assert_eq!(converted.types, ["product", "featured"]);
        assert_eq!(
            converted.match_features["scalar"],
            FeatureValue::Double(1.5)
        );
        assert_eq!(
            converted.match_features["bytes"],
            FeatureValue::Data(vec![1, 2])
        );
        assert_eq!(
            converted.match_features["list"],
            FeatureValue::Json(serde_json::json!([1, true]))
        );
        assert_eq!(
            converted.summary_features["summary"],
            FeatureValue::Json(serde_json::json!({"ready": true}))
        );
        assert_eq!(
            converted.rank_features["rank"],
            FeatureValue::Json(serde_json::json!(7))
        );
        assert_eq!(
            converted.sort_values,
            [serde_json::json!("a"), serde_json::json!(9)]
        );
        assert_eq!(converted.highlights[0].fragments[0].matches[0].end_byte, 5);
    }

    #[test]
    fn nested_group_hits_use_the_same_hit_decoder() {
        let group = core::ResultGroup {
            children: vec![core::ResultNode::Hit(core::Hit {
                id: "nested-1".into(),
                score: Some(3.0),
                fields: BTreeMap::from([("title".into(), Value::String("nested".into()))]),
                source: Some("catalog".into()),
                types: vec!["product".into()],
                match_features: BTreeMap::from([(
                    "typed".into(),
                    Value::List(vec![Value::Int64(1), Value::Bool(true)]),
                )]),
                ..Default::default()
            })],
            ..core::ResultGroup::root(vec![])
        };

        let converted: GroupingResult<serde_json::Value> = convert_group(group).unwrap();
        assert_eq!(converted.hits.len(), 1);
        assert_eq!(converted.hits[0].id.as_str(), "nested-1");
        assert_eq!(converted.hits[0].source.as_deref(), Some("catalog"));
        assert_eq!(converted.hits[0].types, ["product"]);
        assert_eq!(converted.hits[0].document["title"], "nested");
        assert_eq!(
            converted.hits[0].match_features["typed"],
            FeatureValue::Json(serde_json::json!([1, true]))
        );
    }

    #[test]
    fn result_conversion_preserves_canonical_mixed_tree_and_group_metadata() {
        let result = core::SearchResult {
            root: core::ResultNode::Group(core::ResultGroup {
                id: Some("root".into()),
                label: Some("Root".into()),
                fields: BTreeMap::from([("count".into(), Value::UInt64(2))]),
                metadata: BTreeMap::from([("trace_id".into(), Value::String("r-1".into()))]),
                children: vec![
                    core::ResultNode::Hit(core::Hit {
                        id: "root-hit".into(),
                        source: Some("catalog".into()),
                        types: vec!["product".into()],
                        ..Default::default()
                    }),
                    core::ResultNode::Group(core::ResultGroup {
                        id: Some("nested".into()),
                        label: Some("Nested".into()),
                        continuations: BTreeMap::from([("next".into(), "token".into())]),
                        source: Some("group-source".into()),
                        types: vec!["bucket".into()],
                        score: Some(2.5),
                        limits: Some(core::ResultLimits {
                            from: Some(Value::UInt64(0)),
                            to: Some(Value::UInt64(10)),
                        }),
                        metadata: BTreeMap::from([("owner".into(), Value::Bool(true))]),
                        children: vec![core::ResultNode::Hit(core::Hit {
                            id: "nested-hit".into(),
                            ..Default::default()
                        })],
                        ..core::ResultGroup::root(vec![])
                    }),
                ],
                ..core::ResultGroup::root(vec![])
            }),
            total_hits: core::TotalHits {
                value: 2,
                relation: core::TotalHitsRelation::Exact,
            },
            coverage: None,
            timing: None,
            errors: Vec::new(),
            trace: None,
            metadata: BTreeMap::new(),
        };

        let converted: SearchResult<serde_json::Value> = search_result(result, &[]).unwrap();
        assert_eq!(converted.aggregations["count"], serde_json::json!(2));
        let ResultNode::Group(root) = converted.root else {
            panic!("canonical root must remain a result group");
        };
        assert_eq!(root.id.as_deref(), Some("root"));
        assert_eq!(root.fields["count"], serde_json::json!(2));
        assert_eq!(root.metadata["trace_id"], serde_json::json!("r-1"));
        assert_eq!(root.children.len(), 2);
        let ResultNode::Hit(root_hit) = &root.children[0] else { panic!("first root child must remain a hit"); };
        assert_eq!(root_hit.id.as_str(), "root-hit");
        assert_eq!(root_hit.source.as_deref(), Some("catalog"));
        let ResultNode::Group(nested) = &root.children[1] else { panic!("second root child must remain a group"); };
        assert_eq!(nested.continuations["next"], "token");
        assert_eq!(nested.source.as_deref(), Some("group-source"));
        assert_eq!(nested.types, ["bucket"]);
        assert_eq!(nested.relevance, Some(2.5));
        assert_eq!(nested.metadata["owner"], serde_json::json!(true));
        assert_eq!(nested.limits.as_ref().unwrap().to, Some(serde_json::json!(10)));
        assert_eq!(nested.children.len(), 1);
        let ResultNode::Hit(nested_hit) = &nested.children[0] else { panic!("nested hit must remain in the canonical tree"); };
        assert_eq!(nested_hit.id.as_str(), "nested-hit");
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
            search_result::<serde_json::Value>(nonfinite, &[("score", "score")]),
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
