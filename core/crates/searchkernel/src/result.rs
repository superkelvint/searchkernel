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

/// One lossless value decoded from the canonical portable result contract.
///
/// `Float64` represents finite doubles. The explicit non-finite variant keeps
/// the `Value` union discriminant intact and avoids coercing NaN or infinities
/// into JSON strings or nulls.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResultValue {
    Null,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    NonFiniteFloat(NonFiniteFloat),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<ResultValue>),
    Object(std::collections::BTreeMap<String, ResultValue>),
    Tensor(ResultTensor),
}

/// The three non-finite scalar values allowed by the portable `Value` union.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NonFiniteFloat {
    NaN,
    PositiveInfinity,
    NegativeInfinity,
}

/// A portable result tensor, retaining its original payload encoding.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultTensor {
    pub type_spec: String,
    pub payload: ResultTensorPayload,
}

/// Tensor payload encodings supported by the portable result contract.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResultTensorPayload {
    Dense(Vec<f64>),
    Sparse(Vec<ResultTensorCell>),
    Mixed(Vec<ResultTensorBlock>),
}

/// One sparse tensor cell with its named dimension labels.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultTensorCell {
    pub address: std::collections::BTreeMap<String, String>,
    pub value: f64,
}

/// One dense block in a mixed tensor.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultTensorBlock {
    pub address: std::collections::BTreeMap<String, String>,
    pub values: Vec<f64>,
}

/// A hit stored in the canonical result tree.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultHit {
    pub id: DocumentId,
    pub relevance: Option<f64>,
    pub score: Option<f64>,
    pub fields: std::collections::BTreeMap<String, ResultValue>,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub match_features: std::collections::BTreeMap<String, ResultValue>,
    pub summary_features: std::collections::BTreeMap<String, ResultValue>,
    pub rank_features: std::collections::BTreeMap<String, ResultValue>,
    pub sort_values: Vec<ResultValue>,
}

/// Limits attached to a canonical result group.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultLimits {
    pub from: Option<ResultValue>,
    pub to: Option<ResultValue>,
}

/// A canonical recursive result group.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ResultGroup {
    pub id: Option<String>,
    pub label: Option<String>,
    pub value: Option<ResultValue>,
    pub fields: std::collections::BTreeMap<String, ResultValue>,
    pub children: Vec<ResultNode>,
    pub continuations: std::collections::BTreeMap<String, String>,
    pub source: Option<String>,
    pub types: Vec<String>,
    pub relevance: Option<f64>,
    pub limits: Option<ResultLimits>,
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
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
    pub aggregations: std::collections::BTreeMap<String, ResultValue>,
    pub coverage: Option<Coverage>,
    pub timing: Option<SearchTiming>,
    pub issues: Vec<ResultIssue>,
    pub trace: Option<ResultValue>,
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
}

#[derive(Clone, Debug, PartialEq)]
/// One matching document.
#[non_exhaustive]
pub struct Hit<T> {
    pub id: DocumentId,
    pub score: Option<f64>,
    /// Typed document projection. This is absent when the lossless portable
    /// fields cannot be represented through the JSON-backed projection path
    /// (currently non-finite scalar values). Ordinary serde deserialization
    /// failures for `T` are still returned as `Error::Deserialization`.
    pub document: Option<T>,
    /// Exact portable field values, including bytes and non-finite floats.
    pub fields: std::collections::BTreeMap<String, ResultValue>,
    /// Why the JSON-backed document projection was skipped. This is distinct
    /// from ordinary `T` deserialization failures, which remain errors.
    pub document_error: Option<String>,
    pub highlights: Vec<HighlightResult>,
    pub source: Option<String>,
    pub types: Vec<String>,
    /// Backward-compatible alias of `match_features`.
    pub features: std::collections::BTreeMap<String, FeatureSet>,
    pub match_features: std::collections::BTreeMap<String, FeatureValue>,
    pub summary_features: std::collections::BTreeMap<String, FeatureValue>,
    pub rank_features: std::collections::BTreeMap<String, FeatureValue>,
    pub sort_values: Vec<ResultValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureSet {
    Value(FeatureValue),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureValue {
    Double(f64),
    Data(Vec<u8>),
    /// Canonical non-double feature values, preserving portable value kinds.
    Value(ResultValue),
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupingResult<T = serde_json::Value> {
    pub key: Option<ResultValue>,
    pub aggregations: std::collections::BTreeMap<String, ResultValue>,
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
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
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
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResultIssue {
    pub code: i64,
    pub summary: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
    pub stack_trace: Option<String>,
    pub transient: Option<bool>,
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
}

#[derive(Clone, Debug, PartialEq)]
/// Backend timing values, expressed in portable seconds.
pub struct SearchTiming {
    pub query_time_seconds: Option<f64>,
    pub search_time_seconds: Option<f64>,
    pub summary_fetch_time_seconds: Option<f64>,
    pub metadata: std::collections::BTreeMap<String, ResultValue>,
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
    pub value: ResultValue,
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
        coverage: result.coverage.map(coverage).transpose()?,
        timing: result.timing.map(search_timing).transpose()?,
        issues: result
            .errors
            .into_iter()
            .map(result_issue)
            .collect::<Result<Vec<_>>>()?,
        trace: result.trace.map(result_value),
        metadata: result_value_map(result.metadata),
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
    let contains_non_finite_scalar = hit.fields.values().any(contains_non_finite_scalar);
    let fields = result_value_map(hit.fields.clone());
    let (document, document_error) = if contains_non_finite_scalar {
        (
            None,
            Some("document contains non-finite scalar values".to_owned()),
        )
    } else {
        let document = serde_json::from_value(fields_to_json(hit.fields)?)
            .map_err(|error| Error::Deserialization(error.to_string()))?;
        (Some(document), None)
    };
    let match_features = convert_feature_map(hit.match_features)?;
    let features = match_features
        .iter()
        .map(|(name, value)| (name.clone(), FeatureSet::Value(value.clone())))
        .collect();
    Ok(Hit {
        id: DocumentId::from(hit.id),
        score: hit.score,
        document,
        fields,
        document_error,
        highlights: hit.highlights.into_iter().map(convert_highlight).collect(),
        source: hit.source,
        types: hit.types,
        features,
        match_features,
        summary_features: convert_feature_map(hit.summary_features)?,
        rank_features: convert_feature_map(hit.rank_features)?,
        sort_values: hit.sort_values.into_iter().map(result_value).collect(),
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
        core::Value::Float64(value) if value.is_finite() => FeatureValue::Double(value),
        core::Value::Bytes(value) => FeatureValue::Data(value),
        other => FeatureValue::Value(result_value(other)),
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
        fields: result_value_map(hit.fields),
        highlights: hit.highlights.into_iter().map(convert_highlight).collect(),
        source: hit.source,
        types: hit.types,
        match_features: result_value_map(hit.match_features),
        summary_features: result_value_map(hit.summary_features),
        rank_features: result_value_map(hit.rank_features),
        sort_values: hit.sort_values.into_iter().map(result_value).collect(),
    })
}

fn convert_result_group(group: core::ResultGroup) -> Result<ResultGroup> {
    Ok(ResultGroup {
        id: group.id,
        label: group.label,
        value: group.value.map(result_value),
        fields: result_value_map(group.fields),
        children: group
            .children
            .into_iter()
            .map(convert_result_node)
            .collect::<Result<Vec<_>>>()?,
        continuations: group.continuations,
        source: group.source,
        types: group.types,
        relevance: group.score,
        limits: group.limits.map(|limits| ResultLimits {
            from: limits.from.map(result_value),
            to: limits.to.map(result_value),
        }),
        metadata: result_value_map(group.metadata),
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
    let aggregations = result_value_map(group.fields);
    Ok(GroupingResult {
        key: group.value.map(result_value),
        aggregations,
        groups: nested,
        hits,
    })
}

fn coverage(value: core::Coverage) -> Result<Coverage> {
    Ok(Coverage {
        coverage_percent: value.coverage_percent,
        documents: value.documents,
        full: value.full,
        nodes: value.nodes,
        results: value.results,
        results_full: value.results_full,
        degraded: value
            .degraded
            .map(|value| {
                Ok(CoverageDegraded {
                    match_phase: value.match_phase,
                    timeout: value.timeout,
                    adaptive_timeout: value.adaptive_timeout,
                    ann_timeout: value.ann_timeout,
                    non_ideal_state: value.non_ideal_state,
                    metadata: result_value_map(value.metadata),
                })
            })
            .transpose()?,
        metadata: result_value_map(value.metadata),
    })
}

fn result_issue(value: core::SearchResultError) -> Result<ResultIssue> {
    Ok(ResultIssue {
        code: value.code,
        summary: value.summary,
        message: value.message,
        source: value.source,
        stack_trace: value.stack_trace,
        transient: value.transient,
        metadata: result_value_map(value.metadata),
    })
}

fn search_timing(value: core::SearchTiming) -> Result<SearchTiming> {
    Ok(SearchTiming {
        query_time_seconds: value.query_time_seconds,
        search_time_seconds: value.search_time_seconds,
        summary_fetch_time_seconds: value.summary_fetch_time_seconds,
        metadata: result_value_map(value.metadata),
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
        let generic_child =
            child.id.as_deref() == Some("grouping-0") && child.label.as_deref() == Some(name);
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
        if child.fields.len() != 1 {
            return Ok(None);
        }
        let Some(core::Value::UInt64(count)) = child.fields.get("count") else {
            return Ok(None);
        };
        values.push(FacetValue {
            value: result_value(value),
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

fn contains_non_finite_scalar(value: &Value) -> bool {
    match value {
        Value::Float64(value) => !value.is_finite(),
        Value::List(values) => values.iter().any(contains_non_finite_scalar),
        Value::Object(values) => values.values().any(contains_non_finite_scalar),
        _ => false,
    }
}

fn result_value_map(
    values: std::collections::BTreeMap<String, Value>,
) -> std::collections::BTreeMap<String, ResultValue> {
    values
        .into_iter()
        .map(|(name, value)| (name, result_value(value)))
        .collect()
}

fn result_value(value: Value) -> ResultValue {
    match value {
        Value::Null => ResultValue::Null,
        Value::Bool(value) => ResultValue::Bool(value),
        Value::Int64(value) => ResultValue::Int64(value),
        Value::UInt64(value) => ResultValue::UInt64(value),
        Value::Float64(value) if value.is_finite() => ResultValue::Float64(value),
        Value::Float64(value) if value.is_nan() => ResultValue::NonFiniteFloat(NonFiniteFloat::NaN),
        Value::Float64(value) if value.is_sign_positive() => {
            ResultValue::NonFiniteFloat(NonFiniteFloat::PositiveInfinity)
        }
        Value::Float64(_) => ResultValue::NonFiniteFloat(NonFiniteFloat::NegativeInfinity),
        Value::String(value) => ResultValue::String(value),
        Value::Bytes(value) => ResultValue::Bytes(value),
        Value::List(values) => ResultValue::List(values.into_iter().map(result_value).collect()),
        Value::Object(values) => ResultValue::Object(result_value_map(values)),
        Value::Tensor(tensor) => {
            let payload = match tensor.payload {
                searchkernel_ir::RequestTensorPayloadIr::Dense(values) => {
                    ResultTensorPayload::Dense(values)
                }
                searchkernel_ir::RequestTensorPayloadIr::Sparse(cells) => {
                    ResultTensorPayload::Sparse(
                        cells
                            .into_iter()
                            .map(|cell| ResultTensorCell {
                                address: cell.address,
                                value: cell.value,
                            })
                            .collect(),
                    )
                }
                searchkernel_ir::RequestTensorPayloadIr::Mixed(blocks) => {
                    ResultTensorPayload::Mixed(
                        blocks
                            .into_iter()
                            .map(|block| ResultTensorBlock {
                                address: block.address,
                                values: block.values,
                            })
                            .collect(),
                    )
                }
            };
            ResultValue::Tensor(ResultTensor {
                type_spec: tensor.type_spec,
                payload,
            })
        }
    }
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
        Value::Tensor(tensor) => {
            let mut payload = serde_json::Map::new();
            payload.insert(
                "type_spec".to_owned(),
                serde_json::Value::String(tensor.type_spec),
            );
            match tensor.payload {
                searchkernel_ir::RequestTensorPayloadIr::Dense(values) => {
                    payload.insert("dense".to_owned(), serde_json::json!(values));
                }
                searchkernel_ir::RequestTensorPayloadIr::Sparse(cells) => {
                    payload.insert(
                        "sparse".to_owned(),
                        serde_json::Value::Array(
                            cells
                                .into_iter()
                                .map(|cell| {
                                    serde_json::json!({
                                        "address": tensor_address_json(cell.address),
                                        "value": cell.value,
                                    })
                                })
                                .collect(),
                        ),
                    );
                }
                searchkernel_ir::RequestTensorPayloadIr::Mixed(blocks) => {
                    payload.insert(
                        "mixed".to_owned(),
                        serde_json::Value::Array(
                            blocks
                                .into_iter()
                                .map(|block| {
                                    serde_json::json!({
                                        "address": tensor_address_json(block.address),
                                        "values": block.values,
                                    })
                                })
                                .collect(),
                        ),
                    );
                }
            }
            serde_json::json!({"$searchkernel_tensor": payload})
        }
    })
}

fn tensor_address_json(address: std::collections::BTreeMap<String, String>) -> serde_json::Value {
    // Keep the dimension-label entry list used by the FlatBuffers decoders in
    // the other SDKs, so one canonical tensor address has the same public shape.
    serde_json::Value::Array(
        address
            .into_iter()
            .map(|(dimension, label)| serde_json::json!({"dimension": dimension, "label": label}))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_testkit::{
        canonical_facet_result, canonical_result_fixtures, mixed_hit_and_group_children_result,
        non_facet_grouping_result, result_fixtures, sibling_groups_result,
        sparse_mixed_tensor_result,
    };
    use std::collections::BTreeMap;

    fn rv(value: core::Value) -> ResultValue {
        result_value(value)
    }

    fn assert_public_node_matches(expected: &core::ResultNode, actual: &ResultNode) {
        match (expected, actual) {
            (core::ResultNode::Hit(expected), ResultNode::Hit(actual)) => {
                assert_eq!(actual.id.as_str(), expected.id);
                assert_eq!(actual.relevance, expected.score);
                assert_eq!(actual.score, expected.score);
                assert_eq!(actual.fields, result_value_map(expected.fields.clone()));
                assert_eq!(actual.source, expected.source);
                assert_eq!(actual.types, expected.types);
                assert_eq!(
                    actual.match_features,
                    result_value_map(expected.match_features.clone())
                );
                assert_eq!(
                    actual.summary_features,
                    result_value_map(expected.summary_features.clone())
                );
                assert_eq!(
                    actual.rank_features,
                    result_value_map(expected.rank_features.clone())
                );
                assert_eq!(
                    actual.sort_values,
                    expected
                        .sort_values
                        .iter()
                        .cloned()
                        .map(result_value)
                        .collect::<Vec<_>>()
                );
                assert_eq!(actual.highlights.len(), expected.highlights.len());
                for (actual, expected) in actual.highlights.iter().zip(&expected.highlights) {
                    assert_eq!(actual.field, expected.field);
                    assert_eq!(actual.fragments.len(), expected.fragments.len());
                    for (actual, expected) in actual.fragments.iter().zip(&expected.fragments) {
                        assert_eq!(actual.text, expected.text);
                        assert_eq!(actual.matches.len(), expected.matches.len());
                        for (actual, expected) in actual.matches.iter().zip(&expected.matches) {
                            assert_eq!(actual.start_byte, expected.start_byte);
                            assert_eq!(actual.end_byte, expected.end_byte);
                        }
                    }
                }
            }
            (core::ResultNode::Group(expected), ResultNode::Group(actual)) => {
                assert_eq!(actual.id, expected.id);
                assert_eq!(actual.label, expected.label);
                assert_eq!(actual.value, expected.value.clone().map(result_value));
                assert_eq!(actual.fields, result_value_map(expected.fields.clone()));
                assert_eq!(actual.continuations, expected.continuations);
                assert_eq!(actual.source, expected.source);
                assert_eq!(actual.types, expected.types);
                assert_eq!(actual.relevance, expected.score);
                assert_eq!(actual.metadata, result_value_map(expected.metadata.clone()));
                assert_eq!(actual.children.len(), expected.children.len());
                for (expected, actual) in expected.children.iter().zip(&actual.children) {
                    assert_public_node_matches(expected, actual);
                }
                match (&expected.limits, &actual.limits) {
                    (None, None) => {}
                    (Some(expected), Some(actual)) => {
                        assert_eq!(actual.from, expected.from.clone().map(result_value));
                        assert_eq!(actual.to, expected.to.clone().map(result_value));
                    }
                    _ => panic!("public result changed limits presence"),
                }
            }
            _ => panic!("public result changed canonical node kind"),
        }
    }

    #[test]
    fn public_root_preserves_every_shared_canonical_fixture() {
        for (name, expected) in canonical_result_fixtures() {
            let expected_root = expected.root.clone();
            let actual: SearchResult<serde_json::Value> = search_result(expected, &[])
                .unwrap_or_else(|error| {
                    panic!("fixture {name} failed public conversion: {error:?}")
                });
            assert_public_node_matches(&expected_root, &actual.root);
        }
    }

    fn trace_result_value(value: &ResultValue) -> serde_json::Value {
        match value {
            ResultValue::Null => serde_json::Value::Null,
            ResultValue::Bool(value) => serde_json::Value::Bool(*value),
            ResultValue::Int64(value) => serde_json::Value::String(value.to_string()),
            ResultValue::UInt64(value) => serde_json::Value::String(value.to_string()),
            ResultValue::Float64(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(*value).expect("ResultValue::Float64 is finite"),
            ),
            ResultValue::NonFiniteFloat(NonFiniteFloat::NaN) => {
                serde_json::Value::String("nan".into())
            }
            ResultValue::NonFiniteFloat(NonFiniteFloat::PositiveInfinity) => {
                serde_json::Value::String("+inf".into())
            }
            ResultValue::NonFiniteFloat(NonFiniteFloat::NegativeInfinity) => {
                serde_json::Value::String("-inf".into())
            }
            ResultValue::String(value) => serde_json::Value::String(value.clone()),
            ResultValue::Bytes(values) => serde_json::Value::Array(
                values
                    .iter()
                    .map(|value| serde_json::Value::String(value.to_string()))
                    .collect(),
            ),
            ResultValue::List(values) => {
                serde_json::Value::Array(values.iter().map(trace_result_value).collect())
            }
            ResultValue::Object(values) => trace_result_map(values),
            ResultValue::Tensor(tensor) => {
                let address = |values: &std::collections::BTreeMap<String, String>| {
                    serde_json::Value::Array(
                        values
                            .iter()
                            .map(|(dimension, label)| {
                                serde_json::json!({"dimension": dimension, "label": label})
                            })
                            .collect(),
                    )
                };
                let finite = |value: f64| {
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(value)
                            .expect("portable tensor values are finite"),
                    )
                };
                let mut payload = serde_json::Map::new();
                payload.insert(
                    "type_spec".into(),
                    serde_json::Value::String(tensor.type_spec.clone()),
                );
                match &tensor.payload {
                    ResultTensorPayload::Dense(values) => {
                        payload.insert(
                            "dense".into(),
                            serde_json::Value::Array(values.iter().copied().map(finite).collect()),
                        );
                    }
                    ResultTensorPayload::Sparse(cells) => {
                        payload.insert(
                            "sparse".into(),
                            serde_json::Value::Array(
                                cells
                                    .iter()
                                    .map(|cell| {
                                        serde_json::json!({
                                            "address": address(&cell.address),
                                            "value": finite(cell.value),
                                        })
                                    })
                                    .collect(),
                            ),
                        );
                    }
                    ResultTensorPayload::Mixed(blocks) => {
                        payload.insert(
                            "mixed".into(),
                            serde_json::Value::Array(
                                blocks
                                    .iter()
                                    .map(|block| {
                                        serde_json::json!({
                                            "address": address(&block.address),
                                            "values": block.values.iter().copied().map(finite).collect::<Vec<_>>(),
                                        })
                                    })
                                    .collect(),
                            ),
                        );
                    }
                }
                serde_json::json!({"$searchkernel_tensor": payload})
            }
        }
    }

    fn trace_result_map(
        values: &std::collections::BTreeMap<String, ResultValue>,
    ) -> serde_json::Value {
        serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), trace_result_value(value)))
                .collect(),
        )
    }

    fn trace_optional_f64(value: Option<f64>) -> serde_json::Value {
        match value {
            None => serde_json::Value::Null,
            Some(value) if value.is_nan() => serde_json::Value::String("nan".into()),
            Some(value) if value == f64::INFINITY => serde_json::Value::String("+inf".into()),
            Some(value) if value == f64::NEG_INFINITY => serde_json::Value::String("-inf".into()),
            Some(value) => serde_json::Value::Number(
                serde_json::Number::from_f64(value).expect("finite trace float"),
            ),
        }
    }

    fn trace_result_node(node: &ResultNode) -> serde_json::Value {
        match node {
            ResultNode::Hit(hit) => serde_json::json!({
                "kind": "hit",
                "id": hit.id.as_str(),
                "relevance": trace_optional_f64(hit.relevance),
                "fields": trace_result_map(&hit.fields),
                "highlights": hit.highlights.iter().map(|highlight| {
                    serde_json::json!({
                        "field": highlight.field,
                        "fragments": highlight.fragments.iter().map(|fragment| {
                            serde_json::json!({
                                "text": fragment.text,
                                "matches": fragment.matches.iter().map(|span| {
                                    serde_json::json!({
                                        "start_byte": span.start_byte.to_string(),
                                        "end_byte": span.end_byte.to_string(),
                                    })
                                }).collect::<Vec<_>>(),
                            })
                        }).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
                "source": hit.source,
                "types": hit.types,
                "match_features": trace_result_map(&hit.match_features),
                "summary_features": trace_result_map(&hit.summary_features),
                "rank_features": trace_result_map(&hit.rank_features),
                "sort_values": hit.sort_values.iter().map(trace_result_value).collect::<Vec<_>>(),
            }),
            ResultNode::Group(group) => serde_json::json!({
                "kind": "group",
                "id": group.id,
                "label": group.label,
                "value": group.value.as_ref().map(trace_result_value),
                "fields": trace_result_map(&group.fields),
                "children": group.children.iter().map(trace_result_node).collect::<Vec<_>>(),
                "continuations": group.continuations.iter().map(|(key, value)| {
                    serde_json::json!({"key": key, "value": value})
                }).collect::<Vec<_>>(),
                "source": group.source,
                "types": group.types,
                "relevance": trace_optional_f64(group.relevance),
                "limits": group.limits.as_ref().map(|limits| serde_json::json!({
                    "from": limits.from.as_ref().map(trace_result_value),
                    "to": limits.to.as_ref().map(trace_result_value),
                })),
                "metadata": trace_result_map(&group.metadata),
            }),
        }
    }

    fn trace_total_hits_relation(value: TotalHitsRelation) -> &'static str {
        match value {
            TotalHitsRelation::Unspecified => "unspecified",
            TotalHitsRelation::Exact => "exact",
            TotalHitsRelation::LowerBound => "lower_bound",
            TotalHitsRelation::Approximate => "approximate",
            TotalHitsRelation::Unknown => "unknown",
        }
    }

    fn trace_coverage(value: &Coverage) -> serde_json::Value {
        serde_json::json!({
            "coverage_percent": trace_optional_f64(value.coverage_percent),
            "documents": value.documents.map(|item| item.to_string()),
            "full": value.full,
            "nodes": value.nodes.map(|item| item.to_string()),
            "results": value.results.map(|item| item.to_string()),
            "results_full": value.results_full.map(|item| item.to_string()),
            "degraded": value.degraded.as_ref().map(|degraded| serde_json::json!({
                "match_phase": degraded.match_phase,
                "timeout": degraded.timeout,
                "adaptive_timeout": degraded.adaptive_timeout,
                "ann_timeout": degraded.ann_timeout,
                "non_ideal_state": degraded.non_ideal_state,
                "metadata": trace_result_map(&degraded.metadata),
            })),
            "metadata": trace_result_map(&value.metadata),
        })
    }

    fn trace_timing(value: &SearchTiming) -> serde_json::Value {
        serde_json::json!({
            "query_time_seconds": trace_optional_f64(value.query_time_seconds),
            "search_time_seconds": trace_optional_f64(value.search_time_seconds),
            "summary_fetch_time_seconds": trace_optional_f64(value.summary_fetch_time_seconds),
            "metadata": trace_result_map(&value.metadata),
        })
    }

    fn trace_search_result(result: &SearchResult<serde_json::Value>) -> serde_json::Value {
        serde_json::json!({
            "root": trace_result_node(&result.root),
            "total_hits": {
                "value": result.total_hits.value.to_string(),
                "relation": trace_total_hits_relation(result.total_hits.relation),
            },
            "coverage": result.coverage.as_ref().map(trace_coverage),
            "timing": result.timing.as_ref().map(trace_timing),
            "errors": result.issues.iter().map(|issue| serde_json::json!({
                "code": issue.code.to_string(),
                "summary": issue.summary,
                "message": issue.message,
                "source": issue.source,
                "stack_trace": issue.stack_trace,
                "transient": issue.transient,
                "metadata": trace_result_map(&issue.metadata),
            })).collect::<Vec<_>>(),
            "trace": result.trace.as_ref().map(trace_result_value),
            "metadata": trace_result_map(&result.metadata),
        })
    }

    #[test]
    fn response_byte_fixtures_reach_the_public_rust_result_projection() {
        use searchkernel_protocol::protocol::v1 as wire;

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../");
        let corpus: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("reference/result-response-conformance/corpus.json"))
                .expect("read canonical response corpus"),
        )
        .expect("parse canonical response corpus");
        let fixtures = result_fixtures();
        let mut traces = serde_json::Map::new();
        for scenario in corpus["valid"].as_array().unwrap() {
            let name = scenario["id"].as_str().unwrap();
            let fixture = fixtures
                .iter()
                .find(|fixture| fixture.name == name)
                .unwrap_or_else(|| panic!("canonical response fixture has no IR: {name}"));
            let path = root.join(scenario["source"].as_str().unwrap());
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let envelope = core::__flatbuffers::root::<wire::ResponseEnvelope>(&bytes)
                .unwrap_or_else(|error| panic!("decode {}: {error:?}", path.display()));
            let encoded = envelope
                .payload_as_search_kernel_wire_search_result()
                .unwrap_or_else(|| panic!("{} must contain SearchResult", path.display()));
            let decoded = core::wire::v1::search_result_from_api(encoded)
                .unwrap_or_else(|error| panic!("{name} failed IR decoding: {error:?}"));
            let actual: SearchResult<serde_json::Value> = search_result(decoded, &[])
                .unwrap_or_else(|error| panic!("{name} failed public projection: {error:?}"));
            assert_public_node_matches(&fixture.result.root, &actual.root);
            assert_eq!(actual.total_hits, total_hits(fixture.result.total_hits));
            assert_eq!(
                actual.coverage,
                fixture
                    .result
                    .coverage
                    .clone()
                    .map(coverage)
                    .transpose()
                    .unwrap()
            );
            assert_eq!(
                actual.timing,
                fixture
                    .result
                    .timing
                    .clone()
                    .map(search_timing)
                    .transpose()
                    .unwrap()
            );
            assert_eq!(actual.issues.len(), fixture.result.errors.len());
            assert_eq!(actual.trace, fixture.result.trace.clone().map(result_value));
            assert_eq!(
                actual.metadata,
                result_value_map(fixture.result.metadata.clone())
            );
            traces.insert(name.to_owned(), trace_search_result(&actual));
        }

        let mut malformed_outcomes = serde_json::Map::new();
        for case in corpus["malformed"].as_array().unwrap() {
            let name = case["id"].as_str().unwrap();
            let path = root.join(format!(
                "reference/result-response-conformance/fixtures/{}",
                case["file"].as_str().unwrap()
            ));
            let bytes = std::fs::read(&path).unwrap_or_else(|error| {
                panic!("read malformed fixture {}: {error}", path.display())
            });
            let failure_class = case["failure_class"].as_str().unwrap();
            let envelope = match core::__flatbuffers::root::<wire::ResponseEnvelope>(&bytes) {
                Ok(envelope) => envelope,
                Err(error) => {
                    assert!(
                        matches!(
                            failure_class,
                            "missing_continuation_key"
                                | "missing_sparse_address_dimension"
                                | "missing_mixed_address_dimension"
                        ),
                        "{name} failed FlatBuffers verification unexpectedly: {error:?}"
                    );
                    malformed_outcomes
                        .insert(name.to_owned(), serde_json::Value::String("client".into()));
                    continue;
                }
            };
            assert!(
                !matches!(
                    failure_class,
                    "missing_continuation_key"
                        | "missing_sparse_address_dimension"
                        | "missing_mixed_address_dimension"
                ),
                "{name} should fail FlatBuffers verification"
            );
            if case["failure_class"] == "response_payload_union" {
                assert!(
                    envelope
                        .payload_as_search_kernel_wire_search_result()
                        .is_none(),
                    "Rust response projection accepted malformed response payload {name}"
                );
            } else {
                let result = envelope
                    .payload_as_search_kernel_wire_search_result()
                    .unwrap_or_else(|| panic!("{name} must reach the SearchResult decoder"));
                assert!(
                    core::wire::v1::search_result_from_api(result).is_err(),
                    "Rust public result decoding accepted malformed scenario {name}"
                );
            }
            malformed_outcomes.insert(name.to_owned(), serde_json::Value::String("client".into()));
        }

        if let Some(directory) = std::env::var_os("SEARCHKERNEL_RESULT_TRACE_DIR") {
            traces.insert(
                "malformed_outcomes".into(),
                serde_json::Value::Object(malformed_outcomes),
            );
            std::fs::write(
                std::path::PathBuf::from(directory).join("rust.json"),
                serde_json::to_vec(&serde_json::Value::Object(traces)).unwrap(),
            )
            .expect("write Rust result trace");
        }
    }

    #[test]
    fn production_encoded_result_preserves_non_finite_scalar_values_in_public_projection() {
        use searchkernel_protocol::protocol::v1 as wire;

        let mut result = searchkernel_ir::SearchResult::empty(searchkernel_ir::TotalHits {
            value: 1,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        });
        result.root = searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
            id: "non-finite".to_owned(),
            fields: BTreeMap::from([
                ("nan".to_owned(), Value::Float64(f64::NAN)),
                (
                    "positive_infinity".to_owned(),
                    Value::Float64(f64::INFINITY),
                ),
                (
                    "negative_infinity".to_owned(),
                    Value::Float64(f64::NEG_INFINITY),
                ),
            ]),
            ..Default::default()
        });

        let bytes = core::wire::v1::search_result_to_api(&result)
            .expect("production encoder must accept non-finite scalar values");
        let encoded = core::__flatbuffers::root::<wire::SearchResult>(&bytes)
            .expect("production-encoded SearchResult must decode as FlatBuffers");
        let decoded = core::wire::v1::search_result_from_api(encoded)
            .expect("production-encoded non-finite values must decode to IR");
        let projection: SearchResult<serde_json::Value> = search_result(decoded, &[])
            .expect("public result projection must retain non-finite values");
        assert_eq!(
            projection.hits[0].fields["nan"],
            ResultValue::NonFiniteFloat(NonFiniteFloat::NaN)
        );
        assert_eq!(
            projection.hits[0].fields["positive_infinity"],
            ResultValue::NonFiniteFloat(NonFiniteFloat::PositiveInfinity)
        );
        assert_eq!(
            projection.hits[0].fields["negative_infinity"],
            ResultValue::NonFiniteFloat(NonFiniteFloat::NegativeInfinity)
        );
        assert!(projection.hits[0].document.is_none());
        assert!(projection.hits[0].document_error.is_some());
    }

    #[test]
    fn non_finite_document_fallback_has_explicit_precedence_over_t_deserialization() {
        let total_hits = searchkernel_ir::TotalHits {
            value: 1,
            relation: searchkernel_ir::TotalHitsRelation::Exact,
        };

        let mut ordinary = searchkernel_ir::SearchResult::empty(total_hits);
        ordinary.root = searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
            id: "ordinary".to_owned(),
            fields: BTreeMap::from([("title".to_owned(), Value::String("ok".into()))]),
            ..Default::default()
        });
        assert!(matches!(
            search_result::<String>(ordinary, &[]),
            Err(Error::Deserialization(_))
        ));

        let mut mixed = searchkernel_ir::SearchResult::empty(total_hits);
        mixed.root = searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
            id: "mixed".to_owned(),
            fields: BTreeMap::from([
                ("title".to_owned(), Value::String("ok".into())),
                ("ratio".to_owned(), Value::Float64(f64::NAN)),
            ]),
            ..Default::default()
        });
        let result: SearchResult<String> =
            search_result(mixed, &[]).expect("non-finite fields use the lossless fallback");
        assert!(result.hits[0].document.is_none());
        assert_eq!(
            result.hits[0].document_error.as_deref(),
            Some("document contains non-finite scalar values")
        );
        assert_eq!(
            result.hits[0].fields["ratio"],
            ResultValue::NonFiniteFloat(NonFiniteFloat::NaN)
        );
    }

    #[test]
    fn projections_are_views_over_root_and_preserve_projection_order() {
        let mixed = mixed_hit_and_group_children_result();
        let actual: SearchResult<serde_json::Value> = search_result(mixed, &[]).unwrap();
        let ResultNode::Group(root) = &actual.root else {
            panic!("mixed fixture root must be a group");
        };
        assert_eq!(
            actual
                .hits
                .iter()
                .map(|hit| hit.id.as_str())
                .collect::<Vec<_>>(),
            ["root-a", "root-b"]
        );
        assert_eq!(
            actual
                .groups
                .iter()
                .map(|group| group.key.clone())
                .collect::<Vec<_>>(),
            [
                Some(rv(Value::String("A".into()))),
                Some(rv(Value::String("B".into())))
            ]
        );
        assert_eq!(root.children.len(), 4);
        assert!(matches!(root.children[0], ResultNode::Hit(_)));
        assert!(matches!(root.children[1], ResultNode::Group(_)));
        assert!(matches!(root.children[2], ResultNode::Hit(_)));
        assert!(matches!(root.children[3], ResultNode::Group(_)));
        assert_eq!(actual.groups[0].hits[0].id.as_str(), "nested-a");
        assert_eq!(actual.groups[1].hits[0].id.as_str(), "nested-b");

        let sibling: SearchResult<serde_json::Value> =
            search_result(sibling_groups_result(), &[]).unwrap();
        assert_eq!(
            sibling
                .groups
                .iter()
                .map(|group| group.key.clone())
                .collect::<Vec<_>>(),
            [
                Some(rv(Value::String("A".into()))),
                Some(rv(Value::String("B".into()))),
                Some(rv(Value::String("C".into())))
            ]
        );

        let aggregations: SearchResult<serde_json::Value> = search_result(
            searchkernel_testkit::group_fields_and_aggregations_result(),
            &[],
        )
        .unwrap();
        assert_eq!(aggregations.aggregations["count"], rv(Value::UInt64(7)));
        let ResultNode::Group(root) = aggregations.root else {
            panic!("aggregation fixture root must be a group");
        };
        assert_eq!(root.fields["max_price"], rv(Value::Float64(19.5)));
    }

    #[test]
    fn facet_projection_is_strict_and_root_remains_complete() {
        let facet: SearchResult<serde_json::Value> =
            search_result(canonical_facet_result(), &[("brand", "brands")]).unwrap();
        assert_eq!(facet.facets.len(), 1);
        assert!(facet.groups.is_empty());
        let ResultNode::Group(root) = &facet.root else {
            panic!("facet fixture root must be a group");
        };
        assert_eq!(root.children.len(), 1);
        assert!(matches!(root.children[0], ResultNode::Group(_)));

        let ordinary: SearchResult<serde_json::Value> =
            search_result(non_facet_grouping_result(), &[("brand", "brands")]).unwrap();
        assert!(ordinary.facets.is_empty());
        assert_eq!(ordinary.groups.len(), 1);
        assert_eq!(
            ordinary.groups[0].groups[0].aggregations["count"],
            rv(Value::UInt64(4))
        );
        let ResultNode::Group(root) = ordinary.root else {
            panic!("ordinary grouping root must be a group");
        };
        let ResultNode::Group(ordinary) = &root.children[0] else {
            panic!("ordinary grouping child must remain a group");
        };
        assert_eq!(ordinary.metadata["owner"], rv(Value::String("user".into())));
    }

    #[test]
    fn public_api_preserves_meaningful_optional_presence_states() {
        let absent: SearchResult<serde_json::Value> =
            search_result(searchkernel_testkit::absent_optional_fields_result(), &[]).unwrap();
        assert!(absent.coverage.is_none());
        assert!(absent.timing.is_none());
        assert!(absent.trace.is_none());

        let present: SearchResult<serde_json::Value> = search_result(
            searchkernel_testkit::present_empty_optional_fields_result(),
            &[],
        )
        .unwrap();
        assert!(present.coverage.is_some());
        assert!(present.timing.is_some());
        assert_eq!(present.trace, Some(rv(Value::Object(BTreeMap::new()))));
        let ResultNode::Group(root) = present.root else {
            panic!("present-empty fixture root must be a group");
        };
        assert_eq!(root.source.as_deref(), Some(""));
        assert_eq!(root.value, Some(rv(Value::List(Vec::new()))));
        assert_eq!(
            root.limits,
            Some(ResultLimits {
                from: None,
                to: None
            })
        );
    }

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

        let converted: SearchResult<serde_json::Value> =
            search_result(result, &[("brand", "brand")]).unwrap();
        assert_eq!(converted.total_hits.value, 7);
        assert_eq!(converted.total_hits.relation, TotalHitsRelation::LowerBound);
        assert_eq!(converted.hits.len(), 1);
        assert_eq!(converted.hits[0].id.as_str(), "doc-1");
        assert_eq!(
            converted.hits[0].document.as_ref().unwrap()["nested"]["enabled"],
            true
        );
        assert_eq!(
            converted.hits[0].document.as_ref().unwrap()["bytes"],
            serde_json::json!([1, 255])
        );
        assert_eq!(
            converted.hits[0].highlights[0].fragments[0].matches[0].end_byte,
            5
        );
        assert_eq!(converted.facets[0].values[0].value, rv(Value::UInt64(2)));
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
        assert_eq!(
            converted.facets[0].values[0].value,
            rv(Value::String("acme".into()))
        );
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
    fn result_conversion_preserves_coverage_metadata() {
        let converted = coverage(core::Coverage {
            coverage_percent: Some(99.0),
            documents: Some(10),
            full: Some(false),
            nodes: Some(1),
            results: Some(9),
            results_full: Some(10),
            degraded: Some(core::CoverageDegraded {
                match_phase: Some(true),
                timeout: Some(false),
                adaptive_timeout: None,
                ann_timeout: Some(false),
                non_ideal_state: Some(true),
                metadata: BTreeMap::from([(
                    "degraded_reason".into(),
                    Value::String("match-phase".into()),
                )]),
            }),
            metadata: BTreeMap::from([("coverage_source".into(), Value::String("native".into()))]),
        })
        .unwrap();
        assert_eq!(
            converted.metadata["coverage_source"],
            rv(Value::String("native".into()))
        );
        assert_eq!(
            converted.degraded.as_ref().unwrap().metadata["degraded_reason"],
            rv(Value::String("match-phase".into()))
        );
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
        assert_eq!(
            converted.timing.as_ref().unwrap().metadata["timing"],
            rv(Value::String("yes".into()))
        );
        assert_eq!(converted.issues[0].code, 7);
        assert_eq!(converted.issues[0].summary.as_deref(), Some("summary"));
        assert_eq!(converted.issues[0].message.as_deref(), Some("message"));
        assert_eq!(converted.issues[0].source.as_deref(), Some("source"));
        assert_eq!(converted.issues[0].stack_trace.as_deref(), Some("stack"));
        assert_eq!(converted.issues[0].transient, Some(true));
        assert_eq!(
            converted.issues[0].metadata["error"],
            rv(Value::String("yes".into()))
        );
        assert_eq!(converted.trace, Some(rv(Value::String("trace".into()))));
        assert_eq!(
            converted.metadata["result"],
            rv(Value::String("yes".into()))
        );
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
            FeatureValue::Value(rv(Value::List(vec![Value::Int64(1), Value::Bool(true)])))
        );
        assert_eq!(
            converted.summary_features["summary"],
            FeatureValue::Value(rv(Value::Object(BTreeMap::from([(
                "ready".into(),
                Value::Bool(true)
            )]))))
        );
        assert_eq!(
            converted.rank_features["rank"],
            FeatureValue::Value(rv(Value::UInt64(7)))
        );
        assert_eq!(
            converted.sort_values,
            [rv(Value::String("a".into())), rv(Value::UInt64(9))]
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
        assert_eq!(
            converted.hits[0].document.as_ref().unwrap()["title"],
            "nested"
        );
        assert_eq!(
            converted.hits[0].match_features["typed"],
            FeatureValue::Value(rv(Value::List(vec![Value::Int64(1), Value::Bool(true)])))
        );
    }

    #[test]
    fn result_conversion_preserves_sparse_and_mixed_tensor_kinds() {
        let sparse = Value::Tensor(searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(label{})".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Sparse(vec![
                searchkernel_ir::RequestTensorCellIr {
                    address: BTreeMap::from([("label".to_owned(), "a".to_owned())]),
                    value: 0.5,
                },
            ]),
        });
        let mixed = Value::Tensor(searchkernel_ir::RequestTensorIr {
            type_spec: "tensor<float>(label{},x[2])".to_owned(),
            payload: searchkernel_ir::RequestTensorPayloadIr::Mixed(vec![
                searchkernel_ir::RequestTensorBlockIr {
                    address: BTreeMap::from([("label".to_owned(), "b".to_owned())]),
                    values: vec![0.25, 0.75],
                },
            ]),
        });

        let sparse_json = json_value(sparse.clone()).expect("sparse tensor must be public");
        assert_eq!(
            sparse_json["$searchkernel_tensor"]["sparse"][0]["address"][0]["dimension"],
            "label"
        );
        assert_eq!(
            sparse_json["$searchkernel_tensor"]["sparse"][0]["address"][0]["label"],
            "a"
        );
        assert_eq!(
            sparse_json["$searchkernel_tensor"]["sparse"][0]["value"],
            0.5
        );
        assert!(sparse_json["$searchkernel_tensor"].get("dense").is_none());

        let mixed_json = json_value(mixed.clone()).expect("mixed tensor must be public");
        assert_eq!(
            mixed_json["$searchkernel_tensor"]["mixed"][0]["address"][0]["dimension"],
            "label"
        );
        assert_eq!(
            mixed_json["$searchkernel_tensor"]["mixed"][0]["address"][0]["label"],
            "b"
        );
        assert_eq!(
            mixed_json["$searchkernel_tensor"]["mixed"][0]["values"],
            serde_json::json!([0.25, 0.75])
        );
        assert!(mixed_json["$searchkernel_tensor"].get("dense").is_none());

        assert!(matches!(
            convert_feature_value(sparse).unwrap(),
            FeatureValue::Value(ResultValue::Tensor(_))
        ));
        let group = core::ResultGroup {
            value: Some(mixed),
            ..core::ResultGroup::root(vec![])
        };
        let converted = convert_result_group(group).expect("group tensor must convert");
        assert!(matches!(
            converted.value,
            Some(ResultValue::Tensor(ResultTensor {
                payload: ResultTensorPayload::Mixed(_),
                ..
            }))
        ));
    }

    #[test]
    fn public_result_conversion_preserves_sparse_hit_fields_and_mixed_group_values() {
        let converted: SearchResult<serde_json::Value> =
            search_result(sparse_mixed_tensor_result(), &[]).unwrap();
        let ResultNode::Group(root) = converted.root else {
            panic!("tensor fixture root must remain a group");
        };
        assert!(matches!(
            root.value,
            Some(ResultValue::Tensor(ResultTensor {
                payload: ResultTensorPayload::Mixed(_),
                ..
            }))
        ));
        let ResultNode::Hit(hit) = &root.children[0] else {
            panic!("tensor fixture child must remain a hit");
        };
        assert!(matches!(
            hit.fields["sparse"],
            ResultValue::Tensor(ResultTensor {
                payload: ResultTensorPayload::Sparse(_),
                ..
            })
        ));
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
        assert_eq!(converted.aggregations["count"], rv(Value::UInt64(2)));
        let ResultNode::Group(root) = converted.root else {
            panic!("canonical root must remain a result group");
        };
        assert_eq!(root.id.as_deref(), Some("root"));
        assert_eq!(root.fields["count"], rv(Value::UInt64(2)));
        assert_eq!(root.metadata["trace_id"], rv(Value::String("r-1".into())));
        assert_eq!(root.children.len(), 2);
        let ResultNode::Hit(root_hit) = &root.children[0] else {
            panic!("first root child must remain a hit");
        };
        assert_eq!(root_hit.id.as_str(), "root-hit");
        assert_eq!(root_hit.source.as_deref(), Some("catalog"));
        let ResultNode::Group(nested) = &root.children[1] else {
            panic!("second root child must remain a group");
        };
        assert_eq!(nested.continuations["next"], "token");
        assert_eq!(nested.source.as_deref(), Some("group-source"));
        assert_eq!(nested.types, ["bucket"]);
        assert_eq!(nested.relevance, Some(2.5));
        assert_eq!(nested.metadata["owner"], rv(Value::Bool(true)));
        assert_eq!(
            nested.limits.as_ref().unwrap().to,
            Some(rv(Value::UInt64(10)))
        );
        assert_eq!(nested.children.len(), 1);
        let ResultNode::Hit(nested_hit) = &nested.children[0] else {
            panic!("nested hit must remain in the canonical tree");
        };
        assert_eq!(nested_hit.id.as_str(), "nested-hit");
    }

    #[test]
    fn result_conversion_preserves_nonfinite_values_and_rejects_bad_documents() {
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
        let converted = search_result::<serde_json::Value>(nonfinite, &[("score", "score")])
            .expect("explicit non-finite scalar value must survive result projection");
        assert_eq!(
            converted.facets[0].values[0].value,
            ResultValue::NonFiniteFloat(NonFiniteFloat::PositiveInfinity)
        );

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

    #[test]
    fn result_fixture_corpus_freezes_strict_facet_shape() {
        for fixture in searchkernel_testkit::result_fixtures() {
            let converted: SearchResult<serde_json::Value> =
                search_result(fixture.result, &[("brand", "brand")]).unwrap();
            if fixture.name == "facet-valid" {
                assert_eq!(converted.facets.len(), 1);
                assert_eq!(converted.facets[0].values[0].count, 3);
            } else if fixture.name.starts_with("facet-") {
                assert!(
                    converted.facets.is_empty(),
                    "malformed facet fixture was projected: {}",
                    fixture.name
                );
            }
        }
    }
}
