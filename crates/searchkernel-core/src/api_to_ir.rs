//! FlatBuffers API to owned Search Kernel IR conversion.

use crate::error::{PortableError, Result};
use crate::model::{
    AnalyzeRequestIr, AnalyzerIdentity, ComparisonIr, ComparisonOp, DeleteRequestIr, DocumentId,
    DocumentIr, FacetIr, FacetSort, FieldCapabilities, FieldCardinality, FieldIr, FieldKind,
    FieldName, FieldValueIr, FilterIr, GetRequestIr, HighlightingIr, IndexDefinitionIr, IndexName,
    InsertRequestIr, LanguageHint, MissingPlacement, PatchOperationIr, PatchRequestIr,
    PrefixPolicy, ProjectionIr, QueryIr, RangeIr, RankingPolicyIr, ScalarValueIr, SearchFieldIr,
    SearchFieldWeight, SearchRequestIr, SortDirection, SortIr, TypoPolicy, UpsertRequestIr,
    WriteRequestIr,
};
use searchkernel_api::v1 as api;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub enum WriteRequestRef<'a> {
    Insert(api::InsertRequest<'a>),
    Upsert(api::UpsertRequest<'a>),
    Patch(api::PatchRequest<'a>),
    Delete(api::DeleteRequest<'a>),
}

pub fn index_definition_to_ir(definition: api::IndexDefinition<'_>) -> Result<IndexDefinitionIr> {
    let field_views = definition.fields();
    let field_count = field_views.map_or(0, |v| v.len());
    if field_count == 0 {
        return Err(invalid(
            "invalid_schema",
            Some("fields"),
            "an index must define at least one field",
        ));
    }
    let mut fields = Vec::with_capacity(field_count);
    let mut names = BTreeSet::new();
    if let Some(views) = field_views {
        for (index, field) in views.iter().enumerate() {
            let path = format!("fields[{index}]");
            let name = parse_field_name(field.name().unwrap_or_default(), format!("{path}.name"))?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique",
                ));
            }
            fields.push(field_to_ir_at(field, name, &path)?);
        }
    }
    let name = match definition.name() {
        None => None,
        Some(value) => Some(IndexName::parse(value).map_err(|_| {
            invalid(
                "invalid_index_name",
                Some("name"),
                "index name must not be empty",
            )
        })?),
    };
    let default_search_fields = match definition.default_search_fields() {
        None => fields
            .iter()
            .filter(|f| f.capabilities.searchable)
            .map(|f| SearchFieldIr {
                field: f.name.clone(),
                weight: SearchFieldWeight::new(1.0).expect("constant is valid"),
            })
            .collect(),
        Some(selection) => search_fields_to_ir_at(selection, &fields, "default_search_fields")?,
    };
    Ok(IndexDefinitionIr {
        name,
        fields,
        default_search_fields,
        default_ranking: RankingPolicyIr::DefaultRelevanceV1,
        default_prefix_policy: PrefixPolicy::Disabled,
        default_typo_policy: TypoPolicy::Disabled,
    })
}

pub fn field_to_ir(field: api::FieldDefinition<'_>) -> Result<FieldIr> {
    let name = parse_field_name(field.name().unwrap_or_default(), "name".to_owned())?;
    field_to_ir_at(field, name, "field")
}

pub fn analyzer_to_ir(
    analyzer: Option<api::Analyzer<'_>>,
    kind: FieldKind,
) -> Result<Option<AnalyzerIdentity>> {
    analyzer_to_ir_at(analyzer, kind, "analyzer")
}

pub struct IndexContext {
    definition: IndexDefinitionIr,
    fields: BTreeMap<String, usize>,
}

impl std::fmt::Debug for IndexContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexContext")
            .field("definition", &self.definition)
            .finish_non_exhaustive()
    }
}

impl IndexContext {
    pub fn from_api(definition: api::IndexDefinition<'_>) -> Result<Self> {
        let definition = index_definition_to_ir(definition)?;
        let fields = definition
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str().to_owned(), i))
            .collect();
        Ok(Self { definition, fields })
    }

    pub fn definition(&self) -> &IndexDefinitionIr {
        &self.definition
    }
    pub fn describe(&self) -> IndexDefinitionIr {
        self.definition.clone()
    }
    pub fn inspect_query(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        self.search_to_ir(request)
    }

    pub fn insert_to_ir(&self, request: api::InsertRequest<'_>) -> Result<InsertRequestIr> {
        Ok(InsertRequestIr {
            document: self.document_from_optional(request.document(), "document")?,
        })
    }
    pub fn upsert_to_ir(&self, request: api::UpsertRequest<'_>) -> Result<UpsertRequestIr> {
        Ok(UpsertRequestIr {
            document: self.document_from_optional(request.document(), "document")?,
        })
    }
    pub fn patch_to_ir(&self, request: api::PatchRequest<'_>) -> Result<PatchRequestIr> {
        let id = self.document_id(request.id(), "id")?;
        let operations = request.operations();
        let count = operations.map_or(0, |v| v.len());
        if count == 0 {
            return Err(invalid(
                "empty_patch",
                Some("operations"),
                "patch must contain at least one operation",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut converted_operations = Vec::with_capacity(count);
        if let Some(operations) = operations {
            for (index, operation) in operations.iter().enumerate() {
                let path = format!("operations[{index}]");
                let converted = self.patch_operation_to_ir(operation, &path, &mut seen)?;
                converted_operations.push(converted);
            }
        }
        Ok(PatchRequestIr {
            id,
            operations: converted_operations,
        })
    }
    pub fn delete_to_ir(&self, request: api::DeleteRequest<'_>) -> Result<DeleteRequestIr> {
        Ok(DeleteRequestIr {
            id: self.document_id(request.id(), "id")?,
        })
    }
    pub fn get_to_ir(&self, request: api::GetRequest<'_>) -> Result<GetRequestIr> {
        Ok(GetRequestIr {
            id: self.document_id(request.id(), "id")?,
            projection: self.projection_to_ir(request.projection())?,
        })
    }
    pub fn search_to_ir(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        let query = self.query_to_ir(request.query().unwrap_or_default(), request.query_expr())?;
        let search_fields_wrapper = request.search_fields();
        let search_fields = match search_fields_wrapper {
            None => self.definition.default_search_fields.clone(),
            Some(s) => self.search_fields_to_ir(s)?,
        };
        if matches!(query, QueryIr::Text(_)) && search_fields.is_empty() {
            return Err(PortableError::failed_precondition(
                "no_searchable_fields",
                Some("search_fields"),
                "a text query requires at least one searchable field",
            ));
        }
        let filter = request
            .filter()
            .map(|f| self.filter_to_ir_at(f, "filter"))
            .transpose()?;
        let sort = self.sorts_to_ir(request.sort())?;
        let facets = self.facets_to_ir(request.facets())?;
        let ranking = ranking_to_ir(request.ranking())?;
        let language = match request.language() {
            None => None,
            Some(v) => Some(parse_language(v, "language")?),
        };
        Ok(SearchRequestIr {
            query,
            search_fields,
            filter,
            sort,
            facets,
            ranking,
            limit: request.limit().map_or(20, |v| v.value()),
            offset: request.offset().map_or(0, |v| v.value()),
            projection: self.projection_to_ir(request.projection())?,
            highlighting: self.highlighting_to_ir(request.highlighting())?,
            language,
            prefix_policy: PrefixPolicy::Disabled,
            typo_policy: TypoPolicy::Disabled,
        })
    }
    pub fn analyze_to_ir(&self, request: api::AnalyzeRequest<'_>) -> Result<AnalyzeRequestIr> {
        let target = match request.target_type() {
            api::AnalyzeTarget::Analyzer => request
                .target_as_analyzer()
                .map(|a| analyzer_to_ir_at(Some(a), FieldKind::Text, "target.analyzer"))
                .transpose()?
                .flatten(),
            api::AnalyzeTarget::AnalyzeFieldTarget => {
                let target = request.target_as_analyze_field_target().ok_or_else(|| {
                    invalid(
                        "invalid_analyzer",
                        Some("target"),
                        "analyze request must select an analyzer or field",
                    )
                })?;
                let field = self.field(
                    target.field().unwrap_or_default(),
                    "target.field".to_owned(),
                )?;
                if field.kind != FieldKind::Text {
                    return Err(invalid(
                        "invalid_analyzer",
                        Some("target.field"),
                        "analysis requires a text field",
                    ));
                }
                field
                    .analyzer
                    .ok_or_else(|| {
                        invalid(
                            "invalid_analyzer",
                            Some("target.field"),
                            "field has no analyzer",
                        )
                    })?
                    .into()
            }
            _ => {
                return Err(invalid(
                    "invalid_analyze_target",
                    Some("target"),
                    "analyze request must select an analyzer or field",
                ))
            }
        }
        .ok_or_else(|| {
            invalid(
                "invalid_analyzer",
                Some("target"),
                "analyze request must select an analyzer or field",
            )
        })?;
        Ok(AnalyzeRequestIr {
            analyzer: target,
            text: request.text().unwrap_or_default().to_owned(),
            language: request
                .language()
                .map(|v| parse_language(v, "language"))
                .transpose()?,
        })
    }

    pub fn value_to_ir(&self, field: &str, value: api::Value<'_>) -> Result<FieldValueIr> {
        let field = self.field(field, "field".to_owned())?;
        self.value_to_ir_at(field, value, "value".to_owned())
    }
    pub fn document_to_ir(&self, document: api::Document<'_>) -> Result<DocumentIr> {
        self.document_from_optional(Some(document), "document")
    }
    pub fn search_fields_to_ir(
        &self,
        selection: api::SearchFieldSelection<'_>,
    ) -> Result<Vec<SearchFieldIr>> {
        search_fields_to_ir_at(selection, &self.definition.fields, "search_fields")
    }
    pub fn query_to_ir(
        &self,
        text: &str,
        expression: Option<api::QueryExpr<'_>>,
    ) -> Result<QueryIr> {
        match expression {
            Some(expression) => {
                if !text.trim().is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some("query"),
                        "query and query_expr cannot both be supplied",
                    ));
                }
                self.query_expr_to_ir(expression, "query_expr")
            }
            None if text.trim().is_empty() => Ok(QueryIr::MatchAll),
            None => Ok(QueryIr::Text(text.to_owned())),
        }
    }
    pub fn filter_to_ir(&self, filter: api::FilterExpr<'_>) -> Result<FilterIr> {
        self.filter_to_ir_at(filter, "filter")
    }
    pub fn comparison_to_ir(&self, comparison: api::FilterComparison<'_>) -> Result<ComparisonIr> {
        self.comparison_to_ir_at(comparison, "comparison")
    }
    pub fn range_to_ir(&self, range: api::FilterRange<'_>) -> Result<RangeIr> {
        self.range_to_ir_at(range, "range")
    }
    pub fn sort_to_ir(&self, sort: api::SortExpr<'_>) -> Result<SortIr> {
        self.sort_to_ir_at(sort, "sort")
    }
    pub fn facet_to_ir(&self, facet: api::FacetRequest<'_>) -> Result<FacetIr> {
        self.facet_to_ir_at(facet, "facet")
    }
    pub fn projection_to_ir(
        &self,
        projection: Option<api::FieldProjection<'_>>,
    ) -> Result<ProjectionIr> {
        self.projection_to_ir_at(projection)
    }
    pub fn highlighting_to_ir(
        &self,
        highlighting: Option<api::HighlightRequest<'_>>,
    ) -> Result<HighlightingIr> {
        self.highlighting_to_ir_at(highlighting)
    }
    pub fn ranking_to_ir(
        &self,
        ranking: Option<api::RankingPolicy<'_>>,
    ) -> Result<RankingPolicyIr> {
        ranking_to_ir(ranking)
    }
    pub fn write_to_ir(&self, request: WriteRequestRef<'_>) -> Result<WriteRequestIr> {
        match request {
            WriteRequestRef::Insert(r) => self.insert_to_ir(r).map(WriteRequestIr::Insert),
            WriteRequestRef::Upsert(r) => self.upsert_to_ir(r).map(WriteRequestIr::Upsert),
            WriteRequestRef::Patch(r) => self.patch_to_ir(r).map(WriteRequestIr::Patch),
            WriteRequestRef::Delete(r) => self.delete_to_ir(r).map(WriteRequestIr::Delete),
        }
    }
    pub fn bulk_write_to_ir<'a>(
        &self,
        requests: impl IntoIterator<Item = WriteRequestRef<'a>>,
    ) -> Result<Vec<WriteRequestIr>> {
        requests.into_iter().map(|r| self.write_to_ir(r)).collect()
    }

    fn field(&self, name: &str, path: String) -> Result<&FieldIr> {
        self.fields
            .get(name)
            .map(|&i| &self.definition.fields[i])
            .ok_or_else(|| invalid("unknown_field", Some(path), "field is not defined"))
    }

    fn patch_operation_to_ir(
        &self,
        operation: api::PatchOperation<'_>,
        path: &str,
        seen_fields: &mut BTreeSet<FieldName>,
    ) -> Result<PatchOperationIr> {
        match operation.operation_type() {
            api::PatchOperationKind::SetField => {
                let set = operation.operation_as_set_field().ok_or_else(|| {
                    invalid(
                        "invalid_patch_operation",
                        Some(path),
                        "patch operation must select set or unset",
                    )
                })?;
                let field =
                    self.field(set.field().unwrap_or_default(), format!("{path}.set.field"))?;
                ensure_field_is_not_repeated(seen_fields, field, path)?;
                let value = set.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.set.value")),
                        "set operation requires a value",
                    )
                })?;
                Ok(PatchOperationIr::Set {
                    field: field.name.clone(),
                    value: self.value_to_ir_at(field, value, format!("{path}.set.value"))?,
                })
            }
            api::PatchOperationKind::UnsetField => {
                let unset = operation.operation_as_unset_field().ok_or_else(|| {
                    invalid(
                        "invalid_patch_operation",
                        Some(path),
                        "patch operation must select set or unset",
                    )
                })?;
                let field = self.field(
                    unset.field().unwrap_or_default(),
                    format!("{path}.unset.field"),
                )?;
                ensure_field_is_not_repeated(seen_fields, field, path)?;
                Ok(PatchOperationIr::Unset {
                    field: field.name.clone(),
                })
            }
            _ => Err(invalid(
                "invalid_patch_operation",
                Some(path),
                "patch operation must select set or unset",
            )),
        }
    }
    fn document_id(&self, value: Option<&str>, path: &str) -> Result<DocumentId> {
        DocumentId::parse(value.unwrap_or_default().to_owned()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some(path),
                "document ID must not be empty",
            )
        })
    }
    fn document_from_optional(
        &self,
        document: Option<api::Document<'_>>,
        path: &str,
    ) -> Result<DocumentIr> {
        let document = document.ok_or_else(|| {
            invalid(
                "missing_value",
                Some(path),
                "document operation requires a document",
            )
        })?;
        let id = self.document_id(document.id(), "document.id")?;
        let mut fields = BTreeMap::new();
        if let Some(entries) = document.fields() {
            for entry in entries.iter() {
                let name = entry.key();
                let field = self.field(name, format!("document.fields.{name}"))?;
                let value = entry.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("document.fields.{name}")),
                        "value must select a scalar kind",
                    )
                })?;
                fields.insert(
                    field.name.clone(),
                    self.value_to_ir_at(field, value, format!("document.fields.{name}"))?,
                );
            }
        }
        Ok(DocumentIr { id, fields })
    }
    fn value_to_ir_at(
        &self,
        field: &FieldIr,
        value: api::Value<'_>,
        path: String,
    ) -> Result<FieldValueIr> {
        if value.kind_type() == api::ValueKind::NONE {
            return Err(invalid(
                "missing_value",
                Some(path),
                "value must select a scalar kind",
            ));
        }
        let value_list = if value.kind_type() == api::ValueKind::ValueList {
            Some(value.kind_as_value_list().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(path.clone()),
                    "value list union payload is missing",
                )
            })?)
        } else {
            None
        };
        match field.cardinality {
            FieldCardinality::Single => {
                if value_list.is_some() {
                    return Err(invalid(
                        "cardinality_mismatch",
                        Some(path),
                        "single-valued fields require a scalar value",
                    ));
                }
                Ok(FieldValueIr::Single(scalar_to_ir(value, field.kind, path)?))
            }
            FieldCardinality::Multi => {
                let list = value_list.ok_or_else(|| {
                    invalid(
                        "cardinality_mismatch",
                        Some(path.clone()),
                        "multi-valued fields require a list value",
                    )
                })?;
                let values = list
                    .values()
                    .map(|v| {
                        v.iter()
                            .enumerate()
                            .map(|(i, value)| {
                                scalar_to_ir(value, field.kind, format!("{path}.values[{i}]"))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()))?;
                Ok(FieldValueIr::Multi(values))
            }
        }
    }
    fn query_expr_to_ir(&self, expression: api::QueryExpr<'_>, path: &str) -> Result<QueryIr> {
        let tag = expression.expr_type();
        match tag {
            api::QueryExprKind::TermQuery => {
                let term = expression.expr_as_term_query().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let field = self.field(
                    term.field().unwrap_or_default(),
                    format!("{path}.term.field"),
                )?;
                if !field.capabilities.searchable {
                    return Err(invalid(
                        "field_not_searchable",
                        Some(format!("{path}.term.field")),
                        "query term requires a searchable field",
                    ));
                }
                let value = term.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.term.value")),
                        "query term requires a value",
                    )
                })?;
                Ok(QueryIr::Term {
                    field: field.name.clone(),
                    value: scalar_to_ir(value, field.kind, format!("{path}.term.value"))?,
                })
            }
            api::QueryExprKind::QueryAnd => {
                let node = expression.expr_as_query_and().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let clauses = query_children(node.clauses(), self, path, "all")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.all")),
                        "query all requires at least one clause",
                    ));
                }
                Ok(flatten_query_and(clauses))
            }
            api::QueryExprKind::QueryOr => {
                let node = expression.expr_as_query_or().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let clauses = query_children(node.clauses(), self, path, "any")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.any")),
                        "query any requires at least one clause",
                    ));
                }
                Ok(flatten_query_or(clauses))
            }
            api::QueryExprKind::QueryNot => {
                let node = expression.expr_as_query_not().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let clause = node.clause().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(format!("{path}.negation.clause")),
                        "query negation requires a clause",
                    )
                })?;
                Ok(QueryIr::Not(Box::new(self.query_expr_to_ir(
                    clause,
                    &format!("{path}.negation.clause"),
                )?)))
            }
            _ => Err(invalid(
                "invalid_query",
                Some(path),
                "query expression must select an operation",
            )),
        }
    }
    fn filter_to_ir_at(&self, filter: api::FilterExpr<'_>, path: &str) -> Result<FilterIr> {
        let tag = filter.expr_type();
        match tag {
            api::FilterExprKind::FilterAnd => {
                let n = filter.expr_as_filter_and().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let clauses = filter_children(n.clauses(), self, path, "all")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.all")),
                        "all requires at least one clause",
                    ));
                }
                Ok(flatten_and(clauses))
            }
            api::FilterExprKind::FilterOr => {
                let n = filter.expr_as_filter_or().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let clauses = filter_children(n.clauses(), self, path, "any")?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.any")),
                        "any requires at least one clause",
                    ));
                }
                Ok(flatten_or(clauses))
            }
            api::FilterExprKind::FilterNot => {
                let n = filter.expr_as_filter_not().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let c = n.clause().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(format!("{path}.negation.clause")),
                        "negation requires a clause",
                    )
                })?;
                Ok(FilterIr::Not(Box::new(
                    self.filter_to_ir_at(c, &format!("{path}.negation.clause"))?,
                )))
            }
            api::FilterExprKind::FilterComparison => self
                .comparison_to_ir_at(
                    filter.expr_as_filter_comparison().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.comparison"),
                )
                .map(FilterIr::Comparison),
            api::FilterExprKind::FilterRange => self
                .range_to_ir_at(
                    filter.expr_as_filter_range().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.range"),
                )
                .map(FilterIr::Range),
            api::FilterExprKind::FilterExists => {
                let n = filter.expr_as_filter_exists().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(path),
                        "filter expression must select an operation",
                    )
                })?;
                let f = self.field(
                    n.field().unwrap_or_default(),
                    format!("{path}.exists.field"),
                )?;
                if !f.capabilities.filterable {
                    return Err(invalid(
                        "field_not_filterable",
                        Some(format!("{path}.exists.field")),
                        "exists requires a filterable field",
                    ));
                }
                Ok(FilterIr::Exists {
                    field: f.name.clone(),
                })
            }
            _ => Err(invalid(
                "invalid_filter",
                Some(path),
                "filter expression must select an operation",
            )),
        }
    }
    fn comparison_to_ir_at(
        &self,
        c: api::FilterComparison<'_>,
        path: &str,
    ) -> Result<ComparisonIr> {
        let f = self.field(c.field().unwrap_or_default(), format!("{path}.field"))?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "comparison requires a filterable field",
            ));
        }
        let op = match c.op().0 {
            1 => ComparisonOp::Eq,
            2 => ComparisonOp::Ne,
            3 => ComparisonOp::Lt,
            4 => ComparisonOp::Lte,
            5 => ComparisonOp::Gt,
            6 => ComparisonOp::Gte,
            _ => {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{path}.op")),
                    "comparison operator is unspecified or unknown",
                ))
            }
        };
        if matches!(
            op,
            ComparisonOp::Lt | ComparisonOp::Lte | ComparisonOp::Gt | ComparisonOp::Gte
        ) && !matches!(f.kind, FieldKind::Int64 | FieldKind::Float64)
        {
            return Err(invalid(
                "invalid_filter",
                Some(format!("{path}.op")),
                "ordered comparison requires a numeric field",
            ));
        }
        let v = c.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.value")),
                "comparison requires a value",
            )
        })?;
        Ok(ComparisonIr {
            field: f.name.clone(),
            op,
            value: scalar_to_ir(v, f.kind, format!("{path}.value"))?,
        })
    }
    fn range_to_ir_at(&self, r: api::FilterRange<'_>, path: &str) -> Result<RangeIr> {
        let f = self.field(r.field().unwrap_or_default(), format!("{path}.field"))?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "range requires a filterable field",
            ));
        }
        if !matches!(f.kind, FieldKind::Int64 | FieldKind::Float64) {
            return Err(invalid(
                "invalid_filter",
                Some(path),
                "range requires an Int64 or Float64 field",
            ));
        }
        let lower = r.lower();
        let upper = r.upper();
        if lower.is_none() && upper.is_none() {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range requires at least one bound",
            ));
        }
        if lower.is_none() && r.include_lower().is_some() {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_lower")),
                "lower inclusion requires a lower bound",
            ));
        }
        if upper.is_none() && r.include_upper().is_some() {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_upper")),
                "upper inclusion requires an upper bound",
            ));
        }
        let lower = lower
            .map(|v| scalar_to_ir(v, f.kind, format!("{path}.lower")))
            .transpose()?;
        let upper = upper
            .map(|v| scalar_to_ir(v, f.kind, format!("{path}.upper")))
            .transpose()?;
        if let (Some(a), Some(b)) = (&lower, &upper) {
            if numeric_gt(a, b) {
                return Err(invalid(
                    "invalid_range",
                    Some(path),
                    "range lower bound must not exceed upper bound",
                ));
            }
        }
        Ok(RangeIr {
            field: f.name.clone(),
            lower,
            upper,
            include_lower: r.include_lower().is_none_or(|v| v.value()),
            include_upper: r.include_upper().is_none_or(|v| v.value()),
        })
    }
    fn sort_to_ir_at(&self, s: api::SortExpr<'_>, path: &str) -> Result<SortIr> {
        let f = self.field(s.field().unwrap_or_default(), format!("{path}.field"))?;
        if !f.capabilities.sortable {
            return Err(invalid(
                "field_not_sortable",
                Some(format!("{path}.field")),
                "sort requires a sortable field",
            ));
        }
        if f.cardinality != FieldCardinality::Single {
            return Err(invalid(
                "invalid_capability",
                Some(format!("{path}.field")),
                "multi-valued fields cannot be sortable",
            ));
        }
        let direction = match s.direction().0 {
            0 | 1 => SortDirection::Asc,
            2 => SortDirection::Desc,
            _ => {
                return Err(invalid(
                    "invalid_filter",
                    Some(format!("{path}.direction")),
                    "sort direction is unknown",
                ))
            }
        };
        let missing = match s.missing() {
            None => MissingPlacement::Last,
            Some(v) => match v.value().0 {
                1 => MissingPlacement::First,
                2 => MissingPlacement::Last,
                _ => {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.missing")),
                        "missing placement is unknown",
                    ))
                }
            },
        };
        Ok(SortIr {
            field: f.name.clone(),
            direction,
            missing,
        })
    }
    fn sorts_to_ir(
        &self,
        sorts: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::SortExpr<'_>>>>,
    ) -> Result<Vec<SortIr>> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        if let Some(sorts) = sorts {
            for (i, s) in sorts.iter().enumerate() {
                let v = self.sort_to_ir_at(s, &format!("sort[{i}]"))?;
                if !seen.insert(v.field.clone()) {
                    return Err(invalid(
                        "duplicate_sort_field",
                        Some(format!("sort[{i}].field")),
                        "sort fields must be unique",
                    ));
                }
                out.push(v)
            }
        }
        Ok(out)
    }
    fn facet_to_ir_at(&self, f: api::FacetRequest<'_>, path: &str) -> Result<FacetIr> {
        let field = self.field(f.field().unwrap_or_default(), format!("{path}.field"))?;
        if !field.capabilities.facetable {
            return Err(invalid(
                "field_not_facetable",
                Some(format!("{path}.field")),
                "facet requires a facetable field",
            ));
        }
        let limit = f.limit().map_or(10, |v| v.value());
        let sort = match f.sort() {
            None => FacetSort::CountDesc,
            Some(v) => match v.value().0 {
                1 => FacetSort::CountDesc,
                2 => FacetSort::CountAsc,
                3 => FacetSort::ValueAsc,
                4 => FacetSort::ValueDesc,
                _ => {
                    return Err(invalid(
                        "invalid_facet",
                        Some(format!("{path}.sort")),
                        "facet sort is unknown",
                    ))
                }
            },
        };
        Ok(FacetIr {
            field: field.name.clone(),
            limit,
            sort,
        })
    }
    fn facets_to_ir(
        &self,
        facets: Option<
            flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::FacetRequest<'_>>>,
        >,
    ) -> Result<Vec<FacetIr>> {
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        if let Some(facets) = facets {
            for (i, f) in facets.iter().enumerate() {
                let v = self.facet_to_ir_at(f, &format!("facets[{i}]"))?;
                if !seen.insert(v.field.clone()) {
                    return Err(invalid(
                        "duplicate_facet_field",
                        Some(format!("facets[{i}].field")),
                        "facet fields must be unique",
                    ));
                }
                out.push(v)
            }
        }
        Ok(out)
    }
    fn projection_to_ir_at(&self, p: Option<api::FieldProjection<'_>>) -> Result<ProjectionIr> {
        let mut out = Vec::new();
        match p {
            None => out.extend(
                self.definition
                    .fields
                    .iter()
                    .filter(|f| f.capabilities.stored)
                    .map(|f| f.name.clone()),
            ),
            Some(p) => {
                if let Some(fs) = p.fields() {
                    let mut seen = BTreeSet::new();
                    for (i, n) in fs.iter().enumerate() {
                        let f = self.field(n, format!("projection.fields[{i}]"))?;
                        if !f.capabilities.stored {
                            return Err(invalid(
                                "field_not_stored",
                                Some(format!("projection.fields[{i}]")),
                                "projection requires a stored field",
                            ));
                        }
                        if !seen.insert(f.name.clone()) {
                            return Err(invalid(
                                "duplicate_projection_field",
                                Some(format!("projection.fields[{i}]")),
                                "projection fields must be unique",
                            ));
                        }
                        out.push(f.name.clone())
                    }
                }
            }
        }
        Ok(ProjectionIr { fields: out })
    }
    fn highlighting_to_ir_at(
        &self,
        h: Option<api::HighlightRequest<'_>>,
    ) -> Result<HighlightingIr> {
        let Some(h) = h else {
            return Ok(HighlightingIr::Disabled);
        };
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        if let Some(fs) = h.fields() {
            for (i, n) in fs.iter().enumerate() {
                let f = self.field(n, format!("highlighting.fields[{i}]"))?;
                if f.kind != FieldKind::Text || !f.capabilities.searchable {
                    return Err(invalid(
                        "invalid_highlight",
                        Some(format!("highlighting.fields[{i}]")),
                        "highlighting requires a searchable text field",
                    ));
                }
                if !f.capabilities.stored {
                    return Err(invalid(
                        "field_not_stored",
                        Some(format!("highlighting.fields[{i}]")),
                        "highlighting requires a stored field",
                    ));
                }
                if !seen.insert(f.name.clone()) {
                    return Err(invalid(
                        "duplicate_highlight_field",
                        Some(format!("highlighting.fields[{i}]")),
                        "highlight fields must be unique",
                    ));
                }
                out.push(f.name.clone())
            }
        }
        Ok(HighlightingIr::Requested { fields: out })
    }
}

fn field_to_ir_at(
    definition: api::FieldDefinition<'_>,
    name: FieldName,
    path: &str,
) -> Result<FieldIr> {
    let kind = match definition.kind().0 {
        1 => FieldKind::Text,
        2 => FieldKind::Keyword,
        3 => FieldKind::Int64,
        4 => FieldKind::Float64,
        5 => FieldKind::Bool,
        _ => {
            return Err(invalid(
                "invalid_field_kind",
                Some(format!("{path}.kind")),
                "field kind is unspecified or unknown",
            ))
        }
    };
    let cardinality = match definition.cardinality().0 {
        0 | 1 => FieldCardinality::Single,
        2 => FieldCardinality::Multi,
        _ => {
            return Err(invalid(
                "invalid_cardinality",
                Some(format!("{path}.cardinality")),
                "field cardinality is unknown",
            ))
        }
    };
    let defaults = match kind {
        FieldKind::Text => FieldCapabilities {
            stored: true,
            searchable: true,
            filterable: false,
            sortable: false,
            facetable: false,
        },
        FieldKind::Keyword => FieldCapabilities {
            stored: true,
            searchable: false,
            filterable: true,
            sortable: true,
            facetable: true,
        },
        FieldKind::Int64 | FieldKind::Float64 => FieldCapabilities {
            stored: true,
            searchable: false,
            filterable: true,
            sortable: true,
            facetable: false,
        },
        FieldKind::Bool => FieldCapabilities {
            stored: true,
            searchable: false,
            filterable: true,
            sortable: true,
            facetable: true,
        },
    };
    let mut capabilities = FieldCapabilities {
        stored: definition.stored().map_or(defaults.stored, |v| v.value()),
        searchable: definition
            .searchable()
            .map_or(defaults.searchable, |v| v.value()),
        filterable: definition
            .filterable()
            .map_or(defaults.filterable, |v| v.value()),
        sortable: definition
            .sortable()
            .map_or(defaults.sortable, |v| v.value()),
        facetable: definition
            .facetable()
            .map_or(defaults.facetable, |v| v.value()),
    };
    if cardinality == FieldCardinality::Multi && definition.sortable().is_none() {
        capabilities.sortable = false;
    }
    if kind == FieldKind::Text && capabilities.filterable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.filterable")),
            "text fields have incompatible capabilities",
        ));
    }
    if kind == FieldKind::Text && capabilities.sortable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.sortable")),
            "text fields have incompatible capabilities",
        ));
    }
    if kind == FieldKind::Text && capabilities.facetable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.facetable")),
            "text fields have incompatible capabilities",
        ));
    }
    if kind != FieldKind::Text && capabilities.searchable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.searchable")),
            "non-text fields cannot be searchable",
        ));
    }
    if cardinality == FieldCardinality::Multi && capabilities.sortable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.sortable")),
            "multi-valued fields cannot be sortable",
        ));
    }
    Ok(FieldIr {
        name,
        kind,
        cardinality,
        analyzer: analyzer_to_ir_at(definition.analyzer(), kind, &format!("{path}.analyzer"))?,
        capabilities,
    })
}
fn analyzer_to_ir_at(
    analyzer: Option<api::Analyzer<'_>>,
    kind: FieldKind,
    path: &str,
) -> Result<Option<AnalyzerIdentity>> {
    if kind != FieldKind::Text {
        if analyzer.is_some() {
            return Err(invalid(
                "invalid_analyzer",
                Some(path),
                "only text fields may specify an analyzer",
            ));
        }
        return Ok(None);
    }
    let Some(analyzer) = analyzer else {
        return Ok(Some(AnalyzerIdentity::StandardV1));
    };
    if analyzer.name().unwrap_or_default() == "standard"
        && analyzer.version() == 1
        && analyzer.options().map_or(0, |v| v.len()) == 0
    {
        Ok(Some(AnalyzerIdentity::StandardV1))
    } else {
        Err(PortableError::unsupported(
            "unsupported_analyzer",
            Some(path),
            "only standard analyzer version 1 without options is supported",
        ))
    }
}
fn search_fields_to_ir_at(
    selection: api::SearchFieldSelection<'_>,
    fields: &[FieldIr],
    path: &str,
) -> Result<Vec<SearchFieldIr>> {
    let mut search_fields = Vec::new();
    let mut seen_fields = BTreeSet::new();
    if let Some(fields_selection) = selection.fields() {
        for (index, search_field) in fields_selection.iter().enumerate() {
            let field_path = format!("{path}.fields[{index}].field");
            let f = fields
                .iter()
                .find(|field| field.name.as_str() == search_field.field().unwrap_or_default())
                .ok_or_else(|| {
                    invalid(
                        "unknown_field",
                        Some(field_path.clone()),
                        "field is not defined",
                    )
                })?;
            if !f.capabilities.searchable {
                return Err(invalid(
                    "field_not_searchable",
                    Some(field_path),
                    "search field must be searchable",
                ));
            }
            if !seen_fields.insert(f.name.clone()) {
                return Err(invalid(
                    "duplicate_search_field",
                    Some(format!("{path}.fields[{index}].field")),
                    "search fields must be unique",
                ));
            }
            let weight =
                SearchFieldWeight::new(search_field.weight().map_or(1.0, |value| value.value()))
                    .map_err(|_| {
                        invalid(
                            "invalid_search_weight",
                            Some(format!("{path}.fields[{index}].weight")),
                            "search-field weight must be finite and non-negative",
                        )
                    })?;
            search_fields.push(SearchFieldIr {
                field: f.name.clone(),
                weight,
            })
        }
    }
    Ok(search_fields)
}
fn ranking_to_ir(r: Option<api::RankingPolicy<'_>>) -> Result<RankingPolicyIr> {
    let Some(r) = r else {
        return Ok(RankingPolicyIr::DefaultRelevanceV1);
    };
    match r.policy_type() {
        api::RankingPolicyKind::DefaultRelevanceV1 => {
            r.policy_as_default_relevance_v1().ok_or_else(|| {
                invalid(
                    "invalid_ranking",
                    Some("ranking"),
                    "ranking policy must select a policy",
                )
            })?;
            Ok(RankingPolicyIr::DefaultRelevanceV1)
        }
        api::RankingPolicyKind::Bm25Ranking => {
            r.policy_as_bm_25_ranking().ok_or_else(|| {
                invalid(
                    "invalid_ranking",
                    Some("ranking"),
                    "ranking policy must select a policy",
                )
            })?;
            Ok(RankingPolicyIr::Bm25)
        }
        _ => Err(invalid(
            "invalid_ranking",
            Some("ranking"),
            "ranking policy must select a policy",
        )),
    }
}
fn scalar_to_ir(v: api::Value<'_>, kind: FieldKind, path: String) -> Result<ScalarValueIr> {
    if v.kind_type() == api::ValueKind::NONE {
        return Err(invalid(
            "missing_value",
            Some(path),
            "value must select a scalar kind",
        ));
    }
    let scalar = match v.kind_type() {
        api::ValueKind::BoolValue => ScalarValueIr::Bool(
            v.kind_as_bool_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value must select a scalar kind",
                    )
                })?
                .value(),
        ),
        api::ValueKind::Int64Value => ScalarValueIr::Int64(
            v.kind_as_int_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value must select a scalar kind",
                    )
                })?
                .value(),
        ),
        api::ValueKind::Float64Value => {
            let x = v
                .kind_as_float_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value must select a scalar kind",
                    )
                })?
                .value();
            if !x.is_finite() {
                return Err(invalid(
                    "non_finite_number",
                    Some(path),
                    "floating-point values must be finite",
                ));
            }
            ScalarValueIr::Float64(x)
        }
        api::ValueKind::StringValue => ScalarValueIr::String(
            v.kind_as_string_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value must select a scalar kind",
                    )
                })?
                .value()
                .unwrap_or_default()
                .to_owned(),
        ),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "value form is not supported for this field",
            ))
        }
    };
    let matches = matches!(
        (&scalar, kind),
        (
            ScalarValueIr::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
            | (ScalarValueIr::Bool(_), FieldKind::Bool)
    );
    if !matches {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "value type does not match field kind",
        ));
    }
    Ok(scalar)
}
fn parse_field_name(v: &str, path: String) -> Result<FieldName> {
    FieldName::parse(v.to_owned()).map_err(|_| {
        invalid(
            "invalid_field_name",
            Some(path),
            "field name must match [A-Za-z][A-Za-z0-9_]*",
        )
    })
}
fn parse_language(v: &str, path: &str) -> Result<LanguageHint> {
    LanguageHint::parse(v.to_owned()).map_err(|_| {
        invalid(
            "invalid_language",
            Some(path),
            "language must be a basic BCP-47-style ASCII tag",
        )
    })
}
fn filter_children(
    children: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::FilterExpr<'_>>>>,
    context: &IndexContext,
    parent_path: &str,
    operation_name: &str,
) -> Result<Vec<FilterIr>> {
    let mut converted = Vec::new();
    if let Some(children) = children {
        for (index, child) in children.iter().enumerate() {
            let child_path = format!("{parent_path}.{operation_name}.clauses[{index}]");
            converted.push(context.filter_to_ir_at(child, &child_path)?);
        }
    }
    Ok(converted)
}
fn query_children(
    children: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::QueryExpr<'_>>>>,
    context: &IndexContext,
    parent_path: &str,
    operation_name: &str,
) -> Result<Vec<QueryIr>> {
    let mut converted = Vec::new();
    if let Some(children) = children {
        for (index, child) in children.iter().enumerate() {
            let child_path = format!("{parent_path}.{operation_name}.clauses[{index}]");
            converted.push(context.query_expr_to_ir(child, &child_path)?);
        }
    }
    Ok(converted)
}
fn flatten_and(clauses: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let FilterIr::And(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        FilterIr::And(flattened)
    }
}
fn flatten_or(clauses: Vec<FilterIr>) -> FilterIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let FilterIr::Or(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        FilterIr::Or(flattened)
    }
}
fn flatten_query_and(clauses: Vec<QueryIr>) -> QueryIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let QueryIr::And(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        QueryIr::And(flattened)
    }
}
fn flatten_query_or(clauses: Vec<QueryIr>) -> QueryIr {
    let mut flattened = Vec::new();
    for clause in clauses {
        if let QueryIr::Or(nested_clauses) = clause {
            flattened.extend(nested_clauses)
        } else {
            flattened.push(clause)
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause was checked")
    } else {
        QueryIr::Or(flattened)
    }
}
fn numeric_gt(a: &ScalarValueIr, b: &ScalarValueIr) -> bool {
    match (a, b) {
        (ScalarValueIr::Int64(a), ScalarValueIr::Int64(b)) => a > b,
        (ScalarValueIr::Float64(a), ScalarValueIr::Float64(b)) => a > b,
        _ => false,
    }
}

fn ensure_field_is_not_repeated(
    seen_fields: &mut BTreeSet<FieldName>,
    field: &FieldIr,
    path: &str,
) -> Result<()> {
    if seen_fields.insert(field.name.clone()) {
        return Ok(());
    }
    Err(invalid(
        "duplicate_patch_field",
        Some(path),
        "a patch may mention each field at most once",
    ))
}

fn invalid(
    code: impl Into<String>,
    path: Option<impl Into<String>>,
    message: impl Into<String>,
) -> PortableError {
    PortableError::invalid(code, path, message)
}
