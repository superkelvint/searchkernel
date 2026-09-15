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
    Document, DocumentUpdate, Engine, FieldValue, Query, RangeBound, ScalarValue, SearchRequest,
    SortField, SortOrder, TotalHitsRelation as VespaTotalHitsRelation, Value as VespaValue,
};

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
                    update = update.assign(field.as_str(), lower_field_value(value)?);
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
        Ok(GetResult {
            document: document.map(|document| searchkernel_ir::DocumentResult {
                id: document.id().to_owned(),
                fields: document
                    .fields()
                    .iter()
                    .map(|(name, value)| (name.clone(), value_to_ir(value)))
                    .collect(),
            }),
        })
    }

    fn search(&self, request: SearchRequestIr) -> Result<SearchResult> {
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
        result = result.field(field.as_str(), lower_field_value(value)?);
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

fn lower_query(query: &QueryIr, fields: &[searchkernel_ir::SearchFieldIr]) -> Result<Query> {
    match query {
        QueryIr::MatchAll => Ok(Query::True),
        QueryIr::Text(text) => {
            let clauses = fields
                .iter()
                .map(|field| Query::term(field.field().as_str(), text))
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
    PortableError::new(searchkernel_ir::ErrorCode::Internal, error.to_string())
}

fn unsupported(message: impl Into<String>) -> PortableError {
    PortableError::unsupported("m_lowering", None::<String>, message)
}
