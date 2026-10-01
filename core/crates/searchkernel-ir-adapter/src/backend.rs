//! Vespa-backed implementation of the portable backend contract.

use crate::search::{preflight_search_request, GroupingLowering};
use crate::{
    analysis_options, internal, lower_document, lower_field_value, native_error,
    native_search_error, project_document, value_to_ir,
};
use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, AnalyzedToken, Backend, Coverage, CoverageDegraded,
    DeleteRequestIr, DeleteResult, FieldValueIr, GetRequestIr, GetResult, InsertRequestIr,
    InsertResult, PatchOperationIr, PatchRequestIr, PortableError, Result, ScalarValueIr,
    SearchRequestIr, SearchResult, TotalHitsRelation, UpdateArithmeticOperatorIr,
    UpdateOperationIr, UpdateRequestIr, UpdateResult, UpsertRequestIr, UpsertResult,
};
#[cfg(test)]
use searchkernel_vespa_engine::GroupingAggregation;
use searchkernel_vespa_engine::{
    DocumentUpdate, Engine, FieldValue, GroupingOutput, GroupingPlan, GroupingResult,
    GroupingResultValue, MatchFeatureValue, ScalarValue,
    TotalHitsRelation as VespaTotalHitsRelation, Value as VespaValue,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) fn remaining_search_timeout(deadline: Instant, now: Instant) -> Result<Duration> {
    let remaining = deadline.checked_duration_since(now).unwrap_or_default();
    if remaining.is_zero() {
        return Err(PortableError::new(
            searchkernel_ir::ErrorCode::DeadlineExceeded,
            "portable search deadline exceeded before grouping completed",
        ));
    }
    Ok(remaining)
}

/// Map the pinned Vespa `Coverage` counters to the portable result model.
/// Vespa keeps `covered`, `active`, and `targetActive` distinct: the first is
/// what this execution searched, the second is the currently searchable
/// corpus, and the third is the eventual ideal corpus. The portable percentage
/// describes this embedded execution, so its denominator is `active`; the
/// target corpus is deliberately not inferred as a portable non-ideal-state
/// flag. This follows the distinction in the pinned `Coverage` and
/// `container.handler.Coverage` sources.
pub(crate) fn portable_coverage(coverage: searchkernel_vespa_engine::SearchCoverage) -> Coverage {
    let degraded = coverage.degraded_by_match_phase()
        || coverage.degraded_by_timeout()
        || coverage.degraded_by_ann_timeout();
    let full = coverage.covered >= coverage.active && !degraded;
    let coverage_percent = if coverage.active == 0 {
        if degraded {
            0.0
        } else {
            100.0
        }
    } else if coverage.covered < coverage.active {
        (coverage.covered.min(coverage.active) as f64 * 100.0 / coverage.active as f64).round()
    } else {
        100.0
    };
    Coverage {
        coverage_percent: Some(coverage_percent),
        documents: Some(coverage.covered),
        full: Some(full),
        nodes: Some(1),
        results: Some(1),
        results_full: Some(u64::from(full)),
        degraded: Some(CoverageDegraded {
            match_phase: Some(coverage.degraded_by_match_phase()),
            timeout: Some(coverage.degraded_by_timeout()),
            adaptive_timeout: None,
            ann_timeout: Some(coverage.degraded_by_ann_timeout()),
            non_ideal_state: None,
            metadata: std::collections::BTreeMap::new(),
        }),
        metadata: std::collections::BTreeMap::new(),
    }
}

/// `UniqueIssues` are not portable semantic errors. Keep this policy explicit
/// until the portable result contract grows an issue channel rather than treating
/// Vespa diagnostics as request failures.
pub(crate) fn portable_errors_from_native_issues(
    _issues: &[String],
) -> Vec<searchkernel_ir::SearchResultError> {
    Vec::new()
}

/// A real Vespa-backed implementation of the portable backend contract.
pub struct VespaBackend {
    pub(crate) engine: Engine,
    existence_mutation_locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    #[cfg(test)]
    mutation_probe: Option<std::sync::Arc<MutationProbe>>,
    #[cfg(test)]
    search_grouping_probe: Option<std::sync::Arc<SearchGroupingProbe>>,
}

#[cfg(test)]
pub(crate) struct MutationProbe {
    pub(crate) rendezvous: std::sync::Barrier,
    pub(crate) release: std::sync::Barrier,
    pub(crate) update_completed: Option<std::sync::mpsc::Sender<()>>,
}

#[cfg(test)]
pub(crate) struct SearchGroupingProbe {
    pub(crate) after_primary: std::sync::Barrier,
    pub(crate) continue_search: std::sync::Barrier,
}

impl VespaBackend {
    pub fn open(
        data_dir: impl AsRef<std::path::Path>,
        config_dir: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        Engine::open(data_dir, config_dir)
            .map(Self::from_engine)
            .map_err(native_error)
    }

    pub fn from_engine(engine: Engine) -> Self {
        Self {
            engine,
            existence_mutation_locks: Mutex::new(HashMap::new()),
            #[cfg(test)]
            mutation_probe: None,
            #[cfg(test)]
            search_grouping_probe: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_mutation_probe(
        engine: Engine,
        probe: std::sync::Arc<MutationProbe>,
    ) -> Self {
        Self {
            engine,
            existence_mutation_locks: Mutex::new(HashMap::new()),
            mutation_probe: Some(probe),
            search_grouping_probe: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_search_grouping_probe(
        engine: Engine,
        probe: std::sync::Arc<SearchGroupingProbe>,
    ) -> Self {
        Self {
            engine,
            existence_mutation_locks: Mutex::new(HashMap::new()),
            mutation_probe: None,
            search_grouping_probe: Some(probe),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    fn existence_mutation_lock(&self, id: &str) -> Result<Arc<Mutex<()>>> {
        let mut locks = self
            .existence_mutation_locks
            .lock()
            .map_err(|_| internal("existence mutation lock table is poisoned"))?;
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            return Ok(lock);
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(id.to_owned(), Arc::downgrade(&lock));
        // Weak entries do not retain per-document locks. Opportunistically
        // remove dead entries so high-cardinality document IDs cannot grow
        // this coordination table without bound.
        if locks.len() > 256 {
            locks.retain(|_, candidate| candidate.strong_count() != 0);
        }
        Ok(lock)
    }
}

fn facet_result_node(
    grouping: searchkernel_vespa_engine::FacetResult,
    facet: crate::search::FacetLowering,
) -> Result<searchkernel_ir::ResultNode> {
    let groups = grouping
        .groups
        .into_iter()
        .map(|group| {
            let mut fields = std::collections::BTreeMap::new();
            fields.insert("count".to_owned(), searchkernel_ir::Value::UInt64(group.count));
            let value = match group.key {
                VespaValue::String(value) => ScalarValue::String(value),
                VespaValue::Integer(value) => ScalarValue::Integer(value),
                VespaValue::Boolean(value) => ScalarValue::Boolean(value),
                _ => {
                    return Err(PortableError::unsupported(
                        "facet_key_type_not_supported",
                        Some("grouping"),
                        "native grouping returned a key kind not representable by the portable result value contract",
                    ))
                }
            };
            Ok(searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                id: None,
                label: None,
                value: Some(value_to_ir(&FieldValue::Scalar(value))?),
                fields,
                children: Vec::new(),
                continuations: std::collections::BTreeMap::new(),
                source: None,
                types: Vec::new(),
                score: None,
                limits: None,
                metadata: std::collections::BTreeMap::new(),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(searchkernel_ir::ResultNode::Group(
        searchkernel_ir::ResultGroup {
            id: Some(facet.field.clone()),
            label: Some(facet.field),
            value: None,
            fields: std::collections::BTreeMap::new(),
            children: groups,
            continuations: std::collections::BTreeMap::new(),
            source: None,
            types: Vec::new(),
            score: None,
            limits: None,
            metadata: std::collections::BTreeMap::new(),
        },
    ))
}

fn grouping_result_node(
    result: GroupingResult,
    plan: &GroupingPlan,
) -> Result<searchkernel_ir::ResultNode> {
    grouping_result_group(result, plan, None).map(searchkernel_ir::ResultNode::Group)
}

fn insert_unique_grouping_field(
    fields: &mut std::collections::BTreeMap<String, searchkernel_ir::Value>,
    label: String,
    value: searchkernel_ir::Value,
) -> Result<()> {
    if fields.insert(label.clone(), value).is_some() {
        return Err(internal(format!(
            "duplicate grouping result field `{label}` reached adapter assembly"
        )));
    }
    Ok(())
}

fn grouping_result_group(
    result: GroupingResult,
    plan: &GroupingPlan,
    level: Option<usize>,
) -> Result<searchkernel_ir::ResultGroup> {
    let fields = match level {
        None => {
            if result.outputs.len() != plan.root_aggregations.len() {
                return Err(internal(
                    "native grouping returned an unexpected root output count",
                ));
            }
            let mut fields = std::collections::BTreeMap::new();
            for (index, output) in result.outputs.iter().enumerate() {
                let label = plan
                    .root_output_labels
                    .get(index)
                    .and_then(|label| label.clone())
                    .ok_or_else(|| {
                        internal("lowered grouping root output has no canonical result-field name")
                    })?;
                insert_unique_grouping_field(
                    &mut fields,
                    label,
                    grouping_output_value(output.clone())?,
                )?;
            }
            fields
        }
        Some(level) => {
            let grouping_level = plan
                .levels
                .get(level)
                .ok_or_else(|| internal("native grouping returned a node below its plan"))?;
            if result.outputs.len() != grouping_level.aggregations.len() {
                return Err(internal(
                    "native grouping returned an unexpected output count",
                ));
            }
            let mut fields = std::collections::BTreeMap::new();
            for (index, output) in result.outputs.iter().enumerate().filter(|(index, _)| {
                grouping_level
                    .output_visible
                    .get(*index)
                    .copied()
                    .unwrap_or(true)
            }) {
                let label = grouping_level
                    .output_labels
                    .get(index)
                    .and_then(|label| label.clone())
                    .ok_or_else(|| {
                        internal("lowered grouping output has no canonical result-field name")
                    })?;
                insert_unique_grouping_field(
                    &mut fields,
                    label,
                    grouping_output_value(output.clone())?,
                )?;
            }
            fields
        }
    };
    let child_level = level.map_or(0, |level| level + 1);
    let mut children = Vec::new();
    for (index, output) in result.outputs.iter().enumerate() {
        if !matches!(output, GroupingOutput::HitList(_)) {
            continue;
        }
        if level.is_some()
            && !plan
                .levels
                .get(level.expect("level is known"))
                .and_then(|level| level.output_visible.get(index))
                .copied()
                .unwrap_or(true)
        {
            continue;
        }
        if let GroupingOutput::HitList(ids) = output {
            for hit in ids {
                children.push(searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                    id: hit.document_id.clone(),
                    score: Some(hit.score),
                    fields: hit
                        .fields
                        .iter()
                        .map(|(name, value)| value_to_ir(value).map(|value| (name.clone(), value)))
                        .collect::<Result<std::collections::BTreeMap<_, _>>>()?,
                    ..Default::default()
                }));
            }
        }
    }
    children.extend(
        result
            .groups
            .into_iter()
            .map(|child| grouping_result_group(child, plan, Some(child_level)))
            .map(|child| child.map(searchkernel_ir::ResultNode::Group))
            .collect::<Result<Vec<_>>>()?,
    );
    Ok(searchkernel_ir::ResultGroup {
        id: level.map(|index| format!("grouping-{index}")),
        label: match level {
            None => plan.root_label.clone(),
            Some(index) => plan.levels.get(index).and_then(|level| level.label.clone()),
        },
        value: result.key.map(grouping_value_to_ir),
        fields,
        children,
        continuations: std::collections::BTreeMap::new(),
        source: None,
        types: Vec::new(),
        score: None,
        limits: None,
        metadata: std::collections::BTreeMap::new(),
    })
}

fn grouping_output_value(output: GroupingOutput) -> Result<searchkernel_ir::Value> {
    match output {
        GroupingOutput::Value(value) => Ok(grouping_value_to_ir(value)),
        GroupingOutput::HitList(values) => Ok(searchkernel_ir::Value::List(
            values
                .into_iter()
                .map(|hit| searchkernel_ir::Value::String(hit.document_id))
                .collect(),
        )),
    }
}

fn grouping_value_to_ir(value: GroupingResultValue) -> searchkernel_ir::Value {
    match value {
        GroupingResultValue::String(value) => searchkernel_ir::Value::String(value),
        GroupingResultValue::Integer(value) => searchkernel_ir::Value::Int64(value),
        GroupingResultValue::Float(value) => searchkernel_ir::Value::Float64(value),
        GroupingResultValue::Boolean(value) => searchkernel_ir::Value::Bool(value),
        GroupingResultValue::Quantiles(values) => searchkernel_ir::Value::List(
            values
                .into_iter()
                .map(|(quantile, value)| {
                    searchkernel_ir::Value::Object(std::collections::BTreeMap::from([
                        (
                            "quantile".to_owned(),
                            searchkernel_ir::Value::Float64(quantile),
                        ),
                        ("value".to_owned(), searchkernel_ir::Value::Float64(value)),
                    ]))
                })
                .collect(),
        ),
    }
}

impl Backend for VespaBackend {
    fn insert(&self, request: InsertRequestIr) -> Result<InsertResult> {
        let document = lower_document(request.document())?;
        let id = document.id().to_owned();
        let existence_mutation_lock = self.existence_mutation_lock(&id)?;
        let _existence_mutation_guard = existence_mutation_lock
            .lock()
            .map_err(|_| internal("document existence mutation lock is poisoned"))?;
        if self.engine.get(&id).map_err(native_error)?.is_some() {
            return Err(PortableError::new(
                searchkernel_ir::ErrorCode::AlreadyExists,
                format!("document already exists: {id}"),
            ));
        }
        #[cfg(test)]
        if let Some(probe) = &self.mutation_probe {
            probe.rendezvous.wait();
            probe.release.wait();
        }
        self.engine.put(document).map_err(native_error)?;
        Ok(InsertResult { id })
    }

    fn upsert(&self, request: UpsertRequestIr) -> Result<UpsertResult> {
        if request.condition().is_some() {
            return Err(PortableError::unsupported(
                "conditional_mutation_not_supported",
                Some("condition"),
                "the current native Vespa adapter cannot execute upsert test-and-set conditions",
            ));
        }
        let document = lower_document(request.document())?;
        let id = document.id().to_owned();
        #[cfg(test)]
        if let Some(probe) = &self.mutation_probe {
            probe.rendezvous.wait();
        }
        let existence_mutation_lock = self.existence_mutation_lock(&id)?;
        let _existence_mutation_guard = existence_mutation_lock
            .lock()
            .map_err(|_| internal("document existence mutation lock is poisoned"))?;
        self.engine.put(document).map_err(native_error)?;
        Ok(UpsertResult { id })
    }

    fn patch(&self, request: PatchRequestIr) -> Result<searchkernel_ir::PatchResult> {
        let mut update = DocumentUpdate::new(request.id().as_str());
        for operation in request.operations() {
            match operation {
                PatchOperationIr::Set { field, value } => {
                    let lowered = lower_field_value(value)?;
                    update = update.assign(field.as_str(), lowered.clone());
                    if let FieldValueIr::Single(ScalarValueIr::String(text)) = value {
                        let analysis = searchkernel_vespa_engine::Linguistics.analyze_document(
                            text,
                            searchkernel_vespa_engine::LinguisticsOptions::default(),
                        );
                        update = update.annotate_text(field.as_str(), analysis);
                    }
                    if let FieldValueIr::Multi(values) = value {
                        let texts = values
                            .iter()
                            .map(|value| match value {
                                ScalarValueIr::String(text) => Ok(text),
                                _ => Err(()),
                            })
                            .collect::<std::result::Result<Vec<_>, _>>();
                        if let Ok(texts) = texts {
                            let analyses = texts
                                .into_iter()
                                .map(|text| {
                                    searchkernel_vespa_engine::Linguistics.analyze_document(
                                        text,
                                        searchkernel_vespa_engine::LinguisticsOptions::default(),
                                    )
                                })
                                .collect();
                            update = update.annotate_text_array(field.as_str(), analyses);
                        }
                    }
                }
                PatchOperationIr::Unset { field } => update = update.clear(field.as_str()),
            }
        }
        self.engine.update(update).map_err(native_error)?;
        Ok(searchkernel_ir::PatchResult {
            id: request.id().to_string(),
        })
    }

    fn update(&self, request: UpdateRequestIr) -> Result<UpdateResult> {
        if request.condition().is_some() {
            return Err(PortableError::unsupported(
                "conditional_mutation_not_supported",
                Some("condition"),
                "the current embedded Vespa boundary cannot execute update test-and-set conditions atomically",
            ));
        }
        let mut update = DocumentUpdate::new(request.id().as_str());
        if let Some(create) = request.create() {
            update = update.create_if_nonexistent(create);
        }
        for operation in request.operations() {
            match operation {
                UpdateOperationIr::Assign { target, value } => {
                    update = update.assign(target.as_str(), lower_field_value(value)?);
                    if let FieldValueIr::Single(ScalarValueIr::String(text)) = value {
                        let analysis = searchkernel_vespa_engine::Linguistics.analyze_document(
                            text,
                            searchkernel_vespa_engine::LinguisticsOptions::default(),
                        );
                        update = update.annotate_text(target.as_str(), analysis);
                    }
                    if let FieldValueIr::Multi(values) = value {
                        let texts = values
                            .iter()
                            .map(|value| match value {
                                ScalarValueIr::String(text) => Ok(text),
                                _ => Err(()),
                            })
                            .collect::<std::result::Result<Vec<_>, _>>();
                        if let Ok(texts) = texts {
                            let analyses = texts
                                .into_iter()
                                .map(|text| {
                                    searchkernel_vespa_engine::Linguistics.analyze_document(
                                        text,
                                        searchkernel_vespa_engine::LinguisticsOptions::default(),
                                    )
                                })
                                .collect();
                            update = update.annotate_text_array(target.as_str(), analyses);
                        }
                    }
                }
                UpdateOperationIr::Clear { target } => {
                    update = update.clear(target.as_str());
                }
                UpdateOperationIr::Arithmetic {
                    target,
                    operator,
                    operand,
                } => {
                    let operator = match operator {
                        UpdateArithmeticOperatorIr::Add | UpdateArithmeticOperatorIr::Increment => {
                            searchkernel_vespa_engine::ArithmeticOperator::Add
                        }
                        UpdateArithmeticOperatorIr::Subtract
                        | UpdateArithmeticOperatorIr::Decrement => {
                            searchkernel_vespa_engine::ArithmeticOperator::Subtract
                        }
                        UpdateArithmeticOperatorIr::Multiply => {
                            searchkernel_vespa_engine::ArithmeticOperator::Multiply
                        }
                        UpdateArithmeticOperatorIr::Divide => {
                            searchkernel_vespa_engine::ArithmeticOperator::Divide
                        }
                    };
                    update = update.arithmetic(target.as_str(), operator, *operand);
                }
            }
        }
        // create_if_nonexistent(true) is an existence-changing mutation and
        // must share the same serialization boundary as insert/upsert. Ordinary
        // updates cannot create a missing document and remain outside this lock.
        let existence_mutation_lock = if request.create() == Some(true) {
            Some(self.existence_mutation_lock(request.id().as_str())?)
        } else {
            None
        };
        let _existence_mutation_guard = existence_mutation_lock
            .as_ref()
            .map(|lock| {
                lock.lock()
                    .map_err(|_| internal("document existence mutation lock is poisoned"))
            })
            .transpose()?;
        self.engine.update(update).map_err(native_error)?;
        #[cfg(test)]
        if let Some(probe) = &self.mutation_probe {
            if let Some(update_completed) = &probe.update_completed {
                let _ = update_completed.send(());
            }
        }
        Ok(UpdateResult {
            id: request.id().to_string(),
        })
    }

    fn delete(&self, request: DeleteRequestIr) -> Result<DeleteResult> {
        if request.condition().is_some() {
            return Err(PortableError::unsupported(
                "conditional_mutation_not_supported",
                Some("condition"),
                "the current native Vespa adapter cannot execute delete test-and-set conditions",
            ));
        }
        let deleted = self
            .engine
            .delete(request.id().as_str())
            .map_err(native_error)?;
        Ok(DeleteResult {
            id: request.id().to_string(),
            deleted,
        })
    }

    fn get(&self, request: GetRequestIr) -> Result<GetResult> {
        let document = self
            .engine
            .get(request.id().as_str())
            .map_err(native_error)?;
        let Some(document) = document else {
            return Ok(GetResult { document: None });
        };
        let fields = project_document(&document, request.projection().fields())?;
        Ok(GetResult {
            document: Some(searchkernel_ir::DocumentResult {
                id: document.id().to_owned(),
                fields,
            }),
        })
    }

    fn search(&self, request: SearchRequestIr) -> Result<SearchResult> {
        let preflight = preflight_search_request(&request)?;
        let groupings = preflight.groupings;
        let m_request = preflight.request;
        let expected_facets = m_request.facets.len();
        let native_plans = groupings
            .iter()
            .filter_map(|grouping| match grouping {
                GroupingLowering::Facet(_) => None,
                GroupingLowering::Native(native) => Some(native.plan.clone()),
            })
            .collect::<Vec<_>>();
        let result = self
            .engine
            .search_with_grouping_plans(m_request, &native_plans)
            .map_err(|error| native_search_error(error, "sort_field_type_not_supported"))?;
        #[cfg(test)]
        if let Some(probe) = &self.search_grouping_probe {
            probe.after_primary.wait();
            probe.continue_search.wait();
        }
        if result.facets.len() != expected_facets {
            return Err(internal(
                "native search returned an unexpected grouping count",
            ));
        }
        if result.groupings.len() != native_plans.len() {
            return Err(internal(
                "native search returned an unexpected structured grouping count",
            ));
        }
        let mut children = result
            .hits
            .into_iter()
            .map(|hit| {
                Ok(searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                    id: hit.document_id,
                    score: Some(hit.score),
                    fields: hit
                        .fields
                        .into_iter()
                        .map(|(name, value)| value_to_ir(&value).map(|value| (name, value)))
                        .collect::<Result<std::collections::BTreeMap<_, _>>>()?,
                    highlights: hit
                        .highlights
                        .into_iter()
                        .map(|highlight| searchkernel_ir::HighlightResult {
                            field: highlight.field,
                            fragments: highlight
                                .fragments
                                .into_iter()
                                .map(|fragment| searchkernel_ir::HighlightFragment {
                                    text: fragment.text,
                                    matches: fragment
                                        .matches
                                        .into_iter()
                                        .map(|span| searchkernel_ir::HighlightSpan {
                                            start_byte: span.start_byte,
                                            end_byte: span.end_byte,
                                        })
                                        .collect(),
                                })
                                .collect(),
                        })
                        .collect(),
                    source: None,
                    types: Vec::new(),
                    match_features: hit
                        .match_features
                        .into_iter()
                        .map(|(name, value)| {
                            let value = match value {
                                MatchFeatureValue::Double(value) => {
                                    searchkernel_ir::Value::Float64(value)
                                }
                                MatchFeatureValue::Data(value) => {
                                    searchkernel_ir::Value::Bytes(value)
                                }
                            };
                            (name, value)
                        })
                        .collect(),
                    summary_features: std::collections::BTreeMap::new(),
                    rank_features: std::collections::BTreeMap::new(),
                    sort_values: Vec::new(),
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut root_fields = std::collections::BTreeMap::new();
        let mut native_facets = result.facets.into_iter();
        let mut native_groupings = result.groupings.into_iter();
        for grouping in groupings {
            match grouping {
                GroupingLowering::Facet(facet) => {
                    let native = native_facets.next().ok_or_else(|| {
                        internal("native search returned too few facet groupings")
                    })?;
                    children.push(facet_result_node(native, facet)?);
                }
                GroupingLowering::Native(native) => {
                    let grouped = native_groupings.next().ok_or_else(|| {
                        internal("native search returned too few structured groupings")
                    })?;
                    let grouping_node = grouping_result_node(grouped, &native.plan)?;
                    if !native.plan.root_aggregations.is_empty() {
                        let searchkernel_ir::ResultNode::Group(group) = grouping_node else {
                            return Err(internal("native grouping root is not a result group"));
                        };
                        for (name, value) in group.fields {
                            insert_unique_grouping_field(&mut root_fields, name, value)?;
                        }
                        children.extend(group.children);
                    } else {
                        children.push(grouping_node);
                    }
                }
            }
        }
        if native_facets.next().is_some() {
            return Err(internal("native search returned too many facet groupings"));
        }
        if native_groupings.next().is_some() {
            return Err(internal(
                "native search returned too many structured groupings",
            ));
        }
        let errors = portable_errors_from_native_issues(&result.issues);
        Ok(SearchResult {
            root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
                fields: root_fields,
                children,
                ..searchkernel_ir::ResultGroup::root(Vec::new())
            }),
            total_hits: searchkernel_ir::TotalHits {
                value: result.total_hits,
                relation: match result.total_hits_relation {
                    VespaTotalHitsRelation::Exact => TotalHitsRelation::Exact,
                    VespaTotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
                    VespaTotalHitsRelation::Unknown => TotalHitsRelation::Unknown,
                },
            },
            coverage: Some(portable_coverage(result.coverage)),
            timing: None,
            errors,
            trace: None,
            metadata: std::collections::BTreeMap::new(),
        })
    }

    fn analyze(&self, request: AnalyzeRequestIr) -> Result<AnalyzeResult> {
        let options = analysis_options(request.language())?;
        let analysis =
            searchkernel_vespa_engine::Linguistics.analyze_document(request.text(), options);
        Ok(AnalyzeResult {
            tokens: analysis
                .terms
                .into_iter()
                .map(|term| AnalyzedToken {
                    surface: term.original,
                    term: term.term,
                    position: term.position,
                    start_byte: term.byte_start as u32,
                    end_byte: (term.byte_start + term.byte_len) as u32,
                })
                .collect(),
        })
    }

    fn close(self: Box<Self>) -> Result<()> {
        self.engine.close().map_err(native_error)
    }
}

#[cfg(test)]
mod implicit_output_name_regression_tests {
    use super::*;
    use crate::search::lower_groupings;
    use searchkernel_ir::{
        GroupingAggregationIr, GroupingAllIr, GroupingExpressionIr, GroupingOperationIr,
        GroupingOutputIr, GroupingRequestIr, RequestValueIr,
    };

    #[test]
    fn implicit_root_output_name_survives_lowering_without_native_debug_drift() {
        let request = GroupingRequestIr {
            root: GroupingAllIr {
                operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: GroupingExpressionIr::Aggregation(Box::new(
                        GroupingAggregationIr::Sum(GroupingExpressionIr::Literal(
                            RequestValueIr::Int64(1),
                        )),
                    )),
                }])],
                label: None,
            },
            continuations: Vec::new(),
        };
        let lowerings = lower_groupings(&[request]).expect("root grouping should lower");
        let GroupingLowering::Native(native) = &lowerings[0] else {
            panic!("root aggregation should use native grouping lowering");
        };

        let node = grouping_result_node(
            GroupingResult {
                key: None,
                outputs: vec![GroupingOutput::Value(GroupingResultValue::Integer(1))],
                groups: Vec::new(),
            },
            &native.plan,
        )
        .expect("grouping result should project");
        let searchkernel_ir::ResultNode::Group(group) = node else {
            panic!("grouping result should be a group");
        };

        assert!(
            group.fields.contains_key("sum(Literal(Int64(1)))"),
            "portable implicit output names must remain canonical across lowering"
        );
        assert!(
            !group.fields.contains_key("sum(Literal(Integer(1)))"),
            "native enum debug spelling must not leak into portable result keys"
        );
    }

    #[test]
    fn explicit_native_spelling_and_implicit_canonical_name_both_survive_root_assembly() {
        let requests = vec![
            GroupingRequestIr {
                root: GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: Some("sum(Literal(Integer(1)))".to_owned()),
                        expression: GroupingExpressionIr::Aggregation(Box::new(
                            GroupingAggregationIr::Count,
                        )),
                    }])],
                    label: None,
                },
                continuations: Vec::new(),
            },
            GroupingRequestIr {
                root: GroupingAllIr {
                    operations: vec![GroupingOperationIr::Output(vec![GroupingOutputIr {
                        label: None,
                        expression: GroupingExpressionIr::Aggregation(Box::new(
                            GroupingAggregationIr::Sum(GroupingExpressionIr::Literal(
                                RequestValueIr::Int64(1),
                            )),
                        )),
                    }])],
                    label: None,
                },
                continuations: Vec::new(),
            },
        ];
        let lowerings = lower_groupings(&requests).expect("both root groupings should lower");
        let mut root_fields = std::collections::BTreeMap::new();

        for (index, lowering) in lowerings.iter().enumerate() {
            let GroupingLowering::Native(native) = lowering else {
                panic!("root aggregation should use native grouping lowering");
            };
            let node = grouping_result_node(
                GroupingResult {
                    key: None,
                    outputs: vec![GroupingOutput::Value(GroupingResultValue::Integer(
                        i64::try_from(index + 1).expect("small test index"),
                    ))],
                    groups: Vec::new(),
                },
                &native.plan,
            )
            .expect("grouping result should project");
            let searchkernel_ir::ResultNode::Group(group) = node else {
                panic!("grouping result should be a group");
            };
            for (name, value) in group.fields {
                insert_unique_grouping_field(&mut root_fields, name, value)
                    .expect("canonical output names must remain distinct");
            }
        }

        assert_eq!(root_fields.len(), 2);
        assert_eq!(
            root_fields.get("sum(Literal(Integer(1)))"),
            Some(&searchkernel_ir::Value::Int64(1))
        );
        assert_eq!(
            root_fields.get("sum(Literal(Int64(1)))"),
            Some(&searchkernel_ir::Value::Int64(2))
        );
    }

    #[test]
    fn missing_canonical_grouping_output_name_fails_closed() {
        let plan = GroupingPlan {
            levels: Vec::new(),
            group_all: false,
            root_label: None,
            root_aggregations: vec![GroupingAggregation::Count],
            root_output_labels: vec![None],
        };
        let error = grouping_result_node(
            GroupingResult {
                key: None,
                outputs: vec![GroupingOutput::Value(GroupingResultValue::Integer(1))],
                groups: Vec::new(),
            },
            &plan,
        )
        .expect_err("visible lowered outputs must carry a canonical result-field name");

        assert_eq!(error.code, searchkernel_ir::ErrorCode::Internal);
        assert!(error.message.contains("no canonical result-field name"));
    }

    #[test]
    fn duplicate_grouping_result_fields_fail_closed_instead_of_overwriting() {
        let plan = GroupingPlan {
            levels: Vec::new(),
            group_all: false,
            root_label: None,
            root_aggregations: vec![GroupingAggregation::Count, GroupingAggregation::Count],
            root_output_labels: vec![Some("metric".to_owned()), Some("metric".to_owned())],
        };
        let error = grouping_result_node(
            GroupingResult {
                key: None,
                outputs: vec![
                    GroupingOutput::Value(GroupingResultValue::Integer(1)),
                    GroupingOutput::Value(GroupingResultValue::Integer(2)),
                ],
                groups: Vec::new(),
            },
            &plan,
        )
        .expect_err("duplicate grouping result keys must fail closed");

        assert_eq!(error.code, searchkernel_ir::ErrorCode::Internal);
        assert!(error.message.contains("duplicate grouping result field"));
    }
}
