//! Vespa-backed implementation of the portable backend contract.

use crate::search::{
    lower_facet_groupings, lower_facets_into_request, lower_search_options, lower_search_query,
    validate_search_request,
};
use crate::{
    analysis_options, internal, lower_document, lower_field_value, native_error,
    native_search_error, project_document, value_to_ir,
};
use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, AnalyzedToken, Backend, DeleteRequestIr, DeleteResult,
    FieldValueIr, GetRequestIr, GetResult, InsertRequestIr, InsertResult, PatchOperationIr,
    PatchRequestIr, PortableError, Result, ScalarValueIr, SearchRequestIr, SearchResult,
    TotalHitsRelation, UpsertRequestIr, UpsertResult,
};
use searchkernel_vespa_engine::{
    DocumentUpdate, Engine, FieldValue, ScalarValue, TotalHitsRelation as VespaTotalHitsRelation,
    Value as VespaValue,
};
use std::sync::Mutex;

/// A real Vespa-backed implementation of the portable backend contract.
pub struct VespaBackend {
    pub(crate) engine: Engine,
    insert_lock: Mutex<()>,
    #[cfg(test)]
    mutation_probe: Option<std::sync::Arc<MutationProbe>>,
}

#[cfg(test)]
pub(crate) struct MutationProbe {
    pub(crate) rendezvous: std::sync::Barrier,
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
            insert_lock: Mutex::new(()),
            #[cfg(test)]
            mutation_probe: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_mutation_probe(
        engine: Engine,
        probe: std::sync::Arc<MutationProbe>,
    ) -> Self {
        Self {
            engine,
            insert_lock: Mutex::new(()),
            mutation_probe: Some(probe),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Backend for VespaBackend {
    fn insert(&self, request: InsertRequestIr) -> Result<InsertResult> {
        let document = lower_document(request.document())?;
        let id = document.id().to_owned();
        let _insert_guard = self
            .insert_lock
            .lock()
            .map_err(|_| internal("insert lock is poisoned"))?;
        if self.engine.get(&id).map_err(native_error)?.is_some() {
            return Err(PortableError::new(
                searchkernel_ir::ErrorCode::AlreadyExists,
                format!("document already exists: {id}"),
            ));
        }
        #[cfg(test)]
        if let Some(probe) = &self.mutation_probe {
            probe.rendezvous.wait();
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
        let _mutation_guard = self
            .insert_lock
            .lock()
            .map_err(|_| internal("insert/upsert lock is poisoned"))?;
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
                }
                PatchOperationIr::Unset { field } => update = update.clear(field.as_str()),
            }
        }
        self.engine.update(update).map_err(native_error)?;
        Ok(searchkernel_ir::PatchResult {
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
        let fields = project_document(&document, request.projection().fields());
        Ok(GetResult {
            document: Some(searchkernel_ir::DocumentResult {
                id: document.id().to_owned(),
                fields,
            }),
        })
    }

    fn search(&self, request: SearchRequestIr) -> Result<SearchResult> {
        validate_search_request(&request)?;
        let language = analysis_options(request.language())?;
        let query = lower_search_query(&request, language)?;
        let m_request = lower_search_options(&request, query.clone(), language)?;
        let facets = lower_facet_groupings(request.grouping())?;
        let m_request = lower_facets_into_request(m_request, &facets);
        let result = self
            .engine
            .search(m_request)
            .map_err(|error| native_search_error(error, "sort_field_type_not_supported"))?;
        if result.facets.len() != facets.len() {
            return Err(internal(
                "native search returned an unexpected grouping count",
            ));
        }
        let mut children = result
            .hits
            .into_iter()
            .map(|hit| {
                searchkernel_ir::ResultNode::Hit(searchkernel_ir::Hit {
                    id: hit.document_id,
                    score: Some(hit.score),
                    fields: hit
                        .fields
                        .into_iter()
                        .map(|(name, value)| (name, value_to_ir(&value)))
                        .collect(),
                    highlights: Vec::new(),
                    source: None,
                    types: Vec::new(),
                    match_features: hit
                        .match_features
                        .into_iter()
                        .map(|(name, value)| (name, searchkernel_ir::Value::Float64(value)))
                        .collect(),
                    summary_features: std::collections::BTreeMap::new(),
                    rank_features: std::collections::BTreeMap::new(),
                    sort_values: Vec::new(),
                })
            })
            .collect::<Vec<_>>();
        let facet_nodes = result
            .facets
            .into_iter()
            .zip(facets)
            .map(|(grouping, facet)| -> Result<searchkernel_ir::ResultNode> {
                let groups = grouping
                    .groups
                    .into_iter()
                    .map(|group| {
                        let mut fields = std::collections::BTreeMap::new();
                        fields.insert(
                            "count".to_owned(),
                            searchkernel_ir::Value::UInt64(group.count),
                        );
                        for (index, aggregation) in group.aggregations.into_iter().enumerate() {
                            let value = match aggregation {
                                searchkernel_vespa_engine::AggregateValue::Integer(value) => {
                                    searchkernel_ir::Value::Int64(value)
                                }
                                searchkernel_vespa_engine::AggregateValue::Float(value) => {
                                    searchkernel_ir::Value::Float64(value)
                                }
                            };
                            fields.insert(format!("aggregation_{index}"), value);
                        }
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
                            value: Some(value_to_ir(&FieldValue::Scalar(value))),
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
                Ok(searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup {
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
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        children.extend(facet_nodes);
        Ok(SearchResult {
            root: searchkernel_ir::ResultNode::Group(searchkernel_ir::ResultGroup::root(children)),
            total_hits: searchkernel_ir::TotalHits {
                value: result.total_hits,
                relation: match result.total_hits_relation {
                    VespaTotalHitsRelation::Exact => TotalHitsRelation::Exact,
                    VespaTotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
                },
            },
            coverage: None,
            timing: None,
            errors: Vec::new(),
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
