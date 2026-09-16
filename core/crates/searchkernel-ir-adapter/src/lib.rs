//! Integration boundary between the portable Search Kernel IR and the
//! low-level Vespa engine wrapper.
//!
//! This crate is the only translation boundary between portable IR and the
//! Vespa-facing engine wrapper. It contains no fallback or in-memory engine.

use searchkernel_ir::{
    AnalyzeRequestIr, AnalyzeResult, AnalyzedToken, Backend, DeleteRequestIr, DeleteResult,
    DocumentIr, FieldValueIr, FilterIr, GetRequestIr, GetResult, InsertRequestIr, InsertResult,
    PatchOperationIr, PatchRequestIr, PortableError, QueryIr, RangeIr, Result, ScalarValueIr,
    SearchRequestIr, SearchResult, TotalHitsRelation, UpsertRequestIr, UpsertResult, Value,
};
use searchkernel_vespa_engine::{
    ConfigCompiler, Document, DocumentUpdate, Engine, FacetRequest, Field, FieldType, FieldValue,
    GroupingResult, Query, ScalarValue, Schema, SearchRequest, SortField, SortOrder,
    SummaryRequest, TotalHitsRelation as VespaTotalHitsRelation, Value as VespaValue,
};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Compile the resolved portable definition into the M-layer configuration.
///
/// This is deliberately owned by the adapter: the product layer must resolve
/// portable defaults into IR before the Vespa schema is generated.  M does not
/// currently expose collection fields through its generic document ABI, so a
/// multi-valued definition is rejected before any on-disk state is created.
pub fn compile_index_definition(
    definition: &searchkernel_ir::IndexDefinitionIr,
    config_dir: impl AsRef<std::path::Path>,
) -> Result<()> {
    validate_index_definition(definition)?;
    let mut schema = Schema::new(
        definition
            .name()
            .map_or("searchkernel", searchkernel_ir::IndexName::as_str),
    );
    for field in definition.fields() {
        if matches!(
            field.cardinality(),
            searchkernel_ir::FieldCardinality::Multi
        ) {
            return Err(PortableError::unsupported(
                "multi_valued_fields_not_supported",
                Some("fields"),
                format!(
                    "multi-valued field `{}` requires collection-valued M document/configuration and grouping support",
                    field.name().as_str()
                ),
            ));
        }
        let kind = match field.kind() {
            searchkernel_ir::FieldKind::Text => FieldType::Text,
            searchkernel_ir::FieldKind::Keyword => FieldType::String,
            searchkernel_ir::FieldKind::Int64 => FieldType::Integer,
            searchkernel_ir::FieldKind::Float64 => FieldType::Float,
            searchkernel_ir::FieldKind::Bool => FieldType::Boolean,
        };
        let capabilities = field.capabilities();
        let mut native = Field::new(field.name().as_str(), kind);
        if capabilities.searchable() {
            native = native.indexed();
        }
        if matches!(field.kind(), searchkernel_ir::FieldKind::Keyword) && capabilities.filterable()
        {
            native = native.fast_search();
        }
        if capabilities.filterable()
            || capabilities.sortable()
            || capabilities.facetable()
            || !matches!(field.kind(), searchkernel_ir::FieldKind::Text)
        {
            native = native.attribute();
        }
        if capabilities.stored() {
            native = native.summary();
        }
        schema = schema.field(native);
    }
    ConfigCompiler
        .compile_to(&schema, config_dir)
        .map_err(native_error)
}

/// Validate the subset of the portable schema that the current M ABI can
/// execute without losing document values.
pub fn validate_index_definition(definition: &searchkernel_ir::IndexDefinitionIr) -> Result<()> {
    for (index, field) in definition.fields().iter().enumerate() {
        if matches!(
            field.cardinality(),
            searchkernel_ir::FieldCardinality::Multi
        ) {
            return Err(PortableError::unsupported(
                "multi_valued_fields_not_supported",
                Some("fields"),
                format!(
                    "multi-valued field `{}` requires collection-valued M document/configuration and grouping support",
                    field.name().as_str()
                ),
            ));
        }

        let field_path = format!("fields[{index}]");
        if matches!(field.kind(), searchkernel_ir::FieldKind::Text)
            && !field.capabilities().searchable()
        {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.searchable")),
                format!(
                    "text field `{}` must be searchable in the current native schema",
                    field.name().as_str()
                ),
            ));
        }
        if matches!(field.kind(), searchkernel_ir::FieldKind::Float64)
            && field.capabilities().facetable()
        {
            return Err(PortableError::unsupported(
                "unsupported_field_capability",
                Some(format!("{field_path}.facetable")),
                format!(
                    "float field `{}` cannot be facetable in the current native schema",
                    field.name().as_str()
                ),
            ));
        }
    }
    Ok(())
}

/// A real Vespa-backed implementation of the portable backend contract.
pub struct VespaBackend {
    engine: Engine,
    insert_lock: Mutex<()>,
    #[cfg(test)]
    mutation_probe: Option<std::sync::Arc<MutationProbe>>,
}

#[cfg(test)]
struct MutationProbe {
    rendezvous: std::sync::Barrier,
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
    fn with_mutation_probe(engine: Engine, probe: std::sync::Arc<MutationProbe>) -> Self {
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
        let mut m_request = lower_search_options(&request, query.clone(), language)?;
        for facet in request.facets() {
            m_request = m_request.facet(FacetRequest::new(
                facet.field().as_str(),
                facet.limit(),
                lower_facet_order(facet),
            ));
        }
        let result = self.engine.search(m_request).map_err(|error| {
            if error.kind() == searchkernel_vespa_engine::ErrorKind::Unsupported
                && error.to_string().contains("grouping key type")
            {
                PortableError::unsupported(
                    "facet_key_type_not_supported",
                    Some("facets"),
                    error.to_string(),
                )
            } else {
                native_search_error(error, "sort_field_type_not_supported")
            }
        })?;
        let facets = lower_facets(&result.facets, request.facets())?;
        Ok(SearchResult {
            hits: result
                .hits
                .into_iter()
                .map(|hit| searchkernel_ir::Hit {
                    id: hit.document_id,
                    score: Some(hit.score),
                    fields: hit
                        .fields
                        .into_iter()
                        .map(|(name, value)| (name, value_to_ir(&value)))
                        .collect(),
                    highlights: Vec::new(),
                })
                .collect(),
            total_hits: searchkernel_ir::TotalHits {
                value: result.total_hits,
                relation: match result.total_hits_relation {
                    VespaTotalHitsRelation::Exact => TotalHitsRelation::Exact,
                    VespaTotalHitsRelation::LowerBound => TotalHitsRelation::LowerBound,
                },
            },
            facets,
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

fn lower_document(document: &DocumentIr) -> Result<Document> {
    let mut result = Document::new(document.id().as_str());
    for (field, value) in document.fields() {
        let lowered = lower_field_value(value)?;
        result = result.field(field.as_str(), lowered.clone());
        if matches!(lowered, FieldValue::Scalar(ScalarValue::String(_))) {
            let analysis = searchkernel_vespa_engine::Linguistics.analyze_document(
                match value {
                    FieldValueIr::Single(ScalarValueIr::String(value)) => value,
                    _ => unreachable!("string annotation only follows a scalar string"),
                },
                searchkernel_vespa_engine::LinguisticsOptions::default(),
            );
            result = result.annotate_text(field.as_str(), analysis);
        }
    }
    Ok(result)
}

fn project_document(
    document: &Document,
    projection: &[searchkernel_ir::FieldName],
) -> BTreeMap<String, Value> {
    document
        .fields()
        .iter()
        .filter(|(name, _)| {
            projection
                .iter()
                .any(|field| field.as_str() == name.as_str())
        })
        .map(|(name, value)| (name.clone(), value_to_ir(value)))
        .collect()
}

fn lower_field_value(value: &FieldValueIr) -> Result<FieldValue> {
    match value {
        FieldValueIr::Single(value) => Ok(FieldValue::Scalar(lower_scalar(value)?)),
        FieldValueIr::Multi(values) => {
            let mut values = values.iter();
            let Some(first) = values.next() else {
                return Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Strings(Vec::new()),
                ));
            };
            let first = lower_scalar(first)?;
            match first {
                ScalarValue::String(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Strings(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::String(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Integer(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Integers(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Integer(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Float(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Floats(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Float(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
                ScalarValue::Boolean(first) => Ok(FieldValue::Array(
                    searchkernel_vespa_engine::ArrayValue::Booleans(
                        std::iter::once(first)
                            .chain(
                                values
                                    .map(|value| match lower_scalar(value) {
                                        Ok(ScalarValue::Boolean(value)) => Ok(value),
                                        Ok(_) => Err(unsupported("heterogeneous array")),
                                        Err(error) => Err(error),
                                    })
                                    .collect::<Result<Vec<_>>>()?,
                            )
                            .collect(),
                    ),
                )),
            }
        }
    }
}

fn lower_scalar(value: &ScalarValueIr) -> Result<ScalarValue> {
    Ok(match value {
        ScalarValueIr::Bool(value) => ScalarValue::Boolean(*value),
        ScalarValueIr::Int64(value) => ScalarValue::Integer(*value),
        ScalarValueIr::Float64(value) => ScalarValue::Float(*value),
        ScalarValueIr::String(value) => ScalarValue::String(value.clone()),
    })
}

fn lower_search_query(
    request: &SearchRequestIr,
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    let mut clauses = vec![lower_query_with_language(
        request.query(),
        request.search_fields(),
        language,
    )?];
    if let Some(filter) = request.filter() {
        clauses.push(lower_filter(filter)?);
    }
    if clauses.len() == 1 {
        Ok(clauses.pop().expect("one clause was inserted"))
    } else {
        Ok(Query::And(clauses))
    }
}

fn lower_search_options(
    request: &SearchRequestIr,
    query: Query,
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<SearchRequest> {
    let mut m_request = SearchRequest::new(query)
        .limit(request.limit())
        .offset(request.offset())
        .rank_profile(ranking_profile(request.ranking())?)
        .language(language);
    if let Some(timeout) = request.timeout() {
        m_request = m_request.timeout(timeout);
    }
    for sort in request.sort() {
        m_request = m_request.sort(lower_sort(sort));
    }

    // C2 has already resolved the projection. An empty list means that no
    // summary fields should be fetched; the native API rejects an explicit
    // empty field list, so use its no-summary mode instead.
    let fields = request.projection().fields();
    if fields.is_empty() {
        m_request = m_request.summary(SummaryRequest::None);
    } else {
        m_request = m_request.fields(fields.iter().map(|field| field.as_str()));
    }
    Ok(m_request)
}

fn lower_sort(sort: &searchkernel_ir::SortIr) -> SortField {
    let sort_field = match sort.direction() {
        searchkernel_ir::SortDirection::Asc => {
            SortField::new(sort.field().as_str(), SortOrder::Ascending)
        }
        searchkernel_ir::SortDirection::Desc => {
            SortField::new(sort.field().as_str(), SortOrder::Descending)
        }
    };
    match sort.missing() {
        searchkernel_ir::MissingPlacement::First => sort_field.missing_first(),
        searchkernel_ir::MissingPlacement::Last => sort_field.missing_last(),
    }
}

fn lower_facet_order(facet: &searchkernel_ir::FacetIr) -> searchkernel_vespa_engine::GroupingOrder {
    match facet.sort() {
        searchkernel_ir::FacetSort::CountAsc => {
            searchkernel_vespa_engine::GroupingOrder::CountAscending
        }
        searchkernel_ir::FacetSort::CountDesc => {
            searchkernel_vespa_engine::GroupingOrder::CountDescending
        }
        searchkernel_ir::FacetSort::ValueAsc => {
            searchkernel_vespa_engine::GroupingOrder::ValueAscending
        }
        searchkernel_ir::FacetSort::ValueDesc => {
            searchkernel_vespa_engine::GroupingOrder::ValueDescending
        }
    }
}

#[cfg(test)]
fn lower_facet_grouping(
    query: Query,
    facet: &searchkernel_ir::FacetIr,
) -> searchkernel_vespa_engine::GroupingRequest {
    searchkernel_vespa_engine::GroupingRequest::new(query, facet.field().as_str())
        .max_groups(facet.limit())
        .order(lower_facet_order(facet))
}

fn validate_search_request(request: &SearchRequestIr) -> Result<()> {
    if !matches!(
        request.highlighting(),
        searchkernel_ir::HighlightingIr::Disabled
    ) {
        return Err(PortableError::unsupported(
            "highlighting_native_result_unavailable",
            Some("highlighting"),
            "native Vespa result path does not expose generated highlight fragments and spans",
        ));
    }
    if request
        .search_fields()
        .iter()
        .any(|field| field.weight().get() != 1.0)
    {
        return Err(PortableError::unsupported(
            "ranking_weights_not_supported",
            Some("search_fields"),
            "non-default search-field weights require the M10/I7 ranking integration surface",
        ));
    }
    analysis_options(request.language())?;
    Ok(())
}

fn ranking_profile(ranking: searchkernel_ir::RankingPolicyIr) -> Result<&'static str> {
    match ranking {
        searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1 => Ok("default"),
        searchkernel_ir::RankingPolicyIr::Bm25 => Err(PortableError::unsupported(
            "bm25_ranking_not_supported",
            Some("ranking"),
            "BM25 ranking requires the M10/I7 ranking integration surface",
        )),
    }
}

#[cfg(test)]
fn lower_query(query: &QueryIr, fields: &[searchkernel_ir::SearchFieldIr]) -> Result<Query> {
    lower_query_with_language(
        query,
        fields,
        searchkernel_vespa_engine::LinguisticsOptions::default(),
    )
}

fn lower_query_with_language(
    query: &QueryIr,
    fields: &[searchkernel_ir::SearchFieldIr],
    language: searchkernel_vespa_engine::LinguisticsOptions,
) -> Result<Query> {
    match query {
        QueryIr::MatchAll => Ok(Query::True),
        QueryIr::Text(text) => {
            let term_count = searchkernel_vespa_engine::Linguistics
                .analyze_query(text, language)
                .terms
                .len();
            let clauses = fields
                .iter()
                .map(|field| {
                    if term_count > 1 {
                        Query::phrase(field.field().as_str(), text)
                    } else {
                        Query::term(field.field().as_str(), text)
                    }
                })
                .collect::<Vec<_>>();
            match clauses.as_slice() {
                [] => Err(unsupported("text query has no searchable fields")),
                [query] => Ok(query.clone()),
                _ => Ok(Query::Or(clauses)),
            }
        }
        QueryIr::Term { field, value } => match value {
            ScalarValueIr::String(value) => Ok(Query::term(field.as_str(), value)),
            ScalarValueIr::Int64(value) => {
                Ok(Query::equals(field.as_str(), integer_query_value(*value)?))
            }
            _ => Err(unsupported("unsupported typed query value")),
        },
        QueryIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(|child| lower_query_with_language(child, fields, language))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(|child| lower_query_with_language(child, fields, language))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Not(child) => Ok(Query::Not(Box::new(lower_query_with_language(
            child, fields, language,
        )?))),
    }
}

fn lower_filter(filter: &FilterIr) -> Result<Query> {
    match filter {
        FilterIr::And(children) => Ok(Query::And(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(lower_filter)
                .collect::<Result<Vec<_>>>()?,
        )),
        FilterIr::Not(child) => Ok(Query::Not(Box::new(lower_filter(child)?))),
        FilterIr::Exists { field } => Ok(Query::AttributeRange {
            field: field.to_string(),
            lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
            upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
        }),
        FilterIr::Comparison(comparison) => lower_comparison(comparison),
        FilterIr::Range(range) => lower_range(range),
    }
}

fn lower_comparison(comparison: &searchkernel_ir::ComparisonIr) -> Result<Query> {
    use searchkernel_ir::ComparisonOp;
    let field = comparison.field().to_string();
    let value = lower_attribute_value(comparison.value())?;
    match comparison.op() {
        ComparisonOp::Eq => Ok(Query::AttributeEquals { field, value }),
        ComparisonOp::Ne => Ok(Query::Not(Box::new(Query::AttributeEquals {
            field,
            value,
        }))),
        ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte => {
            let lower_bound = matches!(comparison.op(), ComparisonOp::Gt | ComparisonOp::Gte);
            let included = matches!(comparison.op(), ComparisonOp::Gte | ComparisonOp::Lte);
            if !matches!(value, VespaValue::Integer(_) | VespaValue::Float(_)) {
                return Err(unsupported("ordered comparisons require a numeric field"));
            }
            let bound = if included {
                searchkernel_vespa_engine::AttributeBound::Included(value)
            } else {
                searchkernel_vespa_engine::AttributeBound::Excluded(value)
            };
            Ok(Query::AttributeRange {
                field,
                lower: if lower_bound {
                    bound.clone()
                } else {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                },
                upper: if lower_bound {
                    searchkernel_vespa_engine::AttributeBound::Unbounded
                } else {
                    bound
                },
            })
        }
    }
}

fn lower_range(range: &RangeIr) -> Result<Query> {
    Ok(Query::AttributeRange {
        field: range.field().to_string(),
        lower: attribute_bound(range.lower(), range.include_lower())?,
        upper: attribute_bound(range.upper(), range.include_upper())?,
    })
}

fn attribute_bound(
    value: Option<&ScalarValueIr>,
    included: bool,
) -> Result<searchkernel_vespa_engine::AttributeBound> {
    let Some(value) = value else {
        return Ok(searchkernel_vespa_engine::AttributeBound::Unbounded);
    };
    let value = lower_attribute_value(value)?;
    Ok(if included {
        searchkernel_vespa_engine::AttributeBound::Included(value)
    } else {
        searchkernel_vespa_engine::AttributeBound::Excluded(value)
    })
}

fn lower_attribute_value(value: &ScalarValueIr) -> Result<VespaValue> {
    match value {
        ScalarValueIr::Int64(value) => Ok(VespaValue::Integer(integer_query_value(*value)?)),
        ScalarValueIr::Float64(value) if value.is_finite() => Ok(VespaValue::Float(*value)),
        ScalarValueIr::String(value) => Ok(VespaValue::String(value.clone())),
        ScalarValueIr::Bool(value) => Ok(VespaValue::Boolean(*value)),
        ScalarValueIr::Float64(_) => Err(unsupported("float comparison value must be finite")),
    }
}

fn integer_query_value(value: i64) -> Result<i64> {
    if value == i64::MIN {
        return Err(unsupported(
            "INT64_MIN is reserved as Vespa's undefined integer attribute sentinel",
        ));
    }
    Ok(value)
}

fn value_to_ir(value: &FieldValue) -> Value {
    match value {
        FieldValue::Scalar(ScalarValue::String(value)) => Value::String(value.clone()),
        FieldValue::Scalar(ScalarValue::Integer(value)) => Value::Int64(*value),
        FieldValue::Scalar(ScalarValue::Float(value)) => Value::Float64(*value),
        FieldValue::Scalar(ScalarValue::Boolean(value)) => Value::Bool(*value),
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Strings(values)) => {
            Value::List(values.iter().cloned().map(Value::String).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Integers(values)) => {
            Value::List(values.iter().copied().map(Value::Int64).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Floats(values)) => {
            Value::List(values.iter().copied().map(Value::Float64).collect())
        }
        FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Booleans(values)) => {
            Value::List(values.iter().copied().map(Value::Bool).collect())
        }
    }
}

fn native_error(error: searchkernel_vespa_engine::Error) -> PortableError {
    let message = error.to_string();
    match error.kind() {
        searchkernel_vespa_engine::ErrorKind::InvalidArgument => {
            PortableError::invalid("backend_request", None::<String>, message)
        }
        searchkernel_vespa_engine::ErrorKind::ResourceExhausted => {
            PortableError::new(searchkernel_ir::ErrorCode::ResourceExhausted, message)
        }
        searchkernel_vespa_engine::ErrorKind::Unsupported => {
            PortableError::unsupported("native_capability_not_supported", None::<String>, message)
        }
        searchkernel_vespa_engine::ErrorKind::Internal => {
            PortableError::new(searchkernel_ir::ErrorCode::Internal, message)
        }
        searchkernel_vespa_engine::ErrorKind::DeadlineExceeded => {
            PortableError::new(searchkernel_ir::ErrorCode::DeadlineExceeded, message)
        }
    }
}

fn native_search_error(
    error: searchkernel_vespa_engine::Error,
    unsupported_code: &'static str,
) -> PortableError {
    if error.kind() == searchkernel_vespa_engine::ErrorKind::Unsupported {
        return PortableError::unsupported(unsupported_code, Some("sort"), error.to_string());
    }
    native_error(error)
}

fn lower_facets(
    grouped: &[GroupingResult],
    facets: &[searchkernel_ir::FacetIr],
) -> Result<Vec<searchkernel_ir::FacetResult>> {
    if grouped.len() != facets.len() {
        return Err(internal("native search returned an unexpected facet count"));
    }
    facets
        .iter()
        .zip(grouped)
        .map(|(facet, grouped)| {
            let values = grouped
                .groups
                .iter()
                .map(|group| Ok(searchkernel_ir::FacetValue {
                    value: value_to_ir(&FieldValue::Scalar(match &group.key {
                        VespaValue::String(value) => ScalarValue::String(value.clone()),
                        VespaValue::Integer(value) => ScalarValue::Integer(*value),
                        VespaValue::Boolean(value) => ScalarValue::Boolean(*value),
                        _ => return Err(PortableError::unsupported(
                            "facet_key_type_not_supported",
                            Some("facets"),
                            "native grouping returned a key kind not representable by the portable facet contract",
                        )),
                    })),
                    count: group.count,
                }))
                .collect::<Result<Vec<_>>>()?;
            Ok(searchkernel_ir::FacetResult {
                field: facet.field().to_string(),
                values,
            })
        })
        .collect()
}

fn analysis_options(
    language: Option<&searchkernel_ir::LanguageHint>,
) -> Result<searchkernel_vespa_engine::LinguisticsOptions> {
    match language.map(|value| value.as_str()) {
        None | Some("en") => Ok(searchkernel_vespa_engine::LinguisticsOptions::default()),
        Some(value) if value.starts_with("en-") => {
            Ok(searchkernel_vespa_engine::LinguisticsOptions::default())
        }
        Some(value) => Err(PortableError::unsupported(
            "language_not_supported",
            Some("language"),
            format!("language `{value}` is not supported by the current analyzer"),
        )),
    }
}

fn internal(message: impl Into<String>) -> PortableError {
    PortableError::new(searchkernel_ir::ErrorCode::Internal, message)
}

fn unsupported(message: impl Into<String>) -> PortableError {
    PortableError::unsupported("m_lowering", None::<String>, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_ir::{
        AnalyzerIdentity, ComparisonIr, ComparisonOp, FacetIr, FieldCapabilities, FieldCardinality,
        FieldIr, FieldKind, FieldName, FieldValueIr, HighlightingIr, IndexDefinitionIr,
        LanguageHint, MissingPlacement, PrefixPolicy, QueryIr, RangeIr, RankingPolicyIr,
        ScalarValueIr, SearchFieldIr, SearchFieldWeight, SortDirection, SortIr, TypoPolicy,
    };

    fn field(name: &str) -> FieldName {
        FieldName::parse(name).unwrap()
    }

    fn request(
        search_fields: Vec<SearchFieldIr>,
        sort: Vec<SortIr>,
        language: Option<LanguageHint>,
    ) -> SearchRequestIr {
        SearchRequestIr::new(
            QueryIr::Text("wireless headphones".into()),
            search_fields,
            None,
            sort,
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            20,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            language,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
    }

    #[test]
    fn analyzed_multi_term_text_lowers_to_a_phrase() {
        let search_field = || {
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )]
        };
        for text in ["wireless headphones", "foo-bar", "can't"] {
            let query = lower_query(&QueryIr::Text(text.into()), &search_field()).unwrap();
            assert_eq!(query, Query::phrase("title", text));
        }

        let query = lower_query(&QueryIr::Text("headphones".into()), &search_field()).unwrap();
        assert_eq!(query, Query::term("title", "headphones"));
    }

    #[test]
    fn unsupported_search_semantics_are_rejected_instead_of_dropped() {
        let weighted = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(2.0).unwrap(),
            )],
            Vec::new(),
            None,
        );
        let error = validate_search_request(&weighted).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);

        let missing_first = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            vec![SortIr::new(
                field("price"),
                SortDirection::Asc,
                MissingPlacement::First,
            )],
            None,
        );
        assert!(validate_search_request(&missing_first).is_ok());

        let language = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            Vec::new(),
            Some(LanguageHint::parse("en").unwrap()),
        );
        assert!(validate_search_request(&language).is_ok());

        let french = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            Vec::new(),
            Some(LanguageHint::parse("fr").unwrap()),
        );
        let error = validate_search_request(&french).unwrap_err();
        assert_eq!(error.details[0].code, "language_not_supported");
    }

    #[test]
    fn multi_valued_schema_is_rejected_before_m_configuration() {
        let definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                field("tags"),
                FieldKind::Keyword,
                FieldCardinality::Multi,
                None::<AnalyzerIdentity>,
                FieldCapabilities::new(false, false, true, false, false),
            )],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        let error = validate_index_definition(&definition).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    }

    #[test]
    fn unsupported_schema_capabilities_are_rejected_before_m_configuration() {
        for (kind, capabilities, expected_path) in [
            (
                FieldKind::Text,
                FieldCapabilities::new(true, false, false, false, false),
                "fields[0].searchable",
            ),
            (
                FieldKind::Float64,
                FieldCapabilities::new(true, false, true, true, true),
                "fields[0].facetable",
            ),
        ] {
            let definition = IndexDefinitionIr::new(
                None,
                vec![FieldIr::new(
                    field("value"),
                    kind,
                    FieldCardinality::Single,
                    None::<AnalyzerIdentity>,
                    capabilities,
                )],
                Vec::new(),
                RankingPolicyIr::DefaultRelevanceV1,
                PrefixPolicy::Disabled,
                TypoPolicy::Disabled,
            )
            .unwrap();
            let error = validate_index_definition(&definition).unwrap_err();
            assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
            assert_eq!(error.details[0].code, "unsupported_field_capability");
            assert_eq!(error.details[0].path.as_deref(), Some(expected_path));
        }

        let definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                field("active"),
                FieldKind::Bool,
                FieldCardinality::Single,
                None::<AnalyzerIdentity>,
                FieldCapabilities::new(true, false, true, true, true),
            )],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        validate_index_definition(&definition)
            .expect("single-valued boolean capabilities are supported");
    }

    #[test]
    fn field_value_lowering_covers_scalars_arrays_and_type_errors() {
        let scalars = [
            (
                ScalarValueIr::Bool(true),
                FieldValue::Scalar(ScalarValue::Boolean(true)),
            ),
            (
                ScalarValueIr::Int64(-7),
                FieldValue::Scalar(ScalarValue::Integer(-7)),
            ),
            (
                ScalarValueIr::Float64(2.5),
                FieldValue::Scalar(ScalarValue::Float(2.5)),
            ),
            (
                ScalarValueIr::String("title".into()),
                FieldValue::Scalar(ScalarValue::String("title".into())),
            ),
        ];
        for (input, expected) in scalars {
            assert_eq!(
                lower_field_value(&FieldValueIr::Single(input)),
                Ok(expected)
            );
        }

        let arrays = [
            (
                vec![
                    ScalarValueIr::String("a".into()),
                    ScalarValueIr::String("b".into()),
                ],
                FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Strings(vec![
                    "a".into(),
                    "b".into(),
                ])),
            ),
            (
                vec![ScalarValueIr::Int64(1), ScalarValueIr::Int64(2)],
                FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Integers(vec![1, 2])),
            ),
            (
                vec![ScalarValueIr::Float64(1.0), ScalarValueIr::Float64(2.0)],
                FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Floats(vec![
                    1.0, 2.0,
                ])),
            ),
            (
                vec![ScalarValueIr::Bool(true), ScalarValueIr::Bool(false)],
                FieldValue::Array(searchkernel_vespa_engine::ArrayValue::Booleans(vec![
                    true, false,
                ])),
            ),
        ];
        for (input, expected) in arrays {
            assert_eq!(lower_field_value(&FieldValueIr::Multi(input)), Ok(expected));
        }
        assert_eq!(
            lower_field_value(&FieldValueIr::Multi(Vec::new())),
            Ok(FieldValue::Array(
                searchkernel_vespa_engine::ArrayValue::Strings(Vec::new())
            ))
        );
        let error = lower_field_value(&FieldValueIr::Multi(vec![
            ScalarValueIr::String("a".into()),
            ScalarValueIr::Int64(1),
        ]))
        .unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
    }

    #[test]
    fn query_and_filter_lowering_covers_supported_variants() {
        let title = field("title");
        let price = field("price");
        assert_eq!(lower_query(&QueryIr::MatchAll, &[]).unwrap(), Query::True);
        assert_eq!(
            lower_query(
                &QueryIr::Text("fox".into()),
                &[SearchFieldIr::new(
                    title.clone(),
                    SearchFieldWeight::new(1.0).unwrap(),
                )]
            )
            .unwrap(),
            Query::term("title", "fox")
        );
        assert_eq!(
            lower_query(
                &QueryIr::Text("fox hound".into()),
                &[
                    SearchFieldIr::new(title.clone(), SearchFieldWeight::new(1.0).unwrap()),
                    SearchFieldIr::new(field("body"), SearchFieldWeight::new(1.0).unwrap()),
                ],
            )
            .unwrap(),
            Query::Or(vec![
                Query::phrase("title", "fox hound"),
                Query::phrase("body", "fox hound"),
            ])
        );
        assert!(lower_query(&QueryIr::Text("fox".into()), &[]).is_err());
        assert_eq!(
            lower_query(
                &QueryIr::Term {
                    field: price.clone(),
                    value: ScalarValueIr::Int64(10),
                },
                &[],
            )
            .unwrap(),
            Query::equals("price", 10)
        );
        assert_eq!(
            lower_query(
                &QueryIr::Term {
                    field: title.clone(),
                    value: ScalarValueIr::String("fox".into()),
                },
                &[],
            )
            .unwrap(),
            Query::term("title", "fox")
        );
        assert!(lower_query(
            &QueryIr::Term {
                field: price.clone(),
                value: ScalarValueIr::Float64(1.0),
            },
            &[],
        )
        .is_err());
        assert_eq!(
            lower_query(
                &QueryIr::And(vec![
                    QueryIr::MatchAll,
                    QueryIr::Not(Box::new(QueryIr::MatchAll,))
                ]),
                &[],
            )
            .unwrap(),
            Query::And(vec![Query::True, Query::Not(Box::new(Query::True))])
        );

        let eq_int = FilterIr::Comparison(ComparisonIr::new(
            price.clone(),
            ComparisonOp::Eq,
            ScalarValueIr::Int64(10),
        ));
        let ne_string = FilterIr::Comparison(ComparisonIr::new(
            title.clone(),
            ComparisonOp::Ne,
            ScalarValueIr::String("fox".into()),
        ));
        assert_eq!(
            lower_filter(&FilterIr::Exists {
                field: title.clone()
            })
            .unwrap(),
            Query::AttributeRange {
                field: "title".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
            }
        );
        assert_eq!(
            lower_filter(&eq_int).unwrap(),
            Query::AttributeEquals {
                field: "price".into(),
                value: VespaValue::Integer(10),
            }
        );
        assert_eq!(
            lower_filter(&ne_string).unwrap(),
            Query::Not(Box::new(Query::AttributeEquals {
                field: "title".into(),
                value: VespaValue::String("fox".into()),
            }))
        );
        assert_eq!(
            lower_filter(&FilterIr::And(vec![
                eq_int.clone(),
                FilterIr::Not(Box::new(ne_string.clone()))
            ]))
            .unwrap(),
            Query::And(vec![
                Query::AttributeEquals {
                    field: "price".into(),
                    value: VespaValue::Integer(10),
                },
                Query::Not(Box::new(Query::Not(Box::new(Query::AttributeEquals {
                    field: "title".into(),
                    value: VespaValue::String("fox".into()),
                })))),
            ])
        );
        assert_eq!(
            lower_filter(&FilterIr::Range(RangeIr::new(
                price.clone(),
                Some(ScalarValueIr::Int64(1)),
                Some(ScalarValueIr::Int64(5)),
                true,
                false,
            )))
            .unwrap(),
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Included(VespaValue::Integer(1),),
                upper: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Integer(5),),
            }
        );
        assert_eq!(
            lower_filter(&FilterIr::Comparison(ComparisonIr::new(
                price.clone(),
                ComparisonOp::Gt,
                ScalarValueIr::Float64(1.0),
            )))
            .unwrap(),
            Query::AttributeRange {
                field: "price".into(),
                lower: searchkernel_vespa_engine::AttributeBound::Excluded(VespaValue::Float(1.0),),
                upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
            }
        );
        assert_eq!(
            lower_filter(&FilterIr::Comparison(ComparisonIr::new(
                title.clone(),
                ComparisonOp::Eq,
                ScalarValueIr::Bool(true),
            )))
            .unwrap(),
            Query::AttributeEquals {
                field: "title".into(),
                value: VespaValue::Boolean(true),
            }
        );
        assert!(lower_filter(&FilterIr::Comparison(ComparisonIr::new(
            price,
            ComparisonOp::Gt,
            ScalarValueIr::Float64(f64::NAN),
        )))
        .is_err());
    }

    #[test]
    fn value_conversion_preserves_all_native_value_shapes() {
        use searchkernel_vespa_engine::ArrayValue;

        let values = [
            (
                FieldValue::Scalar(ScalarValue::String("a".into())),
                Value::String("a".into()),
            ),
            (FieldValue::Scalar(ScalarValue::Integer(1)), Value::Int64(1)),
            (
                FieldValue::Scalar(ScalarValue::Float(1.5)),
                Value::Float64(1.5),
            ),
            (
                FieldValue::Scalar(ScalarValue::Boolean(true)),
                Value::Bool(true),
            ),
            (
                FieldValue::Array(ArrayValue::Strings(vec!["a".into()])),
                Value::List(vec![Value::String("a".into())]),
            ),
            (
                FieldValue::Array(ArrayValue::Integers(vec![1])),
                Value::List(vec![Value::Int64(1)]),
            ),
            (
                FieldValue::Array(ArrayValue::Floats(vec![1.5])),
                Value::List(vec![Value::Float64(1.5)]),
            ),
            (
                FieldValue::Array(ArrayValue::Booleans(vec![true])),
                Value::List(vec![Value::Bool(true)]),
            ),
        ];
        for (native, expected) in values {
            assert_eq!(value_to_ir(&native), expected);
        }
    }

    #[test]
    fn analyze_language_is_explicitly_supported_or_rejected() {
        let english = LanguageHint::parse("en-US").unwrap();
        assert!(analysis_options(Some(&english)).is_ok());

        let french = LanguageHint::parse("fr").unwrap();
        let error = analysis_options(Some(&french)).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(error.details[0].code, "language_not_supported");
    }

    #[test]
    fn search_validation_supports_facets_and_rejects_highlighting_precisely() {
        let title = field("title");
        let search_fields = vec![SearchFieldIr::new(
            title.clone(),
            SearchFieldWeight::new(1.0).unwrap(),
        )];
        let facets = SearchRequestIr::new(
            QueryIr::Text("fox".into()),
            search_fields.clone(),
            None,
            Vec::new(),
            vec![FacetIr::new(
                title.clone(),
                10,
                searchkernel_ir::FacetSort::CountDesc,
            )],
            RankingPolicyIr::DefaultRelevanceV1,
            10,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        );
        assert!(validate_search_request(&facets).is_ok());

        let highlighting = SearchRequestIr::new(
            QueryIr::Text("fox".into()),
            search_fields,
            None,
            Vec::new(),
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            10,
            0,
            searchkernel_ir::ProjectionIr::new(Vec::new()),
            HighlightingIr::Requested {
                fields: vec![title],
            },
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        );
        let error = validate_search_request(&highlighting).unwrap_err();
        assert_eq!(error.code, searchkernel_ir::ErrorCode::Unsupported);
        assert_eq!(
            error.details[0].code,
            "highlighting_native_result_unavailable"
        );
    }

    #[test]
    fn ranking_and_ordered_comparisons_preserve_supported_boundaries() {
        assert_eq!(
            ranking_profile(RankingPolicyIr::DefaultRelevanceV1),
            Ok("default")
        );
        assert_eq!(
            ranking_profile(RankingPolicyIr::Bm25).unwrap_err().code,
            searchkernel_ir::ErrorCode::Unsupported
        );

        let price = field("price");
        for (operation, expected) in [
            (
                ComparisonOp::Lt,
                Query::AttributeRange {
                    field: "price".into(),
                    lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                    upper: searchkernel_vespa_engine::AttributeBound::Excluded(
                        VespaValue::Integer(10),
                    ),
                },
            ),
            (
                ComparisonOp::Lte,
                Query::AttributeRange {
                    field: "price".into(),
                    lower: searchkernel_vespa_engine::AttributeBound::Unbounded,
                    upper: searchkernel_vespa_engine::AttributeBound::Included(
                        VespaValue::Integer(10),
                    ),
                },
            ),
            (
                ComparisonOp::Gt,
                Query::AttributeRange {
                    field: "price".into(),
                    lower: searchkernel_vespa_engine::AttributeBound::Excluded(
                        VespaValue::Integer(10),
                    ),
                    upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
                },
            ),
            (
                ComparisonOp::Gte,
                Query::AttributeRange {
                    field: "price".into(),
                    lower: searchkernel_vespa_engine::AttributeBound::Included(
                        VespaValue::Integer(10),
                    ),
                    upper: searchkernel_vespa_engine::AttributeBound::Unbounded,
                },
            ),
        ] {
            assert_eq!(
                lower_comparison(&ComparisonIr::new(
                    price.clone(),
                    operation,
                    ScalarValueIr::Int64(10),
                ))
                .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn explicit_empty_projection_returns_no_document_fields() {
        let document = Document::new("doc-1")
            .field(
                "title",
                FieldValue::Scalar(ScalarValue::String("fox".into())),
            )
            .field("price", FieldValue::Scalar(ScalarValue::Integer(10)));
        assert!(project_document(&document, &[]).is_empty());
        assert_eq!(
            project_document(&document, &[field("title")]),
            BTreeMap::from([("title".into(), Value::String("fox".into()))])
        );
    }

    #[test]
    fn empty_projection_lowers_to_no_summary_fields() {
        let request = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            Vec::new(),
            None,
        );
        let language = analysis_options(request.language()).unwrap();
        let query = lower_search_query(&request, language).unwrap();
        let lowered = lower_search_options(&request, query, language).unwrap();

        assert_eq!(
            lowered.summary,
            searchkernel_vespa_engine::SummaryRequest::None
        );
    }

    #[test]
    fn every_expressible_search_option_is_lowered_to_the_generic_m_api() {
        let title = field("title");
        let description = field("description");
        let brand = field("brand");
        let price = field("price");
        let search_fields = vec![
            SearchFieldIr::new(title.clone(), SearchFieldWeight::new(1.0).unwrap()),
            SearchFieldIr::new(description.clone(), SearchFieldWeight::new(1.0).unwrap()),
        ];
        let sorts = vec![
            SortIr::new(price.clone(), SortDirection::Asc, MissingPlacement::First),
            SortIr::new(brand.clone(), SortDirection::Desc, MissingPlacement::Last),
        ];
        let facets = [
            FacetIr::new(brand.clone(), 7, searchkernel_ir::FacetSort::CountDesc),
            FacetIr::new(price.clone(), 6, searchkernel_ir::FacetSort::CountAsc),
            FacetIr::new(title.clone(), 5, searchkernel_ir::FacetSort::ValueAsc),
            FacetIr::new(
                description.clone(),
                4,
                searchkernel_ir::FacetSort::ValueDesc,
            ),
        ];
        let request = SearchRequestIr::new(
            QueryIr::Text("wireless headphones".into()),
            search_fields,
            Some(FilterIr::Comparison(ComparisonIr::new(
                price.clone(),
                ComparisonOp::Gte,
                ScalarValueIr::Int64(10),
            ))),
            sorts,
            facets.to_vec(),
            RankingPolicyIr::DefaultRelevanceV1,
            3,
            2,
            searchkernel_ir::ProjectionIr::new(vec![title.clone(), brand.clone()]),
            HighlightingIr::Disabled,
            Some(LanguageHint::parse("en-US").unwrap()),
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .with_timeout(Some(std::time::Duration::from_millis(123)));

        validate_search_request(&request).unwrap();
        let language = analysis_options(request.language()).unwrap();
        let query = lower_search_query(&request, language).unwrap();
        let lowered = lower_search_options(&request, query.clone(), language).unwrap();
        assert_eq!(lowered.query, query);
        assert_eq!(lowered.source, searchkernel_vespa_engine::Source::All);
        assert_eq!(lowered.rank_profile, "default");
        assert_eq!(lowered.limit, 3);
        assert_eq!(lowered.offset, 2);
        assert_eq!(lowered.timeout, std::time::Duration::from_millis(123));
        assert_eq!(lowered.language, language);
        assert_eq!(
            lowered.sort,
            vec![
                SortField::ascending("price").missing_first(),
                SortField::descending("brand").missing_last(),
            ]
        );
        assert_eq!(
            lowered.summary,
            searchkernel_vespa_engine::SummaryRequest::Fields(
                vec!["title".into(), "brand".into(),]
            )
        );

        let expected_orders = [
            searchkernel_vespa_engine::GroupingOrder::CountDescending,
            searchkernel_vespa_engine::GroupingOrder::CountAscending,
            searchkernel_vespa_engine::GroupingOrder::ValueAscending,
            searchkernel_vespa_engine::GroupingOrder::ValueDescending,
        ];
        for (facet, expected_order) in facets.iter().zip(expected_orders) {
            let grouping = lower_facet_grouping(query.clone(), facet);
            assert_eq!(grouping.query, query);
            assert_eq!(grouping.group_by, facet.field().as_str());
            assert_eq!(grouping.max_groups, Some(facet.limit()));
            assert_eq!(grouping.order, expected_order);
            assert!(grouping.aggregations.is_empty());
        }
    }

    #[test]
    fn unsupported_search_options_have_stable_narrow_codes() {
        let weighted = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(2.0).unwrap(),
            )],
            Vec::new(),
            None,
        );
        let error = validate_search_request(&weighted).unwrap_err();
        assert_eq!(error.details[0].code, "ranking_weights_not_supported");

        assert_eq!(
            ranking_profile(RankingPolicyIr::Bm25).unwrap_err().details[0].code,
            "bm25_ranking_not_supported"
        );
    }

    #[test]
    fn insert_and_upsert_are_linearizable_for_one_document_id() {
        let root = std::env::temp_dir().join(format!(
            "searchkernel-adapter-linearizable-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos()
        ));
        let data = root.join("data");
        let config = root.join("config");
        std::fs::create_dir_all(&data).expect("create test data directory");
        ConfigCompiler
            .compile_to(
                &Schema::new("searchkernel").field(
                    Field::new("value", FieldType::Integer)
                        .attribute()
                        .summary(),
                ),
                &config,
            )
            .expect("compile test schema");
        let engine = Engine::open(&data, &config).expect("open test engine");
        let probe = std::sync::Arc::new(MutationProbe {
            rendezvous: std::sync::Barrier::new(3),
        });
        let backend = std::sync::Arc::new(VespaBackend::with_mutation_probe(engine, probe.clone()));

        let insert_backend = std::sync::Arc::clone(&backend);
        let insert = std::thread::spawn(move || {
            insert_backend.insert(InsertRequestIr::new(test_document(1)))
        });
        let upsert_backend = std::sync::Arc::clone(&backend);
        let upsert = std::thread::spawn(move || {
            upsert_backend.upsert(UpsertRequestIr::new(test_document(2)))
        });

        probe.rendezvous.wait();
        assert_eq!(
            insert
                .join()
                .expect("insert thread")
                .map(|result| result.id),
            Ok("doc-1".into())
        );
        assert_eq!(
            upsert
                .join()
                .expect("upsert thread")
                .map(|result| result.id),
            Ok("doc-1".into())
        );

        let stored = backend
            .engine
            .get("doc-1")
            .expect("read final document")
            .expect("final document exists");
        assert_eq!(
            stored.fields().get("value"),
            Some(&searchkernel_vespa_engine::FieldValue::Scalar(
                searchkernel_vespa_engine::ScalarValue::Integer(2),
            ))
        );

        let backend = match std::sync::Arc::try_unwrap(backend) {
            Ok(backend) => backend,
            Err(_) => panic!("test backend still has owners"),
        };
        backend.engine.close().expect("close test engine");
        std::fs::remove_dir_all(root).expect("remove test index");
    }

    fn test_document(value: i64) -> searchkernel_ir::DocumentIr {
        searchkernel_ir::DocumentIr::new(
            searchkernel_ir::DocumentId::parse("doc-1").expect("valid document ID"),
            std::collections::BTreeMap::from([(
                field("value"),
                FieldValueIr::Single(ScalarValueIr::Int64(value)),
            )]),
        )
    }
}
