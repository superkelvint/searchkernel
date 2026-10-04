use super::*;
use result::{relevance_offset, value_entries_offset, value_offset};
use search::resolved_request_offset;

macro_rules! enum_conversion {
    ($function:ident, $name:ident, $($variant:ident),+) => {
        fn $function(value: ir::$name) -> api::$name {
            match value { $(ir::$name::$variant => api::$name::$variant),+ }
        }
    };
}
enum_conversion!(
    section,
    DiagnosticSection,
    Query,
    Retrieval,
    Ranking,
    Timing,
    MatchedTerms,
    Vectors,
    Trace,
    MatchingProfiler,
    FirstPhaseProfiler,
    SecondPhaseProfiler,
    Blueprint,
    IteratorTree,
    RawEngine,
    Counterfactual
);
enum_conversion!(
    status,
    DiagnosticStatus,
    Available,
    Unsupported,
    TimedOut,
    Truncated,
    UnavailableForQuery,
    Failed
);
enum_conversion!(
    provenance,
    DiagnosticProvenance,
    Profile,
    Engine,
    Calculated
);
enum_conversion!(
    kind,
    DiagnosticExecutionKind,
    Original,
    DiagnosticRerun,
    Experiment,
    Counterfactual
);
enum_conversion!(
    observation,
    DiagnosticObservation,
    Unavailable,
    ObservedYes,
    ObservedNo
);

fn execution_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::DiagnosticExecution,
) -> flatbuffers::WIPOffset<api::DiagnosticExecution<'a>> {
    let id = builder.create_string(&value.id);
    let request_hash = builder.create_string(&value.request_hash);
    let target = value.target.as_ref().map(|name| {
        let name = builder.create_string(name);
        let generation = value
            .generation
            .as_ref()
            .map(|value| builder.create_string(value));
        api::IndexTarget::create(
            builder,
            &api::IndexTargetArgs {
                name: Some(name),
                generation,
            },
        )
    });
    api::DiagnosticExecution::create(
        builder,
        &api::DiagnosticExecutionArgs {
            id: Some(id),
            request_hash: Some(request_hash),
            target,
            kind: kind(value.kind),
            complete: value.complete,
        },
    )
}

pub fn query_diagnostics_result_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::QueryDiagnosticsResult,
) -> Result<flatbuffers::WIPOffset<api::QueryDiagnosticsResult<'a>>, PortableError> {
    let execution = execution_offset(builder, &value.execution);
    let source_run = value
        .source_run
        .as_ref()
        .map(|value| execution_offset(builder, value));
    let mut findings = Vec::new();
    for finding in &value.findings {
        let code = builder.create_string(&finding.code);
        let message = builder.create_string(&finding.message);
        let evidence_ids = string_vector_offset(builder, &finding.evidence_ids);
        findings.push(api::DiagnosticFinding::create(
            builder,
            &api::DiagnosticFindingArgs {
                code: Some(code),
                message: Some(message),
                evidence_ids,
            },
        ));
    }
    let findings = builder.create_vector(&findings);
    let mut evidence = Vec::new();
    for item in &value.evidence {
        let id = builder.create_string(&item.id);
        let execution_id = builder.create_string(&item.execution_id);
        let message = builder.create_string(&item.message);
        let values = value_entries_offset(builder, &item.values);
        let canonical_request = item
            .canonical_request
            .as_ref()
            .map(|value| resolved_request_offset(builder, value));
        let result = item
            .result
            .as_ref()
            .map(|value| search_result_offset(builder, value))
            .transpose()?;
        evidence.push(api::DiagnosticEvidence::create(
            builder,
            &api::DiagnosticEvidenceArgs {
                id: Some(id),
                section: section(item.section),
                status: status(item.status),
                provenance: provenance(item.provenance),
                execution_id: Some(execution_id),
                message: Some(message),
                values,
                canonical_request,
                result,
            },
        ));
    }
    let evidence = builder.create_vector(&evidence);
    let mut hits = Vec::new();
    for hit in &value.hits {
        let id = builder.create_string(&hit.id);
        let execution_id = builder.create_string(&hit.execution_id);
        let relevance = hit.relevance.map(|score| relevance_offset(builder, score));
        let evidence_ids = string_vector_offset(builder, &hit.evidence_ids);
        let mut signals = Vec::new();
        for signal in &hit.signals {
            let name = builder.create_string(&signal.name);
            let value = value_offset(builder, &signal.value);
            let comparison_set = builder.create_string(&signal.comparison_set);
            let position = signal.position.map(api::OptionalUInt32::new);
            let ties = signal.ties.map(api::OptionalUInt32::new);
            signals.push(api::DiagnosticSignal::create(
                builder,
                &api::DiagnosticSignalArgs {
                    name: Some(name),
                    value: Some(value),
                    position: position.as_ref(),
                    ties: ties.as_ref(),
                    comparison_count: signal.comparison_count,
                    comparison_set: Some(comparison_set),
                },
            ));
        }
        let signals = builder.create_vector(&signals);
        hits.push(api::DiagnosticHit::create(
            builder,
            &api::DiagnosticHitArgs {
                id: Some(id),
                execution_id: Some(execution_id),
                returned_position: hit.returned_position,
                relevance,
                signals: Some(signals),
                evidence_ids,
            },
        ));
    }
    let hits = builder.create_vector(&hits);
    let expected_document = value.expected_document.as_ref().map(|document| {
        let id = builder.create_string(&document.id);
        let mut stages = Vec::new();
        for stage in &document.stages {
            let name = builder.create_string(&stage.name);
            let evidence_ids = string_vector_offset(builder, &stage.evidence_ids);
            stages.push(api::DiagnosticDocumentStage::create(
                builder,
                &api::DiagnosticDocumentStageArgs {
                    name: Some(name),
                    observation: observation(stage.observation),
                    evidence_ids,
                },
            ));
        }
        let stages = builder.create_vector(&stages);
        api::DiagnosticExpectedDocument::create(
            builder,
            &api::DiagnosticExpectedDocumentArgs {
                id: Some(id),
                stages: Some(stages),
            },
        )
    });
    let comparison = value.comparison.as_ref().map(|value| {
        let baseline_execution_id = builder.create_string(&value.baseline_execution_id);
        let rerun_execution_id = builder.create_string(&value.rerun_execution_id);
        let results_changed = value.results_changed.map(api::OptionalBool::new);
        let entering = string_vector_offset(builder, &value.entering);
        let leaving = string_vector_offset(builder, &value.leaving);
        let mut movements = Vec::new();
        for movement in &value.movements {
            let id = builder.create_string(&movement.id);
            let baseline_position = movement.baseline_position.map(api::OptionalUInt32::new);
            let rerun_position = movement.rerun_position.map(api::OptionalUInt32::new);
            movements.push(api::DiagnosticRankMovement::create(
                builder,
                &api::DiagnosticRankMovementArgs {
                    id: Some(id),
                    baseline_position: baseline_position.as_ref(),
                    rerun_position: rerun_position.as_ref(),
                },
            ));
        }
        let movements = builder.create_vector(&movements);
        api::DiagnosticComparison::create(
            builder,
            &api::DiagnosticComparisonArgs {
                baseline_execution_id: Some(baseline_execution_id),
                rerun_execution_id: Some(rerun_execution_id),
                results_changed: results_changed.as_ref(),
                overlap_count: value.overlap_count,
                comparison_count: value.comparison_count,
                entering,
                leaving,
                movements: Some(movements),
            },
        )
    });
    let mut raw_engine = Vec::new();
    for item in &value.raw_engine {
        let engine = builder.create_string(&item.engine);
        let version = builder.create_string(&item.version);
        let execution_id = builder.create_string(&item.execution_id);
        let media_type = builder.create_string(&item.media_type);
        let payload = builder.create_string(&item.payload);
        raw_engine.push(api::RawEngineDiagnostics::create(
            builder,
            &api::RawEngineDiagnosticsArgs {
                engine: Some(engine),
                version: Some(version),
                execution_id: Some(execution_id),
                section: section(item.section),
                status: status(item.status),
                media_type: Some(media_type),
                payload: Some(payload),
            },
        ));
    }
    let raw_engine = builder.create_vector(&raw_engine);
    let warnings = string_vector_offset(builder, &value.warnings);
    Ok(api::QueryDiagnosticsResult::create(
        builder,
        &api::QueryDiagnosticsResultArgs {
            execution: Some(execution),
            source_run,
            findings: Some(findings),
            evidence: Some(evidence),
            hits: Some(hits),
            expected_document,
            comparison,
            raw_engine: Some(raw_engine),
            warnings,
        },
    ))
}
