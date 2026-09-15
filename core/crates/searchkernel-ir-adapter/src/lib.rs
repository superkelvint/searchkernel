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
    ConfigCompiler, Document, DocumentUpdate, Engine, Field, FieldType, FieldValue, Query,
    RangeBound, ScalarValue, Schema, SearchRequest, SortField, SortOrder,
    TotalHitsRelation as VespaTotalHitsRelation, Value as VespaValue,
};

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
            return Err(unsupported(format!(
                "multi-valued field `{}` is not supported by the current M document/config ABI",
                field.name().as_str()
            )));
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
    if let Some(field) = definition.fields().iter().find(|field| {
        matches!(
            field.cardinality(),
            searchkernel_ir::FieldCardinality::Multi
        )
    }) {
        return Err(unsupported(format!(
            "multi-valued field `{}` is not supported by the current M document/config ABI",
            field.name().as_str()
        )));
    }
    Ok(())
}

/// A real Vespa-backed implementation of the portable backend contract.
pub struct VespaBackend {
    engine: Engine,
}

impl VespaBackend {
    pub fn open(
        data_dir: impl AsRef<std::path::Path>,
        config_dir: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        Engine::open(data_dir, config_dir)
            .map(|engine| Self { engine })
            .map_err(native_error)
    }

    pub fn from_engine(engine: Engine) -> Self {
        Self { engine }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Backend for VespaBackend {
    fn insert(&self, request: InsertRequestIr) -> Result<InsertResult> {
        let document = lower_document(request.document())?;
        let id = document.id().to_owned();
        if self.engine.get(&id).map_err(native_error)?.is_some() {
            return Err(PortableError::new(
                searchkernel_ir::ErrorCode::AlreadyExists,
                format!("document already exists: {id}"),
            ));
        }
        self.engine.put(document).map_err(native_error)?;
        Ok(InsertResult { id })
    }

    fn upsert(&self, request: UpsertRequestIr) -> Result<UpsertResult> {
        let document = lower_document(request.document())?;
        let id = document.id().to_owned();
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
        let projection = request.projection().fields();
        let fields = document
            .fields()
            .iter()
            .filter(|(name, _)| {
                projection.is_empty()
                    || projection
                        .iter()
                        .any(|field| field.as_str() == name.as_str())
            })
            .map(|(name, value)| (name.clone(), value_to_ir(value)))
            .collect();
        Ok(GetResult {
            document: Some(searchkernel_ir::DocumentResult {
                id: document.id().to_owned(),
                fields,
            }),
        })
    }

    fn search(&self, request: SearchRequestIr) -> Result<SearchResult> {
        validate_search_request(&request)?;
        let query = lower_search_query(&request)?;
        let mut m_request = SearchRequest::new(query)
            .limit(request.limit())
            .offset(request.offset())
            .rank_profile(match request.ranking() {
                searchkernel_ir::RankingPolicyIr::DefaultRelevanceV1 => "default",
                searchkernel_ir::RankingPolicyIr::Bm25 => "bm25",
            });
        for sort in request.sort() {
            m_request = m_request.sort(match sort.direction() {
                searchkernel_ir::SortDirection::Asc => {
                    SortField::new(sort.field().as_str(), SortOrder::Ascending)
                }
                searchkernel_ir::SortDirection::Desc => {
                    SortField::new(sort.field().as_str(), SortOrder::Descending)
                }
            });
        }
        if !request.projection().fields().is_empty() {
            m_request = m_request.fields(
                request
                    .projection()
                    .fields()
                    .iter()
                    .map(|field| field.as_str()),
            );
        }
        let result = self.engine.search(m_request).map_err(native_error)?;
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
            facets: Vec::new(),
        })
    }

    fn analyze(&self, request: AnalyzeRequestIr) -> Result<AnalyzeResult> {
        let analysis = searchkernel_vespa_engine::Linguistics.analyze_document(
            request.text(),
            searchkernel_vespa_engine::LinguisticsOptions::default(),
        );
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

fn lower_search_query(request: &SearchRequestIr) -> Result<Query> {
    let mut clauses = vec![lower_query(request.query(), request.search_fields())?];
    if let Some(filter) = request.filter() {
        clauses.push(lower_filter(filter)?);
    }
    if clauses.len() == 1 {
        Ok(clauses.pop().expect("one clause was inserted"))
    } else {
        Ok(Query::And(clauses))
    }
}

fn validate_search_request(request: &SearchRequestIr) -> Result<()> {
    if !request.facets().is_empty()
        || !matches!(
            request.highlighting(),
            searchkernel_ir::HighlightingIr::Disabled
        )
    {
        return Err(unsupported(
            "facets and highlighting are not yet lowered by the M API",
        ));
    }
    if request
        .search_fields()
        .iter()
        .any(|field| field.weight().get() != 1.0)
    {
        return Err(unsupported(
            "weighted search fields are not yet represented by the M query model",
        ));
    }
    if request
        .sort()
        .iter()
        .any(|sort| matches!(sort.missing(), searchkernel_ir::MissingPlacement::First))
    {
        return Err(unsupported(
            "missing-first sorting is not yet represented by the M query model",
        ));
    }
    if request.language().is_some() {
        return Err(unsupported(
            "per-request language is not yet represented by the M query model",
        ));
    }
    Ok(())
}

fn lower_query(query: &QueryIr, fields: &[searchkernel_ir::SearchFieldIr]) -> Result<Query> {
    match query {
        QueryIr::MatchAll => Ok(Query::True),
        QueryIr::Text(text) => {
            let term_count = searchkernel_vespa_engine::Linguistics
                .analyze_query(
                    text,
                    searchkernel_vespa_engine::LinguisticsOptions::default(),
                )
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
                .map(|child| lower_query(child, fields))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Or(children) => Ok(Query::Or(
            children
                .iter()
                .map(|child| lower_query(child, fields))
                .collect::<Result<Vec<_>>>()?,
        )),
        QueryIr::Not(child) => Ok(Query::Not(Box::new(lower_query(child, fields)?))),
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
        FilterIr::Exists { field } => Ok(Query::exists(field.as_str())),
        FilterIr::Comparison(comparison) => lower_comparison(comparison),
        FilterIr::Range(range) => lower_range(range),
    }
}

fn lower_comparison(comparison: &searchkernel_ir::ComparisonIr) -> Result<Query> {
    use searchkernel_ir::ComparisonOp;
    match (&comparison.op(), comparison.value()) {
        (ComparisonOp::Eq, ScalarValueIr::Int64(value)) => Ok(Query::equals(
            comparison.field().as_str(),
            integer_query_value(*value)?,
        )),
        (ComparisonOp::Eq, ScalarValueIr::String(value)) => Ok(Query::AttributeEquals {
            field: comparison.field().to_string(),
            value: VespaValue::String(value.clone()),
        }),
        (ComparisonOp::Ne, ScalarValueIr::Int64(value)) => Ok(Query::Not(Box::new(Query::equals(
            comparison.field().as_str(),
            integer_query_value(*value)?,
        )))),
        (ComparisonOp::Ne, ScalarValueIr::String(value)) => {
            Ok(Query::Not(Box::new(Query::AttributeEquals {
                field: comparison.field().to_string(),
                value: VespaValue::String(value.clone()),
            })))
        }
        _ => Err(unsupported(
            "comparison is not lowerable by the M query model",
        )),
    }
}

fn lower_range(range: &RangeIr) -> Result<Query> {
    let lower = range
        .lower()
        .map(|value| lower_bound(value, range.include_lower()))
        .transpose()?
        .unwrap_or(RangeBound::Unbounded);
    let upper = range
        .upper()
        .map(|value| lower_bound(value, range.include_upper()))
        .transpose()?
        .unwrap_or(RangeBound::Unbounded);
    Ok(Query::range(range.field().as_str(), lower, upper))
}

fn lower_bound(value: &ScalarValueIr, included: bool) -> Result<RangeBound> {
    match value {
        ScalarValueIr::Int64(value) => {
            let value = integer_query_value(*value)?;
            Ok(if included {
                RangeBound::Included(value)
            } else {
                RangeBound::Excluded(value)
            })
        }
        _ => Err(unsupported("only integer ranges are currently lowerable")),
    }
}

fn integer_query_value(value: i64) -> Result<i64> {
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
    let lower = message.to_ascii_lowercase();
    if lower.contains("unsupported") || lower.contains("not supported") {
        PortableError::unsupported("m_native", None::<String>, message)
    } else if error.code() == 1
        || lower.contains("invalid")
        || lower.contains("must not")
        || lower.contains("must contain")
        || lower.contains("unknown")
        || lower.contains("requires an")
    {
        PortableError::invalid("backend_request", None::<String>, message)
    } else {
        PortableError::new(searchkernel_ir::ErrorCode::Internal, message)
    }
}

fn unsupported(message: impl Into<String>) -> PortableError {
    PortableError::unsupported("m_lowering", None::<String>, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use searchkernel_ir::{
        AnalyzerIdentity, ComparisonIr, ComparisonOp, FieldCapabilities, FieldCardinality, FieldIr,
        FieldKind, FieldName, FieldValueIr, HighlightingIr, IndexDefinitionIr, LanguageHint,
        MissingPlacement, PrefixPolicy, QueryIr, RangeIr, RankingPolicyIr, ScalarValueIr,
        SearchFieldIr, SearchFieldWeight, SortDirection, SortIr, TypoPolicy,
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
        assert_eq!(
            lower_search_request_for_test(weighted).code,
            searchkernel_ir::ErrorCode::Unsupported
        );

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
        assert_eq!(
            lower_search_request_for_test(missing_first).code,
            searchkernel_ir::ErrorCode::Unsupported
        );

        let language = request(
            vec![SearchFieldIr::new(
                field("title"),
                SearchFieldWeight::new(1.0).unwrap(),
            )],
            Vec::new(),
            Some(LanguageHint::parse("en").unwrap()),
        );
        assert_eq!(
            lower_search_request_for_test(language).code,
            searchkernel_ir::ErrorCode::Unsupported
        );
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
            Query::exists("title")
        );
        assert_eq!(lower_filter(&eq_int).unwrap(), Query::equals("price", 10));
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
                Query::equals("price", 10),
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
            Query::range("price", RangeBound::Included(1), RangeBound::Excluded(5),)
        );
        assert!(lower_filter(&FilterIr::Comparison(ComparisonIr::new(
            price,
            ComparisonOp::Gt,
            ScalarValueIr::Float64(1.0),
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

    fn lower_search_request_for_test(request: SearchRequestIr) -> PortableError {
        validate_search_request(&request).unwrap_err()
    }
}
