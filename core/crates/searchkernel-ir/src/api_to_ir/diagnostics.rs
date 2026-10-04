use crate::{model::*, IndexContext, PortableError, Result};
use searchkernel_protocol::wire::v1 as api;

fn invalid(path: &str, message: &str) -> PortableError {
    PortableError::invalid("invalid_diagnostic_request", Some(path), message)
}

pub fn query_diagnostics_to_ir(
    context: &IndexContext,
    input: api::QueryDiagnosticsRequest<'_>,
) -> Result<QueryDiagnosticsRequestIr> {
    let intent = match input.intent() {
        api::DiagnosticIntent::ExplainHit => DiagnosticIntent::ExplainHit,
        api::DiagnosticIntent::ZeroResults => DiagnosticIntent::ZeroResults,
        api::DiagnosticIntent::MissingDocument => DiagnosticIntent::MissingDocument,
        api::DiagnosticIntent::RankingComparison => DiagnosticIntent::RankingComparison,
        api::DiagnosticIntent::Overmatching => DiagnosticIntent::Overmatching,
        api::DiagnosticIntent::QueryInterpretation => DiagnosticIntent::QueryInterpretation,
        api::DiagnosticIntent::VectorRetrieval => DiagnosticIntent::VectorRetrieval,
        api::DiagnosticIntent::Performance => DiagnosticIntent::Performance,
        api::DiagnosticIntent::DeepEngine => DiagnosticIntent::DeepEngine,
        api::DiagnosticIntent::Experiment => DiagnosticIntent::Experiment,
        _ => {
            return Err(invalid(
                "intent",
                "a supported diagnostic intent is required",
            ))
        }
    };
    let request = context.search_to_ir(
        input
            .request()
            .ok_or_else(|| invalid("request", "canonical search request is required"))?,
    )?;
    let experiment = input
        .experiment()
        .map(|value| context.search_to_ir(value))
        .transpose()?;
    if (intent == DiagnosticIntent::Experiment) != experiment.is_some() {
        return Err(invalid(
            "experiment",
            "an experimental request is required only for Experiment intent",
        ));
    }
    let selected_hits: Vec<String> = input
        .selected_hits()
        .map(|values| values.iter().map(str::to_owned).collect())
        .unwrap_or_default();
    if selected_hits.len() > 2
        || selected_hits
            .iter()
            .any(|id| id.len() > 1024 || id.is_empty())
    {
        return Err(invalid(
            "selected_hits",
            "select at most two nonempty document IDs, each at most 1024 bytes",
        ));
    }
    let expected_document_id = input.expected_document_id().map(str::to_owned);
    if intent == DiagnosticIntent::MissingDocument && expected_document_id.is_none() {
        return Err(invalid(
            "expected_document_id",
            "select an expected document",
        ));
    }
    if let Some(id) = &expected_document_id {
        if id.len() > 1024 || DocumentId::parse(id.clone()).is_err() {
            return Err(invalid(
                "expected_document_id",
                "invalid expected document ID",
            ));
        }
    }
    let source_run_id = input.source_run_id().map(str::to_owned);
    if source_run_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.len() > 256)
    {
        return Err(invalid("source_run_id", "invalid source run identity"));
    }
    let mut requested_evidence = Vec::new();
    for section in input.requested_evidence().into_iter().flatten() {
        let section = match section {
            api::DiagnosticSection::Query => DiagnosticSection::Query,
            api::DiagnosticSection::Retrieval => DiagnosticSection::Retrieval,
            api::DiagnosticSection::Ranking => DiagnosticSection::Ranking,
            api::DiagnosticSection::Timing => DiagnosticSection::Timing,
            api::DiagnosticSection::MatchedTerms => DiagnosticSection::MatchedTerms,
            api::DiagnosticSection::Vectors => DiagnosticSection::Vectors,
            api::DiagnosticSection::Trace => DiagnosticSection::Trace,
            api::DiagnosticSection::MatchingProfiler => DiagnosticSection::MatchingProfiler,
            api::DiagnosticSection::FirstPhaseProfiler => DiagnosticSection::FirstPhaseProfiler,
            api::DiagnosticSection::SecondPhaseProfiler => DiagnosticSection::SecondPhaseProfiler,
            api::DiagnosticSection::Blueprint => DiagnosticSection::Blueprint,
            api::DiagnosticSection::IteratorTree => DiagnosticSection::IteratorTree,
            api::DiagnosticSection::RawEngine => DiagnosticSection::RawEngine,
            api::DiagnosticSection::Counterfactual => DiagnosticSection::Counterfactual,
            _ => {
                return Err(invalid(
                    "requested_evidence",
                    "unknown diagnostic evidence section",
                ))
            }
        };
        if !requested_evidence.contains(&section) {
            requested_evidence.push(section);
        }
    }
    let mut limits = DiagnosticLimits::default();
    if let Some(value) = input.limits() {
        macro_rules! limit {
            ($name:ident, $max:expr) => {
                if let Some(value) = value.$name() {
                    if value.value() == 0 || value.value() > $max {
                        return Err(invalid(
                            concat!("limits.", stringify!($name)),
                            concat!(stringify!($name), " exceeds diagnostic bounds"),
                        ));
                    }
                    limits.$name = value.value();
                }
            };
        }
        limit!(timeout_ms, 30_000);
        limit!(variant_count, 16);
        limit!(feature_hit_count, 100);
        limit!(tensor_cells, 4096);
        limit!(raw_bytes, 262_144);
        limit!(trace_depth, 9);
        limit!(profiler_depth, 16);
        limit!(exact_nn_hits, 1000);
    }
    Ok(QueryDiagnosticsRequestIr {
        request,
        intent,
        requested_evidence,
        source_run_id,
        selected_hits,
        expected_document_id,
        experiment,
        compare_exact: input.compare_exact(),
        limits,
    })
}
