//! Backend-owned findings. No browser or engine-specific syntax is interpreted here.
use crate::{model::*, Backend, GetInput, IndexContext, PortableError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

pub const SOURCE_RUN_METADATA_KEY: &str = "searchkernel.source_run";

/// A deterministic identity over canonical resolved request bytes, not an
/// authentication token. Source-cache equality additionally checks the full IR.
pub fn request_hash(request: &SearchRequestIr) -> String {
    let bytes = crate::wire::v1::search_request_ir_to_api(request);
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("canonical-v1-fnv1a-{hash:016x}")
}

pub fn returned_hits(node: &ResultNode) -> Vec<&Hit> {
    match node {
        ResultNode::Hit(hit) => vec![hit],
        ResultNode::Group(group) => group.children.iter().flat_map(returned_hits).collect(),
    }
}

pub fn execution_complete(result: &SearchResult) -> bool {
    result.errors.is_empty()
        && !result
            .coverage
            .as_ref()
            .is_some_and(|value| value.full == Some(false))
        && !result
            .coverage
            .as_ref()
            .and_then(|value| value.degraded.as_ref())
            .is_some_and(|value| {
                value.timeout == Some(true)
                    || value.ann_timeout == Some(true)
                    || value.adaptive_timeout == Some(true)
            })
}

pub fn compare_runs(
    baseline: &DiagnosticSourceRun,
    execution: &DiagnosticExecution,
    result: &SearchResult,
    rerun_offset: u32,
) -> DiagnosticComparison {
    let baseline_hits = returned_hits(&baseline.result.root);
    let rerun_hits = returned_hits(&result.root);
    let old: BTreeMap<_, _> = baseline_hits
        .iter()
        .enumerate()
        .map(|(i, hit)| (hit.id.clone(), i as u32 + 1 + baseline.request.offset()))
        .collect();
    let new: BTreeMap<_, _> = rerun_hits
        .iter()
        .enumerate()
        .map(|(i, hit)| (hit.id.clone(), i as u32 + 1 + rerun_offset))
        .collect();
    let ids: BTreeSet<_> = old.keys().chain(new.keys()).cloned().collect();
    DiagnosticComparison {
        baseline_execution_id: baseline.execution.id.clone(),
        rerun_execution_id: execution.id.clone(),
        results_changed: (baseline.execution.complete && execution.complete).then_some(
            baseline.result.total_hits != result.total_hits
                || baseline_hits
                    .iter()
                    .map(|hit| (&hit.id, hit.score))
                    .collect::<Vec<_>>()
                    != rerun_hits
                        .iter()
                        .map(|hit| (&hit.id, hit.score))
                        .collect::<Vec<_>>(),
        ),
        overlap_count: old.keys().filter(|id| new.contains_key(*id)).count() as u32,
        comparison_count: old.len().max(new.len()) as u32,
        entering: new
            .keys()
            .filter(|id| !old.contains_key(*id))
            .cloned()
            .collect(),
        leaving: old
            .keys()
            .filter(|id| !new.contains_key(*id))
            .cloned()
            .collect(),
        movements: ids
            .into_iter()
            .map(|id| DiagnosticRankMovement {
                baseline_position: old.get(&id).copied(),
                rerun_position: new.get(&id).copied(),
                id,
            })
            .collect(),
    }
}

fn evidence(
    id: &str,
    section: DiagnosticSection,
    execution: &DiagnosticExecution,
    message: impl Into<String>,
) -> DiagnosticEvidence {
    DiagnosticEvidence {
        id: id.into(),
        section,
        status: DiagnosticStatus::Available,
        provenance: DiagnosticProvenance::Engine,
        execution_id: execution.id.clone(),
        message: message.into(),
        values: BTreeMap::new(),
        canonical_request: None,
        result: None,
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float64(value) => Some(*value),
        // Integer ordinals are intentionally not coerced through double: large
        // exact integers cannot be compared losslessly by that representation.
        _ => None,
    }
}

fn tensor_cells(value: &Value) -> usize {
    match value {
        Value::Tensor(tensor) => match &tensor.payload {
            RequestTensorPayloadIr::Dense(values) => values.len(),
            RequestTensorPayloadIr::Sparse(values) => values.len(),
            RequestTensorPayloadIr::Mixed(blocks) => {
                blocks.iter().map(|block| block.values.len()).sum()
            }
        },
        Value::List(values) => values.iter().map(tensor_cells).sum(),
        Value::Object(values) => values.values().map(tensor_cells).sum(),
        _ => 0,
    }
}

fn hit_evidence(
    result: &SearchResult,
    request: &SearchRequestIr,
    execution: &DiagnosticExecution,
    limits: &DiagnosticLimits,
    signals: &BTreeSet<String>,
) -> (Vec<DiagnosticHit>, bool) {
    let hits = returned_hits(&result.root);
    let mut truncated = hits.len() > limits.feature_hit_count as usize
        || result.metadata.get("diagnostic_feature_truncated") == Some(&Value::Bool(true));
    let displayed = &hits[..hits.len().min(limits.feature_hit_count as usize)];
    let mut remaining_cells = limits.tensor_cells as usize;
    let hits = displayed
        .iter()
        .enumerate()
        .map(|(index, hit)| {
            let features: BTreeMap<_, _> = hit
                .match_features
                .iter()
                .chain(&hit.summary_features)
                .chain(&hit.rank_features)
                .filter(|(name, _)| signals.contains(*name))
                .collect();
            let mut output = Vec::new();
            for (name, value) in features {
                let cells = tensor_cells(value);
                if cells > remaining_cells {
                    truncated = true;
                    continue;
                }
                remaining_cells -= cells;
                let values: Vec<_> = displayed
                    .iter()
                    .filter_map(|hit| {
                        hit.match_features
                            .get(name)
                            .or_else(|| hit.summary_features.get(name))
                            .or_else(|| hit.rank_features.get(name))
                    })
                    .collect();
                let numeric = number(value).filter(|value| value.is_finite());
                let position = numeric.map(|number_value| {
                    1 + values
                        .iter()
                        .filter(|other| number(other).is_some_and(|other| other > number_value))
                        .count() as u32
                });
                let ties = numeric.map(|number_value| {
                    values
                        .iter()
                        .filter(|other| number(other) == Some(number_value))
                        .count() as u32
                });
                output.push(DiagnosticSignal {
                    name: name.clone(),
                    value: value.clone(),
                    position,
                    ties,
                    comparison_count: values.len() as u32,
                    comparison_set: format!("{}:returned-set", execution.id),
                });
            }
            DiagnosticHit {
                id: hit.id.clone(),
                execution_id: execution.id.clone(),
                returned_position: request.offset() + index as u32 + 1,
                relevance: hit.score,
                signals: output,
                evidence_ids: vec!["ranking".into()],
            }
        })
        .collect();
    (hits, truncated)
}

fn configured_profiles<'a>(
    context: &'a IndexContext,
    name: &str,
) -> Vec<&'a RankProfileDefinitionIr> {
    let mut names = vec![name.to_owned()];
    let mut seen = BTreeSet::new();
    let mut profiles = Vec::new();
    while let Some(name) = names.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(profile) = context
            .definition()
            .rank_profiles()
            .iter()
            .find(|profile| profile.name == name)
        {
            names.extend(profile.inherits.iter().cloned());
            profiles.push(profile);
        }
    }
    profiles
}

fn filter_variants(request: &SearchRequestIr) -> Vec<(String, SearchRequestIr)> {
    let Some(filter) = request.filter() else {
        return Vec::new();
    };
    let clauses = match filter {
        FilterIr::And(clauses) => clauses.clone(),
        other => vec![other.clone()],
    };
    clauses
        .iter()
        .enumerate()
        .map(|(index, clause)| {
            let mut variant = request.clone();
            let remaining: Vec<_> = clauses
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, clause)| clause.clone())
                .collect();
            variant.filter = match remaining.len() {
                0 => None,
                1 => remaining.into_iter().next(),
                _ => Some(FilterIr::And(remaining)),
            };
            (format!("Removing {}", friendly_filter(clause)), variant)
        })
        .collect()
}

fn friendly_filter(filter: &FilterIr) -> String {
    match filter {
        FilterIr::Comparison(comparison) => {
            let operator = match comparison.op {
                ComparisonOp::Eq => "=",
                ComparisonOp::Ne => "!=",
                ComparisonOp::Lt => "<",
                ComparisonOp::Lte => "<=",
                ComparisonOp::Gt => ">",
                ComparisonOp::Gte => ">=",
            };
            let value = match &comparison.value {
                ScalarValueIr::Bool(value) => value.to_string(),
                ScalarValueIr::Int64(value) => value.to_string(),
                ScalarValueIr::Float64(value) => value.to_string(),
                ScalarValueIr::String(value) => format!("{value:?}"),
            };
            format!("{} {operator} {value}", comparison.field.as_str())
        }
        FilterIr::Exists { field } => format!("{} exists", field.as_str()),
        FilterIr::In { field, .. } => {
            format!("the explicit membership filter on {}", field.as_str())
        }
        FilterIr::Range(range) => format!("the explicit range on {}", range.field.as_str()),
        FilterIr::And(children) => format!(
            "({})",
            children
                .iter()
                .map(friendly_filter)
                .collect::<Vec<_>>()
                .join(" AND ")
        ),
        FilterIr::Or(children) => format!(
            "({})",
            children
                .iter()
                .map(friendly_filter)
                .collect::<Vec<_>>()
                .join(" OR ")
        ),
        FilterIr::Not(child) => format!("NOT ({})", friendly_filter(child)),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct ExactVariant {
    request: SearchRequestIr,
    query_path: String,
    document_field: String,
    query_tensor: String,
    previous_approximate: bool,
    target_hits: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct ExactNnCandidate {
    query_path: String,
    document_field: String,
    query_tensor: String,
    previous_approximate: bool,
    target_hits: u32,
    eligible_path: bool,
}

#[derive(Default)]
struct ExactNnScan {
    candidates: Vec<ExactNnCandidate>,
    contains_parser_driven_query: bool,
}

fn exact_nn_u32_annotation(
    annotations: &BTreeMap<String, RequestValueIr>,
    key: &str,
) -> std::result::Result<Option<u32>, String> {
    match annotations.get(key) {
        Some(RequestValueIr::Int64(value)) => u32::try_from(*value)
            .map(Some)
            .map_err(|_| format!("{key} must be a non-negative 32-bit integer")),
        Some(RequestValueIr::UInt64(value)) => u32::try_from(*value)
            .map(Some)
            .map_err(|_| format!("{key} must fit in a 32-bit integer")),
        None => Ok(None),
        Some(_) => Err(format!("{key} must be an integer")),
    }
}

fn scan_exact_nn(
    expression: &QueryExprIr,
    path: &str,
    eligible_path: bool,
    scan: &mut ExactNnScan,
) -> std::result::Result<(), String> {
    let scan_children = |children: &[QueryExprIr],
                         operator: &str,
                         child_eligible: bool,
                         scan: &mut ExactNnScan|
     -> std::result::Result<(), String> {
        for (index, child) in children.iter().enumerate() {
            scan_exact_nn(
                child,
                &format!("{path}.{operator}[{index}]"),
                child_eligible,
                scan,
            )?;
        }
        Ok(())
    };

    match &expression.kind {
        QueryExprKindIr::NearestNeighbor {
            document_field,
            query_tensor,
        } => {
            let target_num_hits =
                exact_nn_u32_annotation(&expression.annotations, "targetNumHits")?;
            let total_target_hits =
                exact_nn_u32_annotation(&expression.annotations, "totalTargetHits")?;
            let target_hits = match (target_num_hits, total_target_hits) {
                (Some(_), Some(_)) => return Err(
                    "nearest-neighbor may specify only one of targetNumHits and totalTargetHits"
                        .into(),
                ),
                (Some(value), None) | (None, Some(value)) => value,
                (None, None) => 100,
            };
            let previous_approximate = match expression.annotations.get("approximate") {
                Some(RequestValueIr::Bool(value)) => *value,
                None => true,
                Some(_) => return Err("nearest-neighbor approximate must be a boolean".into()),
            };
            scan.candidates.push(ExactNnCandidate {
                query_path: path.into(),
                document_field: document_field.path.as_str().into(),
                query_tensor: query_tensor.clone(),
                previous_approximate,
                target_hits,
                eligible_path,
            });
        }
        QueryExprKindIr::UserInput { .. } | QueryExprKindIr::UserQuery { .. } => {
            scan.contains_parser_driven_query = true;
        }
        QueryExprKindIr::And(children) => {
            scan_children(children, "and", eligible_path, scan)?;
        }
        QueryExprKindIr::Or(children) => {
            scan_children(children, "or", eligible_path, scan)?;
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            scan_exact_nn(matching, &format!("{path}.rank.match"), eligible_path, scan)?;
            // Rank-only branches are preserved byte-for-byte, but an NN inside
            // one is not a retrieval target for an exact-retrieval diagnostic.
            scan_children(ranking, "rank.ranking", false, scan)?;
        }
        QueryExprKindIr::Labeled { query, .. } => {
            scan_exact_nn(query, &format!("{path}.labeled.query"), eligible_path, scan)?;
        }
        QueryExprKindIr::NonEmpty(query) => {
            scan_exact_nn(
                query,
                &format!("{path}.non_empty.query"),
                eligible_path,
                scan,
            )?;
        }
        // These operators are structurally typed, but an NN nested inside them
        // participates in negation, field scoping, proximity/equivalence, or
        // another bounded candidate operator. We do not claim that replacing
        // ANN with exact NN there isolates only ANN recall.
        QueryExprKindIr::Contains { item, .. } => {
            scan_exact_nn(item, &format!("{path}.contains.item"), false, scan)?;
        }
        QueryExprKindIr::Not(child) => {
            scan_exact_nn(child, &format!("{path}.not.child"), false, scan)?;
        }
        QueryExprKindIr::Phrase(children) => {
            scan_children(children, "phrase", false, scan)?;
        }
        QueryExprKindIr::Near { children, .. } => {
            scan_children(children, "near", false, scan)?;
        }
        QueryExprKindIr::ONear { children, .. } => {
            scan_children(children, "onear", false, scan)?;
        }
        QueryExprKindIr::SameElement(children) => {
            scan_children(children, "same_element", false, scan)?;
        }
        QueryExprKindIr::Equiv(children) => {
            scan_children(children, "equiv", false, scan)?;
        }
        QueryExprKindIr::Alternatives(values) => {
            for (index, value) in values.iter().enumerate() {
                scan_exact_nn(
                    &value.query,
                    &format!("{path}.alternatives[{index}].query"),
                    false,
                    scan,
                )?;
            }
        }
        QueryExprKindIr::WeakAnd { children, .. } => {
            scan_children(children, "weak_and", false, scan)?;
        }
        QueryExprKindIr::Term { .. }
        | QueryExprKindIr::True
        | QueryExprKindIr::False
        | QueryExprKindIr::Comparison { .. }
        | QueryExprKindIr::Range { .. }
        | QueryExprKindIr::In { .. }
        | QueryExprKindIr::Fuzzy { .. }
        | QueryExprKindIr::Matches { .. }
        | QueryExprKindIr::Text { .. }
        | QueryExprKindIr::DotProduct { .. }
        | QueryExprKindIr::WeightedSet { .. }
        | QueryExprKindIr::Wand { .. }
        | QueryExprKindIr::GeoLocation { .. }
        | QueryExprKindIr::GeoBoundingBox { .. }
        | QueryExprKindIr::Predicate { .. } => {}
    }
    Ok(())
}

fn set_exact_nn_at_path(expression: &mut QueryExprIr, path: &str, target: &str) -> bool {
    if path == target {
        if !matches!(expression.kind, QueryExprKindIr::NearestNeighbor { .. }) {
            return false;
        }
        expression
            .annotations
            .insert("approximate".into(), RequestValueIr::Bool(false));
        return true;
    }

    match &mut expression.kind {
        QueryExprKindIr::And(children) => children.iter_mut().enumerate().any(|(index, child)| {
            set_exact_nn_at_path(child, &format!("{path}.and[{index}]"), target)
        }),
        QueryExprKindIr::Or(children) => children.iter_mut().enumerate().any(|(index, child)| {
            set_exact_nn_at_path(child, &format!("{path}.or[{index}]"), target)
        }),
        QueryExprKindIr::Rank { matching, .. } => {
            set_exact_nn_at_path(matching, &format!("{path}.rank.match"), target)
        }
        QueryExprKindIr::Labeled { query, .. } => {
            set_exact_nn_at_path(query, &format!("{path}.labeled.query"), target)
        }
        QueryExprKindIr::NonEmpty(query) => {
            set_exact_nn_at_path(query, &format!("{path}.non_empty.query"), target)
        }
        _ => false,
    }
}

/// Build a semantics-preserving exact-NN counterfactual from canonical typed IR.
///
/// Eligibility is deliberately structural: there must be exactly one NN node,
/// it must be in a retrieval-preserving root/AND/OR/rank-matching/wrapper path,
/// and the tree may not contain parser-driven userInput/userQuery operators.
/// Opaque YQL is never parsed or rewritten.
fn exact_variant(
    request: &SearchRequestIr,
    limits: &DiagnosticLimits,
) -> std::result::Result<ExactVariant, String> {
    let QueryIr::Typed(expression) = request.query() else {
        return Err(
            "Exact comparison requires a canonical typed query; opaque YQL and text query sources are not rewritten."
                .into(),
        );
    };
    let mut scan = ExactNnScan::default();
    scan_exact_nn(expression, "query", true, &mut scan)?;
    if scan.contains_parser_driven_query {
        return Err(
            "Exact comparison is unavailable when the typed tree contains userInput/userQuery because those parser-driven operators are not structurally rewritten."
                .into(),
        );
    }
    let candidate = match scan.candidates.as_slice() {
        [] => {
            return Err(
                "Exact comparison requires one canonical nearest-neighbor operator.".into(),
            )
        }
        [candidate] => candidate,
        _ => {
            return Err(
                "Exact comparison is ambiguous for multiple nearest-neighbor operators without a selected NN identity."
                    .into(),
            )
        }
    };
    if !candidate.eligible_path {
        return Err(
            "The nearest-neighbor operator is nested under a query structure whose exact-vs-approximate retrieval semantics are not proven for this diagnostic."
                .into(),
        );
    }
    if !candidate.previous_approximate {
        return Err(
            "The selected nearest-neighbor operator is already exact (approximate=false).".into(),
        );
    }
    if candidate.target_hits > limits.exact_nn_hits || request.limit() > limits.exact_nn_hits {
        return Err(format!(
            "Exact comparison exceeds the diagnostic hit budget (target hits {}, result limit {}, budget {}).",
            candidate.target_hits,
            request.limit(),
            limits.exact_nn_hits
        ));
    }

    let mut variant = request.clone();
    let QueryIr::Typed(expression) = &mut variant.query else {
        unreachable!("typed query was checked above");
    };
    if !set_exact_nn_at_path(expression, "query", &candidate.query_path) {
        return Err(
            "failed to locate the selected nearest-neighbor node in the cloned query".into(),
        );
    }

    Ok(ExactVariant {
        request: variant,
        query_path: candidate.query_path.clone(),
        document_field: candidate.document_field.clone(),
        query_tensor: candidate.query_tensor.clone(),
        previous_approximate: candidate.previous_approximate,
        target_hits: candidate.target_hits,
    })
}

fn friendly_typed_query(expression: &QueryExprIr) -> String {
    let list = |children: &[QueryExprIr], operator: &str| {
        children
            .iter()
            .map(|child| format!("({})", friendly_typed_query(child)))
            .collect::<Vec<_>>()
            .join(operator)
    };
    let text = match &expression.kind {
        QueryExprKindIr::True => "Match all".into(),
        QueryExprKindIr::False => "Match none".into(),
        QueryExprKindIr::Term { value } => format!("Term {value:?}"),
        QueryExprKindIr::Text { text } => format!("Text query {text:?}, using configured analysis"),
        QueryExprKindIr::Contains { field, item } => format!("Field {} contains ({})", field.path.as_str(), friendly_typed_query(item)),
        QueryExprKindIr::And(children) => list(children, " AND "),
        QueryExprKindIr::Or(children) => list(children, " OR "),
        QueryExprKindIr::Not(child) => format!("NOT ({})", friendly_typed_query(child)),
        QueryExprKindIr::Comparison { field, op, value } => format!("Field {} {op:?} {value:?}", field.path.as_str()),
        QueryExprKindIr::In { field, values } => format!("Field {} is in {values:?}", field.path.as_str()),
        QueryExprKindIr::Rank { matching, ranking } => format!("Retrieve using ({}); {} additional ranking clause(s) do not themselves define the matching branch", friendly_typed_query(matching), ranking.len()),
        QueryExprKindIr::NearestNeighbor { document_field, query_tensor } => format!("Nearest-neighbor retrieval in {} using query tensor {query_tensor:?}. This is top-k retrieval, not exhaustive matching", document_field.path.as_str()),
        QueryExprKindIr::WeakAnd { children, target_hits } => format!("Weak-AND candidate retrieval with target {target_hits} hits from ({})", list(children, ", ")),
        QueryExprKindIr::Labeled { query, label, .. } => format!("Label {label:?}: ({})", friendly_typed_query(query)),
        QueryExprKindIr::Phrase(children) => format!("Phrase ({})", list(children, ", ")),
        // The technical tree is authoritative for operators without a friendly
        // renderer. Never guess their semantics from an engine query string.
        _ => "Advanced canonical operator; inspect its normalized query tree for exact controls".into(),
    };
    if expression.annotations.is_empty() {
        text
    } else {
        format!("{text}. Explicit operator annotations are shown in the normalized request")
    }
}

fn positive_clause_variants(request: &SearchRequestIr) -> Vec<(String, SearchRequestIr)> {
    let QueryIr::Typed(expression) = request.query() else {
        return Vec::new();
    };
    let QueryExprKindIr::And(clauses) = &expression.kind else {
        return Vec::new();
    };
    // Root annotations can affect retrieval; removing a clause must not silently
    // discard or reinterpret those controls.
    if !expression.annotations.is_empty() {
        return Vec::new();
    }
    clauses
        .iter()
        .enumerate()
        .filter(|(_, clause)| !matches!(clause.kind, QueryExprKindIr::Not(_)))
        .map(|(index, _)| {
            let mut variant = request.clone();
            let remaining: Vec<_> = clauses
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != index)
                .map(|(_, clause)| clause.clone())
                .collect();
            variant.query = match remaining.len() {
                0 => QueryIr::MatchAll,
                1 => QueryIr::Typed(Box::new(remaining.into_iter().next().unwrap_or(
                    QueryExprIr {
                        kind: QueryExprKindIr::True,
                        annotations: Default::default(),
                    },
                ))),
                _ => QueryIr::Typed(Box::new(QueryExprIr {
                    kind: QueryExprKindIr::And(remaining),
                    annotations: expression.annotations.clone(),
                })),
            };
            (
                format!("Removing positive AND clause {}", index + 1),
                variant,
            )
        })
        .collect()
}

fn bounded_request(
    request: &SearchRequestIr,
    limits: &DiagnosticLimits,
    deadline: Instant,
) -> Result<SearchRequestIr> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(PortableError::new(
            crate::ErrorCode::DeadlineExceeded,
            "diagnostic timeout exceeded",
        ));
    }
    let mut request = request.clone();
    request.timeout = Some(request.timeout.unwrap_or(remaining).min(remaining));
    request.limit = request.limit.min(limits.feature_hit_count);
    Ok(request)
}

pub fn execute(
    context: &IndexContext,
    backend: &dyn Backend,
    input: QueryDiagnosticsRequestIr,
    source: Option<DiagnosticSourceRun>,
    mut execution: DiagnosticExecution,
) -> Result<QueryDiagnosticsResult> {
    if source
        .as_ref()
        .is_some_and(|source| source.request != input.request)
    {
        return Err(PortableError::failed_precondition(
            "diagnostic_source_request_mismatch",
            Some("source_run_id"),
            "the source run belongs to a different canonical request",
        ));
    }
    let deadline = Instant::now() + Duration::from_millis(input.limits.timeout_ms);
    let request = bounded_request(
        input.experiment.as_ref().unwrap_or(&input.request),
        &input.limits,
        deadline,
    )?;
    execution.request_hash = request_hash(&request);
    execution.kind = if input.intent == DiagnosticIntent::Experiment {
        DiagnosticExecutionKind::Experiment
    } else {
        DiagnosticExecutionKind::DiagnosticRerun
    };
    let (acquired, timed_out) = match backend.search_diagnostic(
        request.clone(),
        &input.requested_evidence,
        &input.limits,
    ) {
        Ok(result) => (result, false),
        Err(error) if error.code == crate::ErrorCode::DeadlineExceeded => (
            DiagnosticSearchResult {
                result: SearchResult::empty(TotalHits {
                    value: 0,
                    relation: TotalHitsRelation::Unknown,
                }),
                raw_engine: Vec::new(),
                observed_sections: Vec::new(),
            },
            true,
        ),
        Err(error) => return Err(error),
    };
    execution.complete = !timed_out && execution_complete(&acquired.result);
    let comparison = source
        .as_ref()
        .map(|source| compare_runs(source, &execution, &acquired.result, request.offset()));
    let mut warnings = Vec::new();
    if timed_out {
        warnings.push("Diagnostic acquisition exceeded its timeout. The original search was not affected; no complete rerun evidence was obtained.".into());
    }
    if input.source_run_id.is_some() && source.is_none() {
        warnings.push("Original execution evidence has expired or was too large to retain. This evidence comes from a diagnostic rerun.".into());
    }
    if comparison
        .as_ref()
        .is_some_and(|value| value.results_changed == Some(true))
    {
        warnings.push("The diagnostic rerun differs from the original search; its evidence does not describe the original execution.".into());
    }
    let mut effective = evidence("query", DiagnosticSection::Query, &execution,
        match request.query() {
            QueryIr::MatchAll => "Match all documents, subject to the explicit filters and result controls.",
            QueryIr::Text(_) => "Text query evaluated using the configured Search Kernel search fields and analysis.",
            QueryIr::Typed(_) => "Canonical typed query; inspect the normalized tree for its explicit operators.",
            QueryIr::Yql { .. } => "Engine-owned YQL query. Friendly interpretation of arbitrary YQL is unavailable; inspect the exact source and engine tree.",
        });
    effective.canonical_request = Some(request.clone());
    if let QueryIr::Typed(query) = request.query() {
        effective.values.insert(
            "friendly_query".into(),
            Value::String(friendly_typed_query(query)),
        );
    }

    effective.values.insert(
        "searchkernel_version".into(),
        Value::String(env!("CARGO_PKG_VERSION").into()),
    );
    if let Some(filter) = request.filter() {
        effective.values.insert(
            "explicit_filters".into(),
            Value::String(friendly_filter(filter)),
        );
    }
    let profiles = configured_profiles(context, &request.ranking_request().profile);
    let signals: BTreeSet<_> = profiles
        .iter()
        .flat_map(|profile| {
            profile
                .match_features
                .iter()
                .chain(&profile.summary_features)
                .chain(&profile.rank_features)
        })
        .cloned()
        .collect();
    let mut ranking = evidence(
        "ranking",
        DiagnosticSection::Ranking,
        &execution,
        if signals.is_empty() {
            "Named semantic explanation signals are not configured. Returned position and final relevance are structural evidence, not a causal explanation."
        } else {
            "Intentionally configured signals; positions and ties describe this returned set only. Numeric differences do not establish causal importance."
        },
    );
    ranking.provenance = DiagnosticProvenance::Profile;
    ranking.values.insert(
        "profile".into(),
        Value::String(request.ranking_request().profile.clone()),
    );
    ranking.values.insert(
        "named_signals_configured".into(),
        Value::Bool(!signals.is_empty()),
    );
    for (name, configured) in [
        (
            "first_phase_configured",
            profiles.iter().any(|profile| profile.first_phase.is_some()),
        ),
        (
            "second_phase_configured",
            profiles
                .iter()
                .any(|profile| profile.second_phase.is_some()),
        ),
        (
            "global_phase_configured",
            profiles
                .iter()
                .any(|profile| profile.global_phase.is_some()),
        ),
    ] {
        ranking.values.insert(
            name.into(),
            if profiles.is_empty() {
                Value::Null
            } else {
                Value::Bool(configured)
            },
        );
    }
    ranking
        .values
        .insert("per_phase_participation".into(), Value::Null);
    let (hits, truncated) = hit_evidence(
        &acquired.result,
        &request,
        &execution,
        &input.limits,
        &signals,
    );
    if truncated {
        ranking.status = DiagnosticStatus::Truncated;
        warnings.push("Ranking evidence exceeded the hit/tensor limit.".into());
    }
    if let Some(unavailable) = acquired
        .result
        .metadata
        .get("diagnostic_unavailable_features")
    {
        ranking
            .values
            .insert("unavailable_features".into(), unavailable.clone());
        warnings.push("Some configured ranking features could not be decoded within supported tensor types and diagnostic limits. Their values were omitted.".into());
    }
    let mut timing = evidence("timing", DiagnosticSection::Timing, &execution,
        "Timing from this diagnostic rerun. Original timing remains in the original search response.");
    if let Some(time) = &acquired.result.timing {
        timing.values.extend(time.metadata.clone());
        for (name, value) in [
            ("query_time_seconds", time.query_time_seconds),
            ("search_time_seconds", time.search_time_seconds),
            (
                "summary_fetch_time_seconds",
                time.summary_fetch_time_seconds,
            ),
        ] {
            timing.values.insert(
                name.into(),
                value.map(Value::Float64).unwrap_or(Value::Null),
            );
        }
    } else {
        timing.status = DiagnosticStatus::UnavailableForQuery;
    }
    let mut retrieval = evidence("retrieval", DiagnosticSection::Retrieval, &execution,
        "Returned hits are observed. Retrieval candidates and per-phase participation are unavailable unless the engine provides them; absence from returned results does not establish failure to match.");
    retrieval.values.insert(
        "total_hits".into(),
        Value::UInt64(acquired.result.total_hits.value),
    );
    retrieval.values.insert(
        "total_hits_relation".into(),
        Value::String(format!("{:?}", acquired.result.total_hits.relation)),
    );
    retrieval
        .values
        .insert("retrieval_candidate_count".into(), Value::Null);
    let mut evidence_items = vec![effective, retrieval, ranking, timing];
    if timed_out {
        for item in &mut evidence_items {
            if item.section != DiagnosticSection::Query {
                item.status = DiagnosticStatus::TimedOut;
            }
        }
    }
    if input.intent == DiagnosticIntent::VectorRetrieval {
        let mut vector = evidence("vectors", DiagnosticSection::Vectors, &execution,
            "Canonical vector request configuration. Approximate is a requested mode, not proof of exhaustive matching or of the engine's actual retrieval algorithm. Candidate counts are unavailable.");
        vector.provenance = DiagnosticProvenance::Profile;
        if let QueryIr::Typed(expression) = request.query() {
            if let QueryExprKindIr::NearestNeighbor {
                document_field,
                query_tensor,
            } = &expression.kind
            {
                vector.values.insert(
                    "tensor_field".into(),
                    Value::String(document_field.path.as_str().into()),
                );
                vector.values.insert(
                    "query_tensor_name".into(),
                    Value::String(query_tensor.clone()),
                );
                if let Some(QueryInputIr::Literal(RequestValueIr::Tensor(tensor))) =
                    request.inputs().get(query_tensor)
                {
                    vector.values.insert(
                        "query_tensor_type".into(),
                        Value::String(tensor.type_spec.clone()),
                    );
                }
                for (key, default) in [
                    ("approximate", Value::Bool(true)),
                    ("targetNumHits", Value::UInt64(100)),
                    ("exploreAdditionalHits", Value::UInt64(0)),
                ] {
                    let configured = expression.annotations.get(key).or_else(|| {
                        if key == "targetNumHits" {
                            expression.annotations.get("totalTargetHits")
                        } else {
                            None
                        }
                    });
                    let value = match configured {
                        Some(RequestValueIr::Bool(v)) => Value::Bool(*v),
                        Some(RequestValueIr::Int64(v)) => Value::Int64(*v),
                        Some(RequestValueIr::UInt64(v)) => Value::UInt64(*v),
                        _ => default,
                    };
                    vector.values.insert(key.into(), value);
                }
                if let Some(field) = context
                    .definition()
                    .fields()
                    .iter()
                    .find(|field| field.name.as_str() == document_field.path.as_str())
                {
                    vector.values.insert(
                        "distance_metric".into(),
                        field
                            .attribute
                            .as_ref()
                            .map(|attribute| {
                                Value::String(format!("{:?}", attribute.distance_metric))
                            })
                            .unwrap_or(Value::Null),
                    );
                }
                vector
                    .values
                    .insert("observed_candidate_count".into(), Value::Null);
            } else {
                vector.status = DiagnosticStatus::UnavailableForQuery;
            }
        } else {
            vector.status = DiagnosticStatus::UnavailableForQuery;
        }
        evidence_items.push(vector);
    }
    let mut findings = Vec::new();
    if let Some(time) = &acquired.result.timing {
        if let (Some(Value::Float64(first)), Some(Value::Float64(second))) = (
            time.metadata.get("first_phase_profile_elapsed_seconds_sum"),
            time.metadata
                .get("second_phase_profile_elapsed_seconds_sum"),
        ) {
            if first != second {
                findings.push(DiagnosticFinding { code: "measured_ranking_phase_comparison".into(), message: format!("Summed measured {}-phase profiler elapsed time exceeded {}-phase time in this rerun ({first:.6}s first phase; {second:.6}s second phase). These are sums across engine threads, not wall-clock query latency.", if first > second { "first" } else { "second" }, if first > second { "second" } else { "first" }), evidence_ids: vec!["timing".into()] });
            }
        }
    }
    let mut expected_document = None;
    if let Some(id) = &input.expected_document_id {
        let get = crate::resolve_get(
            context,
            GetInput {
                id: id.clone(),
                projection: Some(Vec::new()),
            },
        )?;
        let exists = if timed_out {
            false
        } else {
            backend.get(get)?.document.is_some()
        };
        let returned = returned_hits(&acquired.result.root)
            .iter()
            .any(|hit| &hit.id == id);
        let mut item = evidence("expected-document", DiagnosticSection::Retrieval, &execution,
            "Existence is a fresh document read; returned status is observed in the diagnostic rerun. Unobserved intermediate stages remain unavailable.");
        item.values
            .insert("document_exists".into(), Value::Bool(exists));
        item.values.insert(
            "returned".into(),
            if returned || execution.complete {
                Value::Bool(returned)
            } else {
                Value::Null
            },
        );
        if !timed_out && (!exists || (!returned && execution.complete)) {
            item.values.insert(
                "first_observed_negative_stage".into(),
                Value::String(
                    if exists {
                        "Returned"
                    } else {
                        "Document exists"
                    }
                    .into(),
                ),
            );
        }
        if timed_out {
            item.status = DiagnosticStatus::TimedOut;
            item.values.clear();
        }
        evidence_items.push(item);
        let observed = |value| {
            if timed_out {
                DiagnosticObservation::Unavailable
            } else if value {
                DiagnosticObservation::ObservedYes
            } else {
                DiagnosticObservation::ObservedNo
            }
        };
        expected_document = Some(DiagnosticExpectedDocument {
            id: id.clone(),
            stages: [
                ("Document exists", observed(exists)),
                ("Explicit filters", DiagnosticObservation::Unavailable),
                ("Retrieval candidates", DiagnosticObservation::Unavailable),
                ("First-phase ranking", DiagnosticObservation::Unavailable),
                ("Second-phase ranking", DiagnosticObservation::Unavailable),
                (
                    "Returned",
                    if !returned && !execution.complete {
                        DiagnosticObservation::Unavailable
                    } else {
                        observed(returned)
                    },
                ),
            ]
            .into_iter()
            .map(|(name, observation)| DiagnosticDocumentStage {
                name: name.into(),
                observation,
                evidence_ids: if observation == DiagnosticObservation::Unavailable {
                    Vec::new()
                } else {
                    vec!["expected-document".into()]
                },
            })
            .collect(),
        });
        if !timed_out && (!exists || (!returned && execution.complete)) {
            findings.push(DiagnosticFinding { code: if exists { "expected_document_not_returned" } else { "expected_document_absent" }.into(),
                message: if exists { "The expected document exists but was not returned by this diagnostic rerun. Its matching and ranking stages were not observed." }
                    else { "A fresh document read did not find the expected document." }.into(), evidence_ids: vec!["expected-document".into()] });
        }
    }
    if input.intent == DiagnosticIntent::ZeroResults
        || input.intent == DiagnosticIntent::MissingDocument
    {
        for (i, (description, variant)) in filter_variants(&input.request)
            .into_iter()
            .chain(positive_clause_variants(&input.request))
            .take(input.limits.variant_count as usize)
            .enumerate()
        {
            let id = format!("constraint-variant-{}", i + 1);
            let variant = match bounded_request(&variant, &input.limits, deadline) {
                Ok(value) => value,
                Err(_) => {
                    let mut item =
                        evidence(&id, DiagnosticSection::Retrieval, &execution, description);
                    item.status = DiagnosticStatus::TimedOut;
                    evidence_items.push(item);
                    execution.complete = false;
                    break;
                }
            };
            let mut variant_execution = execution.clone();
            variant_execution.id = format!("{}/variant-{}", execution.id, i + 1);
            variant_execution.request_hash = request_hash(&variant);
            match backend.search(variant.clone()) {
                Ok(mut result) => {
                    let mut item = evidence(
                        &id,
                        DiagnosticSection::Retrieval,
                        &variant_execution,
                        description.clone(),
                    );
                    item.canonical_request = Some(variant);
                    item.values.insert(
                        "result_count".into(),
                        Value::UInt64(result.total_hits.value),
                    );
                    let expected_returned = input.expected_document_id.as_ref().is_some_and(|id| {
                        returned_hits(&result.root).iter().any(|hit| &hit.id == id)
                    });
                    item.values.insert(
                        "expected_document_returned".into(),
                        Value::Bool(expected_returned),
                    );
                    if !execution_complete(&result) {
                        item.status = DiagnosticStatus::TimedOut;
                        execution.complete = false;
                    } else if !timed_out
                        && execution_complete(&acquired.result)
                        && ((acquired.result.total_hits.value == 0 && result.total_hits.value > 0)
                            || (expected_returned
                                && input.expected_document_id.as_ref().is_some_and(|id| {
                                    !returned_hits(&acquired.result.root)
                                        .iter()
                                        .any(|hit| &hit.id == id)
                                })))
                    {
                        findings.push(DiagnosticFinding {
                            code: "constraint_variant_sufficient".into(),
                            message: format!(
                                "{description} is sufficient to {} in this diagnostic rerun.",
                                if expected_returned {
                                    "return the expected document"
                                } else {
                                    "produce results"
                                }
                            ),
                            evidence_ids: vec![id.clone(), "retrieval".into()],
                        });
                    }
                    strip_document_bodies(&mut result.root);
                    strip_features(&mut result.root);
                    item.result = Some(result);
                    evidence_items.push(item);
                }
                Err(error) => {
                    let mut item = evidence(
                        &id,
                        DiagnosticSection::Retrieval,
                        &variant_execution,
                        error.message.clone(),
                    );
                    item.canonical_request = Some(variant);
                    item.status = if error.code == crate::ErrorCode::DeadlineExceeded {
                        DiagnosticStatus::TimedOut
                    } else {
                        DiagnosticStatus::Failed
                    };
                    evidence_items.push(item);
                    execution.complete = false;
                }
            }
        }
    }
    if input.compare_exact {
        let mut item = evidence(
            "exact-comparison",
            DiagnosticSection::Vectors,
            &execution,
            "Exact diagnostic comparison is a backend-owned counterfactual over canonical typed IR.",
        );
        item.provenance = DiagnosticProvenance::Calculated;
        match exact_variant(&request, &input.limits) {
            Ok(plan) => {
                item.message = format!(
                    "Exact diagnostic comparison cloned the canonical typed query and changed only {}.annotations.approximate from true to false. Filters, boolean structure, rank-only branches, ranking, query inputs, target-hit budget, sorting and result controls are preserved. Overlap describes returned sets, not corpus recall.",
                    plan.query_path
                );
                item.values.insert(
                    "changed_query_path".into(),
                    Value::String(plan.query_path.clone()),
                );
                item.values.insert(
                    "changed_annotation".into(),
                    Value::String("approximate".into()),
                );
                item.values.insert(
                    "baseline_approximate".into(),
                    Value::Bool(plan.previous_approximate),
                );
                item.values
                    .insert("variant_approximate".into(), Value::Bool(false));
                item.values.insert(
                    "document_field".into(),
                    Value::String(plan.document_field.clone()),
                );
                item.values.insert(
                    "query_tensor".into(),
                    Value::String(plan.query_tensor.clone()),
                );
                item.values
                    .insert("target_hits".into(), Value::UInt64(plan.target_hits.into()));
                item.canonical_request = Some(plan.request.clone());

                match bounded_request(&plan.request, &input.limits, deadline).and_then(|variant| {
                    let variant_hash = request_hash(&variant);
                    backend
                        .search_diagnostic(variant, &[], &input.limits)
                        .map(|exact| (exact, variant_hash))
                }) {
                    Ok((mut exact, variant_hash)) => {
                        let mut exact_execution = execution.clone();
                        exact_execution.id = format!("{}/exact", execution.id);
                        exact_execution.request_hash = variant_hash;
                        exact_execution.complete = execution_complete(&exact.result);
                        item.execution_id = exact_execution.id.clone();
                        if execution.complete && exact_execution.complete {
                            let baseline = DiagnosticSourceRun {
                                execution: execution.clone(),
                                request: request.clone(),
                                result: acquired.result.clone(),
                            };
                            let comparison = compare_runs(
                                &baseline,
                                &exact_execution,
                                &exact.result,
                                request.offset(),
                            );
                            item.values.insert(
                                "overlap_count".into(),
                                Value::UInt64(comparison.overlap_count.into()),
                            );
                            item.values.insert(
                                "comparison_count".into(),
                                Value::UInt64(comparison.comparison_count.into()),
                            );
                            item.values.insert(
                                "entering".into(),
                                Value::List(
                                    comparison
                                        .entering
                                        .iter()
                                        .cloned()
                                        .map(Value::String)
                                        .collect(),
                                ),
                            );
                            item.values.insert(
                                "leaving".into(),
                                Value::List(
                                    comparison
                                        .leaving
                                        .iter()
                                        .cloned()
                                        .map(Value::String)
                                        .collect(),
                                ),
                            );
                            item.values.insert(
                                "rank_movements".into(),
                                Value::List(
                                    comparison
                                        .movements
                                        .iter()
                                        .map(|movement| {
                                            Value::Object(BTreeMap::from([
                                                ("id".into(), Value::String(movement.id.clone())),
                                                (
                                                    "baseline_position".into(),
                                                    movement
                                                        .baseline_position
                                                        .map(|v| Value::UInt64(v.into()))
                                                        .unwrap_or(Value::Null),
                                                ),
                                                (
                                                    "exact_position".into(),
                                                    movement
                                                        .rerun_position
                                                        .map(|v| Value::UInt64(v.into()))
                                                        .unwrap_or(Value::Null),
                                                ),
                                            ]))
                                        })
                                        .collect(),
                                ),
                            );
                            findings.push(DiagnosticFinding { code: "exact_retrieval_comparison".into(), message: format!("Exact retrieval produced {} returned documents absent from the diagnostic baseline; overlap was {} of {} returned documents.", comparison.entering.len(), comparison.overlap_count, comparison.comparison_count), evidence_ids: vec![item.id.clone(), "retrieval".into()] });
                        } else {
                            item.status = DiagnosticStatus::TimedOut;
                            execution.complete = false;
                        }
                        strip_document_bodies(&mut exact.result.root);
                        strip_features(&mut exact.result.root);
                        item.result = Some(exact.result);
                    }
                    Err(error) => {
                        item.message = error.message;
                        item.status = if error.code == crate::ErrorCode::DeadlineExceeded {
                            DiagnosticStatus::TimedOut
                        } else {
                            DiagnosticStatus::Failed
                        };
                        execution.complete = false;
                    }
                }
            }
            Err(reason) => {
                item.status = DiagnosticStatus::UnavailableForQuery;
                item.message = reason;
            }
        }
        evidence_items.push(item);
    }
    let mut raw_engine = acquired.raw_engine;
    for item in &mut raw_engine {
        item.execution_id = execution.id.clone();
    }
    for section in &input.requested_evidence {
        if evidence_items.iter().any(|item| &item.section == section) {
            continue;
        }
        let raw = raw_engine.iter().find(|item| &item.section == section);
        let mut item = evidence(
            &format!("section-{section:?}"),
            *section,
            &execution,
            if raw.is_some() {
                "Engine-specific evidence is available in Inspect; its raw field names are unstable."
            } else {
                "This engine integration does not provide authoritative evidence for this section."
            },
        );
        item.status = raw.map(|item| item.status).unwrap_or_else(|| {
            if acquired.observed_sections.contains(section) {
                DiagnosticStatus::Available
            } else {
                DiagnosticStatus::Unsupported
            }
        });
        if acquired.observed_sections.contains(section) {
            item.message = "Observed in this engine execution. Inspect its versioned raw trace for the complete tree; raw keys are unstable.".into();
            item.values
                .insert("raw_evidence_section".into(), Value::String("Trace".into()));
        }
        if timed_out {
            item.status = DiagnosticStatus::TimedOut;
        } else if raw.is_none()
            && raw_engine
                .iter()
                .any(|item| item.status == DiagnosticStatus::Truncated)
        {
            item.status = DiagnosticStatus::Truncated;
            item.message = "Raw diagnostic acquisition was truncated; this section's availability could not be established.".into();
        }
        evidence_items.push(item);
    }
    Ok(QueryDiagnosticsResult {
        execution,
        source_run: source.map(|source| source.execution),
        findings,
        evidence: evidence_items,
        hits,
        expected_document,
        comparison,
        raw_engine,
        warnings,
    })
}

fn strip_features(node: &mut ResultNode) {
    match node {
        ResultNode::Hit(hit) => {
            hit.match_features.clear();
            hit.summary_features.clear();
            hit.rank_features.clear();
        }
        ResultNode::Group(group) => {
            for child in &mut group.children {
                strip_features(child);
            }
        }
    }
}

/// Source snapshots and diagnostic samples never retain document bodies.
pub fn strip_document_bodies(node: &mut ResultNode) {
    match node {
        ResultNode::Hit(hit) => {
            hit.fields.clear();
            hit.highlights.clear();
        }
        ResultNode::Group(group) => {
            group.fields.clear();
            for node in &mut group.children {
                strip_document_bodies(node);
            }
        }
    }
}
