//! Backend-owned findings. No browser or engine-specific syntax is interpreted here.
use crate::{model::*, Backend, GetInput, IndexContext, PortableError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

pub const SOURCE_RUN_METADATA_KEY: &str = "searchkernel.source_run";

/// A deterministic identity over canonical resolved request bytes, not an
/// authentication token. Source-cache equality additionally checks the full IR.
pub fn request_hash(request: &SearchRequestIr) -> String {
    let bytes = crate::ir_to_api::search_request_ir_to_api(request);
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
            .and_then(|value| value.degraded.as_ref())
            .is_some_and(|value| value.timeout == Some(true) || value.ann_timeout == Some(true))
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
        results_changed: Some(
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
    let mut truncated = hits.len() > limits.feature_hit_count as usize;
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
            (
                format!("Remove explicit filter clause {} ({clause:?})", index + 1),
                variant,
            )
        })
        .collect()
}

/// Only an isolated canonical NN operator has a supported exact comparison.
/// We do not rewrite opaque YQL or arbitrary non-monotonic hybrid trees.
fn exact_variant(request: &SearchRequestIr, limits: &DiagnosticLimits) -> Option<SearchRequestIr> {
    let QueryIr::Typed(expression) = request.query() else {
        return None;
    };
    if !matches!(expression.kind, QueryExprKindIr::NearestNeighbor { .. }) {
        return None;
    }
    let target_hits = expression
        .annotations
        .get("targetNumHits")
        .or_else(|| expression.annotations.get("totalTargetHits"));
    let count = match target_hits {
        Some(RequestValueIr::Int64(value)) => u32::try_from(*value).ok()?,
        Some(RequestValueIr::UInt64(value)) => u32::try_from(*value).ok()?,
        None => 100,
        _ => return None,
    };
    if count > limits.exact_nn_hits || request.limit() > limits.exact_nn_hits {
        return None;
    }
    let mut variant = request.clone();
    let QueryIr::Typed(expression) = &mut variant.query else {
        return None;
    };
    expression
        .annotations
        .insert("approximate".into(), RequestValueIr::Bool(false));
    Some(variant)
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
    let acquired =
        backend.search_diagnostic(request.clone(), &input.requested_evidence, &input.limits)?;
    execution.complete = execution_complete(&acquired.result);
    let comparison = source
        .as_ref()
        .map(|source| compare_runs(source, &execution, &acquired.result, request.offset()));
    let mut warnings = Vec::new();
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
    let mut timing = evidence("timing", DiagnosticSection::Timing, &execution,
        "Timing from this diagnostic rerun. Original timing remains in the original search response.");
    if let Some(time) = &acquired.result.timing {
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
    let mut findings = Vec::new();
    let mut expected_document = None;
    if let Some(id) = &input.expected_document_id {
        let get = crate::resolve_get(
            context,
            GetInput {
                id: id.clone(),
                projection: Some(Vec::new()),
            },
        )?;
        let exists = backend.get(get)?.document.is_some();
        let returned = returned_hits(&acquired.result.root)
            .iter()
            .any(|hit| &hit.id == id);
        let mut item = evidence("expected-document", DiagnosticSection::Retrieval, &execution,
            "Existence is a fresh document read; returned status is observed in the diagnostic rerun. Unobserved intermediate stages remain unavailable.");
        item.values
            .insert("document_exists".into(), Value::Bool(exists));
        item.values.insert("returned".into(), Value::Bool(returned));
        evidence_items.push(item);
        let observed = |value| {
            if value {
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
                ("Returned", observed(returned)),
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
        if !exists || !returned {
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
            .take(input.limits.variant_count as usize)
            .enumerate()
        {
            let id = format!("filter-variant-{}", i + 1);
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
                    } else if execution_complete(&acquired.result)
                        && ((acquired.result.total_hits.value == 0 && result.total_hits.value > 0)
                            || (expected_returned
                                && input.expected_document_id.as_ref().is_some_and(|id| {
                                    !returned_hits(&acquired.result.root)
                                        .iter()
                                        .any(|hit| &hit.id == id)
                                })))
                    {
                        findings.push(DiagnosticFinding {
                            code: "filter_variant_sufficient".into(),
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
        let mut item = evidence("exact-comparison", DiagnosticSection::Vectors, &execution,
            "Exact diagnostic comparison uses the same canonical NN query, filters, ranking and target-hit budget; only approximate=false changes. Overlap describes these returned sets, not corpus recall.");
        if let Some(variant) = exact_variant(&input.request, &input.limits) {
            match bounded_request(&variant, &input.limits, deadline).and_then(|variant| {
                item.canonical_request = Some(variant.clone());
                backend.search_diagnostic(variant, &[], &input.limits)
            }) {
                Ok(mut exact) => {
                    let mut exact_execution = execution.clone();
                    exact_execution.id = format!("{}/exact", execution.id);
                    exact_execution.request_hash =
                        request_hash(item.canonical_request.as_ref().unwrap_or(&request));
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
                        item.provenance = DiagnosticProvenance::Calculated;
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
        } else {
            item.status = DiagnosticStatus::UnavailableForQuery;
            item.message = "Exact comparison requires an isolated canonical nearest-neighbor query within the exact-NN hit budget. Opaque YQL and arbitrary hybrid trees are not rewritten.".into();
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
