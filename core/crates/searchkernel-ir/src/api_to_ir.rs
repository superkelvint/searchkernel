//! FlatBuffers API to owned Search Kernel IR conversion.

use crate::error::{PortableError, Result};
use crate::filter_expression::{bind_filter_expression_with_references, FilterParameters};
use crate::model::{
    AnalyzeRequestIr, AnalyzerIdentity, AnalyzerIr, AttributeConfigIr, ComparisonIr, ComparisonOp,
    ConstantDefinitionIr, ConstantSourceIr, DataTypeIr, DeleteRequestIr, DistanceMetricIr,
    DocumentId, DocumentIr, DocumentSummaryDefinitionIr, DocumentSummaryFieldDefinitionIr,
    FieldCapabilities, FieldCardinality, FieldIr, FieldKind, FieldName, FieldSetDefinitionIr,
    FieldValueIr, FilterIr, GetRequestIr, GroupingAggregationIr, GroupingAllIr,
    GroupingBinaryOperatorIr, GroupingBucketIr, GroupingExpressionIr, GroupingFilterIr,
    GroupingOperationIr, GroupingOutputIr, GroupingRangeBoundIr, GroupingRequestIr, HighlightingIr,
    HnswIndexConfigIr, ImportedFieldDefinitionIr, IndexConfigIr, IndexDefinitionIr, IndexName,
    InsertRequestIr, LanguageHint, MatchingConfigIr, MatchingModeIr, MissingPlacement,
    NamedStructDefinitionIr, OnnxModelDefinitionIr, PatchOperationIr, PatchRequestIr, PrefixPolicy,
    PresentationIr, PrimitiveDataTypeIr, ProjectionIr, QueryInputIr, QueryIr, QueryModelIr,
    QueryModelTypeIr, RangeIr, RankFunctionDefinitionIr, RankPhaseDefinitionIr,
    RankProfileDefinitionIr, RankProfileInputIr, RankingPolicyIr, RankingRequestIr,
    RequestNonFiniteFloatIr, RequestTensorBlockIr, RequestTensorCellIr, RequestTensorIr,
    RequestTensorPayloadIr, RequestValueIr, ScalarValueIr, SchemaPropertiesIr, SearchFieldIr,
    SearchFieldWeight, SearchRequestIr, SortDirection, SortIr, StructFieldConfigIr, StructFieldIr,
    SummaryConfigIr, TypoPolicy, UpsertRequestIr, Value, WriteRequestIr,
};
use searchkernel_protocol::v2 as api;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

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
            let name = parse_field_name(field.name(), format!("{path}.name"))?;
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
    let properties = schema_properties_to_ir(definition.properties(), "properties")?;
    let document_type = optional_nonempty(definition.document_type(), "document_type")?;
    let schema_inherits = strings_to_ir(definition.schema_inherits(), "schema_inherits")?;
    let document_inherits = strings_to_ir(definition.document_inherits(), "document_inherits")?;
    let structs = named_structs_to_ir(definition.structs())?;
    let schema_fields = schema_fields_to_ir(definition.schema_fields(), &names)?;
    let all_fields = fields
        .iter()
        .chain(schema_fields.iter())
        .collect::<Vec<_>>();
    let searchable_fields = all_fields
        .iter()
        .map(|field| (*field).clone())
        .collect::<Vec<_>>();
    validate_struct_references(&fields, &structs)?;
    validate_struct_references(&schema_fields, &structs)?;
    let default_search_fields = match definition.default_search_fields() {
        None => all_fields
            .iter()
            .filter(|field| field.capabilities.searchable)
            .map(|field| SearchFieldIr {
                field: field.name.clone(),
                weight: SearchFieldWeight::new(1.0).expect("constant is valid"),
            })
            .collect(),
        Some(selection) => {
            search_fields_to_ir_at(selection, &searchable_fields, "default_search_fields")?
        }
    };
    let fieldsets = fieldsets_to_ir(definition.fieldsets(), &all_fields)?;
    let imported_fields = imported_fields_to_ir(definition.imported_fields(), &all_fields)?;
    let document_summaries =
        document_summaries_to_ir(definition.document_summaries(), &all_fields)?;
    let constants = constants_to_ir(definition.constants(), "constants")?;
    let onnx_models = onnx_models_to_ir(definition.onnx_models(), "onnx_models")?;
    let rank_profiles = rank_profiles_to_ir(definition.rank_profiles())?;
    Ok(IndexDefinitionIr {
        name,
        fields,
        default_search_fields,
        default_ranking: RankingPolicyIr::DefaultRelevanceV1,
        default_prefix_policy: PrefixPolicy::Disabled,
        default_typo_policy: TypoPolicy::Disabled,
        properties,
        document_type,
        schema_inherits,
        document_inherits,
        structs,
        fieldsets,
        schema_fields,
        imported_fields,
        document_summaries,
        rank_profiles,
        constants,
        onnx_models,
    })
}

pub fn field_to_ir(field: api::FieldDefinition<'_>) -> Result<FieldIr> {
    let name = parse_field_name(field.name(), "name".to_owned())?;
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
            condition: condition_to_ir(request.condition()),
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
            condition: condition_to_ir(request.condition()),
        })
    }
    pub fn get_to_ir(&self, request: api::GetRequest<'_>) -> Result<GetRequestIr> {
        Ok(GetRequestIr {
            id: self.document_id(request.id(), "id")?,
            projection: self.projection_to_ir(request.projection())?,
        })
    }
    pub fn search_to_ir(&self, request: api::SearchRequest<'_>) -> Result<SearchRequestIr> {
        let query = self.query_source_to_ir(request.query())?;
        let search_fields_wrapper = request.search_fields();
        validate_search_request_combinations(request, &query)?;
        let search_fields = match (&query, search_fields_wrapper) {
            (_, None) => self.definition.default_search_fields.clone(),
            (_, Some(s)) => self.search_fields_to_ir(s)?,
        };
        if matches!(query, QueryIr::Text(_)) && search_fields.is_empty() {
            return Err(PortableError::failed_precondition(
                "no_searchable_fields",
                Some("search_fields"),
                "a text query requires at least one searchable field",
            ));
        }
        let filter = self.filters_to_ir(request.filters())?;
        let sort = self.sorts_to_ir(request.sort())?;
        let grouping = grouping_to_ir(request.grouping())?;
        let ranking = ranking_to_ir(request.ranking())?;
        let query_model = query_model_to_ir(request.query_model())?;
        let language = query_model.language.clone();
        let presentation = presentation_to_ir(request.presentation())?;
        let timeout = request
            .timeout_ms()
            .map(|value| Duration::from_millis(value.value()));
        Ok(SearchRequestIr {
            query,
            search_fields,
            filter,
            sort,
            grouping,
            ranking,
            ranking_request: ranking_request_to_ir(request.ranking())?,
            inputs: query_inputs_to_ir(request.inputs())?,
            limit: request.limit().map_or(20, |v| v.value()),
            offset: request.offset().map_or(0, |v| v.value()),
            projection: self.projection_to_ir(request.presentation().and_then(|p| p.fields()))?,
            highlighting: self
                .highlighting_to_ir(request.presentation().and_then(|p| p.highlighting()))?,
            language,
            query_model,
            presentation,
            properties: request_values_to_map(request.properties(), "properties")?,
            query_profile: normalized_optional_string(request.query_profile()),
            search_chain: normalized_optional_string(request.search_chain()),
            timeout,
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
                if field.kind() != FieldKind::Text {
                    return Err(invalid(
                        "invalid_analyzer",
                        Some("target.field"),
                        "analysis requires a text field",
                    ));
                }
                let analyzer = field.analyzer().ok_or_else(|| {
                    invalid(
                        "invalid_analyzer",
                        Some("target.field"),
                        "field has no analyzer",
                    )
                })?;
                Some(analyzer_identity(analyzer, "target.field")?)
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

    fn query_source_to_ir(&self, source: Option<api::QuerySource<'_>>) -> Result<QueryIr> {
        let source = source.ok_or_else(|| {
            invalid(
                "missing_query",
                Some("query"),
                "search request requires a query",
            )
        })?;
        match source.source_type() {
            api::QuerySourceKind::SearchKernelTextQuery => self.query_to_ir(
                source
                    .source_as_search_kernel_text_query()
                    .and_then(|query| query.text())
                    .unwrap_or_default(),
                None,
            ),
            api::QuerySourceKind::TypedQuery => {
                let query = source
                    .source_as_typed_query()
                    .and_then(|query| query.query())
                    .ok_or_else(|| {
                        invalid("missing_query", Some("query"), "typed query is missing")
                    })?;
                self.query_expr_to_ir(query, "query")
            }
            api::QuerySourceKind::MatchAllQuery => Ok(QueryIr::MatchAll),
            api::QuerySourceKind::YqlQuery => {
                let yql = source.source_as_yql_query().ok_or_else(|| {
                    invalid("invalid_query", Some("query"), "YQL query is missing")
                })?;
                let source = yql.source().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some("query.source"),
                        "YQL source is required",
                    )
                })?;
                Ok(QueryIr::Yql {
                    source: source.to_owned(),
                    parameters: request_values_to_map(yql.parameters(), "query.parameters")?,
                })
            }
            _ => Err(invalid(
                "invalid_query",
                Some("query"),
                "unknown query source",
            )),
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
        ranking: Option<api::RankingRequest<'_>>,
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
        match field.cardinality() {
            FieldCardinality::Single => {
                if value_list.is_some() {
                    return Err(invalid(
                        "cardinality_mismatch",
                        Some(path),
                        "single-valued fields require a scalar value",
                    ));
                }
                Ok(FieldValueIr::Single(scalar_to_ir(
                    value,
                    field.kind(),
                    path,
                )?))
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
                                scalar_to_ir(value, field.kind(), format!("{path}.values[{i}]"))
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
            api::QueryExprKind::TermQuery => Err(invalid(
                "missing_query_field",
                Some(format!("{path}.term_query")),
                "query term requires a containing field",
            )),
            api::QueryExprKind::QueryContains => {
                let contains = expression.expr_as_query_contains().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let field_name = contains.field().ok_or_else(|| {
                    invalid(
                        "missing_query_field",
                        Some(format!("{path}.contains.field")),
                        "query contains requires a field",
                    )
                })?;
                let field = self.field(
                    field_name.path().unwrap_or_default(),
                    format!("{path}.contains.field.path"),
                )?;
                if !field.capabilities.searchable {
                    return Err(invalid(
                        "field_not_searchable",
                        Some(format!("{path}.contains.field")),
                        "query term requires a searchable field",
                    ));
                }
                let item = contains.item().ok_or_else(|| {
                    invalid(
                        "missing_query_item",
                        Some(format!("{path}.contains.item")),
                        "query contains requires an item",
                    )
                })?;
                let term = item.expr_as_term_query().ok_or_else(|| {
                    invalid(
                        "unsupported_query",
                        Some(format!("{path}.contains.item")),
                        "only term query items are supported by the embedded IR",
                    )
                })?;
                let value = term.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.contains.item.value")),
                        "query term requires a value",
                    )
                })?;
                Ok(QueryIr::Term {
                    field: field.name.clone(),
                    value: scalar_to_ir(
                        value,
                        field.kind(),
                        format!("{path}.contains.item.value"),
                    )?,
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
    fn filters_to_ir(&self, filters: Option<api::FilterRequest<'_>>) -> Result<Option<FilterIr>> {
        let Some(filters) = filters else {
            return Ok(None);
        };
        let mut clauses = filters
            .structured()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        self.filter_to_ir_at(value, &format!("filters.structured[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        let parameters = filter_parameters(filters.parameters())?;
        let mut referenced_parameters = BTreeSet::new();
        if let Some(sources) = filters.sources() {
            for (clause_index, source) in sources.iter().enumerate() {
                let bound =
                    bind_filter_expression_with_references(source, &self.definition, &parameters)
                        .map_err(|error| decorate_filter_source_error(error, clause_index))?;
                referenced_parameters.extend(bound.referenced_parameters().iter().cloned());
                clauses.push(bound.expression().clone());
            }
        }
        if let Some((name, _)) = parameters
            .iter()
            .find(|(name, _)| !referenced_parameters.contains(*name))
        {
            return Err(invalid(
                "unused_parameter",
                Some(format!("filters.parameters.{name}")),
                format!("parameter `:{name}` is not referenced by a filter source"),
            ));
        }
        Ok(match clauses.len() {
            0 => None,
            1 => clauses.into_iter().next(),
            _ => Some(FilterIr::And(clauses)),
        })
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
                    n.field().and_then(|field| field.path()).unwrap_or_default(),
                    format!("{path}.exists.field.path"),
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
            api::FilterExprKind::FilterIn => self
                .in_to_ir_at(
                    filter.expr_as_filter_in().ok_or_else(|| {
                        invalid(
                            "invalid_filter",
                            Some(path),
                            "filter expression must select an operation",
                        )
                    })?,
                    &format!("{path}.in"),
                )
                .map(|(field, values)| FilterIr::In { field, values }),
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
        let f = self.field(
            c.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field.path"),
        )?;
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
        ) && !matches!(f.kind(), FieldKind::Int64 | FieldKind::Float64)
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
            value: scalar_to_ir(v, f.kind(), format!("{path}.value"))?,
        })
    }
    fn in_to_ir_at(
        &self,
        input: api::FilterIn<'_>,
        path: &str,
    ) -> Result<(FieldName, Vec<ScalarValueIr>)> {
        let f = self.field(
            input
                .field()
                .and_then(|field| field.path())
                .unwrap_or_default(),
            format!("{path}.field.path"),
        )?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "IN requires a filterable field",
            ));
        }
        if !matches!(f.kind(), FieldKind::Keyword | FieldKind::Int64) {
            return Err(invalid(
                "invalid_filter",
                Some(path),
                "IN requires a Keyword or Int64 field",
            ));
        }
        let values = input
            .values()
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        scalar_to_ir(value, f.kind(), format!("{path}.values[{index}]"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        if values.is_empty() {
            return Err(invalid(
                "empty_in",
                Some(path),
                "IN requires at least one value",
            ));
        }
        Ok((f.name.clone(), values))
    }
    fn range_to_ir_at(&self, r: api::FilterRange<'_>, path: &str) -> Result<RangeIr> {
        let f = self.field(
            r.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field.path"),
        )?;
        if !f.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "range requires a filterable field",
            ));
        }
        if !matches!(f.kind(), FieldKind::Int64 | FieldKind::Float64) {
            return Err(invalid(
                "invalid_filter",
                Some(path),
                "range requires an Int64 or Float64 field",
            ));
        }
        let lower = finite_range_bound(r.lower(), &format!("{path}.lower"))?;
        let upper = finite_range_bound(r.upper(), &format!("{path}.upper"))?;
        if lower.is_none() && upper.is_none() {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range requires at least one bound",
            ));
        }
        if lower.is_none() && r.include_lower().is_some_and(|v| v.value()) {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_lower")),
                "lower inclusion requires a lower bound",
            ));
        }
        if upper.is_none() && r.include_upper().is_some_and(|v| v.value()) {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_upper")),
                "upper inclusion requires an upper bound",
            ));
        }
        let lower = lower
            .map(|v| scalar_to_ir(v, f.kind(), format!("{path}.lower")))
            .transpose()?;
        let upper = upper
            .map(|v| scalar_to_ir(v, f.kind(), format!("{path}.upper")))
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
        let target = s.target().ok_or_else(|| {
            invalid(
                "invalid_sort",
                Some(path),
                "sort expression requires a target",
            )
        })?;
        if target.target_type() != api::SortTargetKind::FieldSortTarget {
            return Err(PortableError::unsupported(
                "unsupported_sort",
                Some(path),
                "only field sort targets are supported by the embedded IR",
            ));
        }
        let f = self.field(
            target
                .target_as_field_sort_target()
                .and_then(|target| target.field())
                .unwrap_or_default(),
            format!("{path}.target.field"),
        )?;
        if !f.capabilities.sortable {
            return Err(invalid(
                "field_not_sortable",
                Some(format!("{path}.target.field")),
                "sort requires a sortable field",
            ));
        }
        if f.cardinality() != FieldCardinality::Single {
            return Err(invalid(
                "invalid_capability",
                Some(format!("{path}.target.field")),
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
            Some(v) => match v.policy_type() {
                api::MissingPolicyKind::MissingFirst if v.policy_as_missing_first().is_some() => {
                    MissingPlacement::First
                }
                api::MissingPolicyKind::MissingLast if v.policy_as_missing_last().is_some() => {
                    MissingPlacement::Last
                }
                _ => {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.missing")),
                        "missing policy is unsupported or missing its payload",
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
                if f.kind() != FieldKind::Text || !f.capabilities.searchable {
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

fn validate_search_request_combinations(
    request: api::SearchRequest<'_>,
    query: &QueryIr,
) -> Result<()> {
    if matches!(query, QueryIr::Yql { .. }) {
        if request.filters().is_some() {
            return Err(invalid(
                "yql_conflict",
                Some("filters"),
                "YQL cannot be combined with high-level filters",
            ));
        }
        if request.sort().is_some_and(|sort| !sort.is_empty()) {
            return Err(invalid(
                "yql_conflict",
                Some("sort"),
                "YQL cannot be combined with high-level sort",
            ));
        }
        if let Some(presentation) = request.presentation() {
            if presentation.fields().is_some() || presentation.highlighting().is_some() {
                return Err(invalid(
                    "yql_conflict",
                    Some("presentation"),
                    "YQL owns projection and highlighting",
                ));
            }
        }
    }
    validate_property_overlaps(request)
}

fn validate_property_overlaps(request: api::SearchRequest<'_>) -> Result<()> {
    let Some(properties) = request.properties() else {
        return Ok(());
    };
    let mut owned = BTreeSet::new();
    if request.ranking().is_some() {
        owned.insert("ranking.profile");
    }
    if request.limit().is_some() {
        owned.insert("hits");
    }
    if request.offset().is_some() {
        owned.insert("offset");
    }
    if request.timeout_ms().is_some() {
        owned.insert("timeout");
    }
    if request.query_model().is_some() {
        owned.insert("model");
    }
    if request.presentation().is_some() {
        owned.insert("presentation");
    }
    if request
        .query_profile()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("queryProfile");
    }
    if request
        .search_chain()
        .is_some_and(|value| !value.is_empty())
    {
        owned.insert("searchChain");
    }
    for (index, property) in properties.iter().enumerate() {
        let key = property.key();
        if owned
            .iter()
            .any(|prefix| key == *prefix || key.starts_with(&format!("{prefix}.")))
        {
            return Err(invalid(
                "property_overlaps_typed_field",
                Some(format!("properties[{index}].key")),
                format!("property `{key}` overlaps a populated typed request field"),
            ));
        }
    }
    Ok(())
}

fn grouping_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::GroupingRequest<'_>>>>,
) -> Result<Vec<GroupingRequestIr>> {
    let mut output = Vec::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let structured = match value.source_type() {
                api::GroupingSourceKind::StructuredGroupingSource => value.source_as_structured_grouping_source().ok_or_else(|| invalid("invalid_grouping", Some(format!("grouping[{index}].source")), "structured grouping source is missing"))?,
                api::GroupingSourceKind::RawGroupingSource => return Err(PortableError::unsupported("raw_grouping_not_supported", Some(format!("grouping[{index}].source")), "raw grouping text requires a grouping parser before it can enter canonical IR")),
                _ => return Err(invalid("invalid_grouping", Some(format!("grouping[{index}].source")), "grouping request must select a source")),
            };
            let root = structured.root().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(format!("grouping[{index}].source.root")),
                    "structured grouping requires an all() root",
                )
            })?;
            output.push(GroupingRequestIr {
                root: grouping_all_to_ir(root, &format!("grouping[{index}].source.root"))?,
                continuations: value
                    .continuations()
                    .map(|values| values.iter().map(ToOwned::to_owned).collect())
                    .unwrap_or_default(),
            });
        }
    }
    Ok(output)
}

fn grouping_all_to_ir(value: api::GroupingAllOperation<'_>, path: &str) -> Result<GroupingAllIr> {
    let operations = value
        .operations()
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    grouping_operation_to_ir(value, &format!("{path}.operations[{index}]"))
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(GroupingAllIr {
        operations,
        label: normalized_optional_string(value.label()),
    })
}

fn grouping_operation_to_ir(
    value: api::GroupingOperation<'_>,
    path: &str,
) -> Result<GroupingOperationIr> {
    match value.operation_type() {
        api::GroupingOperationKind::GroupingAllOperation => value
            .operation_as_grouping_all_operation()
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "all operation is missing"))
            .and_then(|value| grouping_all_to_ir(value, path))
            .map(GroupingOperationIr::All),
        api::GroupingOperationKind::GroupingEachOperation => {
            let value = value
                .operation_as_grouping_each_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "each operation is missing")
                })?;
            let operations = value
                .operations()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_operation_to_ir(value, &format!("{path}.operations[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Each(GroupingAllIr {
                operations,
                label: normalized_optional_string(value.label()),
            }))
        }
        api::GroupingOperationKind::GroupingGroupOperation => value
            .operation_as_grouping_group_operation()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "group operation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(GroupingOperationIr::Group),
        api::GroupingOperationKind::GroupingOutputOperation => {
            let value = value
                .operation_as_grouping_output_operation()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "output operation is missing",
                    )
                })?;
            let outputs = value
                .outputs()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, output)| {
                            Ok(GroupingOutputIr {
                                label: normalized_optional_string(output.label()),
                                expression: grouping_expression_to_ir(
                                    output.expression().ok_or_else(|| {
                                        invalid(
                                            "invalid_grouping",
                                            Some(format!("{path}.outputs[{index}].expression")),
                                            "output requires an expression",
                                        )
                                    })?,
                                    &format!("{path}.outputs[{index}].expression"),
                                )?,
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Output(outputs))
        }
        api::GroupingOperationKind::GroupingMaxOperation => {
            let value = value.operation_as_grouping_max_operation().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "max operation is missing")
            })?;
            let max = match value.value_type() {
                api::GroupingMaxKind::FiniteGroupingMax => Some(
                    value
                        .value_as_finite_grouping_max()
                        .ok_or_else(|| {
                            invalid("invalid_grouping", Some(path), "finite max is missing")
                        })?
                        .value(),
                ),
                api::GroupingMaxKind::InfiniteGroupingMax => None,
                _ => {
                    return Err(invalid(
                        "invalid_grouping",
                        Some(path),
                        "max requires a value",
                    ))
                }
            };
            Ok(GroupingOperationIr::Max(max))
        }
        api::GroupingOperationKind::GroupingPrecisionOperation => value
            .operation_as_grouping_precision_operation()
            .map(|value| GroupingOperationIr::Precision(value.value()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "precision operation is missing",
                )
            }),
        api::GroupingOperationKind::GroupingOrderOperation => {
            let value = value
                .operation_as_grouping_order_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "order operation is missing")
                })?;
            let expressions = value
                .expressions()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_expression_to_ir(
                                value.expression().ok_or_else(|| {
                                    invalid(
                                        "invalid_grouping",
                                        Some(format!("{path}.expressions[{index}].expression")),
                                        "order expression is missing",
                                    )
                                })?,
                                &format!("{path}.expressions[{index}]"),
                            )
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(GroupingOperationIr::Order(expressions))
        }
        api::GroupingOperationKind::GroupingAliasOperation => {
            let value = value
                .operation_as_grouping_alias_operation()
                .ok_or_else(|| {
                    invalid("invalid_grouping", Some(path), "alias operation is missing")
                })?;
            Ok(GroupingOperationIr::Alias {
                name: value
                    .name()
                    .ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "alias requires a name")
                    })?
                    .to_owned(),
                expression: grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "alias requires an expression",
                        )
                    })?,
                    path,
                )?,
            })
        }
        api::GroupingOperationKind::GroupingFilterOperation => value
            .operation_as_grouping_filter_operation()
            .and_then(|value| value.filter())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "filter operation requires a filter",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(GroupingOperationIr::Filter),
        api::GroupingOperationKind::GroupingKeepOperation => value
            .operation_as_grouping_keep_operation()
            .and_then(|value| value.filter())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "keep operation requires a filter",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(GroupingOperationIr::Keep),
        api::GroupingOperationKind::GroupingWhereOperation => value
            .operation_as_grouping_where_operation()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "where operation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(GroupingOperationIr::Where),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping operation must select a kind",
        )),
    }
}

fn grouping_expression_to_ir(
    value: api::GroupingExpression<'_>,
    path: &str,
) -> Result<GroupingExpressionIr> {
    match value.expr_type() {
        api::GroupingExpressionKind::GroupingAttributeExpression => value
            .expr_as_grouping_attribute_expression()
            .and_then(|value| value.field())
            .map(|value| GroupingExpressionIr::Attribute(value.to_owned()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "attribute expression requires a field",
                )
            }),
        api::GroupingExpressionKind::GroupingLiteralExpression => value
            .expr_as_grouping_literal_expression()
            .and_then(|value| value.value())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "literal expression requires a value",
                )
            })
            .and_then(|value| RequestValueIr::from_api(value, path.to_owned()))
            .map(GroupingExpressionIr::Literal),
        api::GroupingExpressionKind::GroupingFunctionExpression => {
            let value = value
                .expr_as_grouping_function_expression()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "function expression is missing",
                    )
                })?;
            let arguments = value
                .arguments()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            grouping_expression_to_ir(value, &format!("{path}.arguments[{index}]"))
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default();
            let name = value.name().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "function expression requires a name",
                )
            })?;
            if !supported_grouping_function(name) {
                return Err(PortableError::unsupported(
                    "unsupported_grouping_function",
                    Some(format!("{path}.name")),
                    format!(
                        "grouping function `{name}` is not supported by the portable grouping grammar"
                    ),
                ));
            }
            Ok(GroupingExpressionIr::Function {
                name: name.to_owned(),
                arguments,
            })
        }
        api::GroupingExpressionKind::GroupingUnaryExpression => {
            let value = value.expr_as_grouping_unary_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "unary expression is missing",
                )
            })?;
            let expression = Box::new(grouping_expression_to_ir(
                value.expression().ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "unary expression requires an operand",
                    )
                })?,
                path,
            )?);
            match value.op() {
                api::GroupingUnaryOperator::Positive => {
                    Ok(GroupingExpressionIr::Positive(expression))
                }
                api::GroupingUnaryOperator::Negative => {
                    Ok(GroupingExpressionIr::Negative(expression))
                }
                _ => Err(invalid(
                    "invalid_grouping",
                    Some(path),
                    "unary expression requires an operator",
                )),
            }
        }
        api::GroupingExpressionKind::GroupingBinaryExpression => {
            let value = value.expr_as_grouping_binary_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "binary expression is missing",
                )
            })?;
            let operator = match value.op() {
                api::GroupingBinaryOperator::Add => GroupingBinaryOperatorIr::Add,
                api::GroupingBinaryOperator::Subtract => GroupingBinaryOperatorIr::Subtract,
                api::GroupingBinaryOperator::Multiply => GroupingBinaryOperatorIr::Multiply,
                api::GroupingBinaryOperator::Divide => GroupingBinaryOperatorIr::Divide,
                api::GroupingBinaryOperator::Modulo => GroupingBinaryOperatorIr::Modulo,
                _ => {
                    return Err(invalid(
                        "invalid_grouping",
                        Some(path),
                        "binary expression requires an operator",
                    ))
                }
            };
            Ok(GroupingExpressionIr::Binary {
                operator,
                left: Box::new(grouping_expression_to_ir(
                    value.left().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "binary expression requires a left operand",
                        )
                    })?,
                    path,
                )?),
                right: Box::new(grouping_expression_to_ir(
                    value.right().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "binary expression requires a right operand",
                        )
                    })?,
                    path,
                )?),
            })
        }
        api::GroupingExpressionKind::GroupingAliasReference => value
            .expr_as_grouping_alias_reference()
            .and_then(|value| value.name())
            .map(|value| GroupingExpressionIr::Alias(value.to_owned()))
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "alias reference requires a name",
                )
            }),
        api::GroupingExpressionKind::GroupingMemberExpression => {
            let value = value.expr_as_grouping_member_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "member expression is missing",
                )
            })?;
            Ok(GroupingExpressionIr::Member {
                base: Box::new(grouping_expression_to_ir(
                    value.base().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "member expression requires a base",
                        )
                    })?,
                    path,
                )?),
                member: value
                    .member()
                    .ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "member expression requires a member",
                        )
                    })?
                    .to_owned(),
            })
        }
        api::GroupingExpressionKind::GroupingMapLookupExpression => {
            let value = value
                .expr_as_grouping_map_lookup_expression()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "map lookup expression is missing",
                    )
                })?;
            Ok(GroupingExpressionIr::MapLookup {
                base: Box::new(grouping_expression_to_ir(
                    value.base().ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "map lookup requires a base")
                    })?,
                    path,
                )?),
                key: Box::new(grouping_expression_to_ir(
                    value.key().ok_or_else(|| {
                        invalid("invalid_grouping", Some(path), "map lookup requires a key")
                    })?,
                    path,
                )?),
            })
        }
        api::GroupingExpressionKind::GroupingBucketExpression => {
            let value = value.expr_as_grouping_bucket_expression().ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "bucket expression is missing",
                )
            })?;
            Ok(GroupingExpressionIr::Bucket(GroupingBucketIr {
                lower: grouping_range_bound_to_ir(value.lower(), &format!("{path}.lower"))?,
                upper: grouping_range_bound_to_ir(value.upper(), &format!("{path}.upper"))?,
                include_lower: value.include_lower().is_none_or(|value| value.value()),
                include_upper: value.include_upper().is_some_and(|value| value.value()),
            }))
        }
        api::GroupingExpressionKind::GroupingAggregationExpression => value
            .expr_as_grouping_aggregation_expression()
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "aggregation expression is missing",
                )
            })
            .and_then(|value| grouping_aggregation_to_ir(value, path))
            .map(|value| GroupingExpressionIr::Aggregation(Box::new(value))),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping expression must select a kind",
        )),
    }
}

fn grouping_range_bound_to_ir(
    value: Option<api::RangeBound<'_>>,
    path: &str,
) -> Result<GroupingRangeBoundIr> {
    let value =
        value.ok_or_else(|| invalid("invalid_grouping", Some(path), "bucket bound is required"))?;
    match value.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => value
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "finite bucket bound requires a value",
                )
            })
            .and_then(|value| RequestValueIr::from_api(value, path.to_owned()))
            .map(GroupingRangeBoundIr::Finite),
        api::RangeBoundKind::NegativeInfinityBound => Ok(GroupingRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(GroupingRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(GroupingRangeBoundIr::Unbounded),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "unknown bucket bound",
        )),
    }
}

fn supported_grouping_function(name: &str) -> bool {
    matches!(
        name,
        "cat"
            | "md5"
            | "xor"
            | "fixedwidth"
            | "predefined"
            | "interpolatedlookup"
            | "reverse"
            | "normalize"
            | "strlen"
            | "lowercase"
            | "uppercase"
            | "now"
            | "relevance"
            | "array.at"
            | "map.at"
            | "math.sqrt"
            | "math.log"
            | "math.exp"
            | "math.floor"
            | "math.ceil"
            | "time.year"
            | "time.month"
            | "time.dayofmonth"
            | "time.dayofweek"
            | "time.hour"
            | "time.minute"
            | "time.second"
    )
}

fn grouping_aggregation_to_ir(
    value: api::GroupingAggregationExpression<'_>,
    path: &str,
) -> Result<GroupingAggregationIr> {
    let expression = |value: Option<api::GroupingExpression<'_>>| {
        value
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "aggregation requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
    };
    match value.aggregation_type() {
        api::AggregationKind::CountAggregation => value
            .aggregation_as_count_aggregation()
            .map(|_| GroupingAggregationIr::Count)
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "count aggregation is missing",
                )
            }),
        api::AggregationKind::SumAggregation => expression(
            value
                .aggregation_as_sum_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Sum),
        api::AggregationKind::AvgAggregation => expression(
            value
                .aggregation_as_avg_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Avg),
        api::AggregationKind::MinAggregation => expression(
            value
                .aggregation_as_min_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Min),
        api::AggregationKind::MaxAggregation => expression(
            value
                .aggregation_as_max_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Max),
        api::AggregationKind::XorAggregation => expression(
            value
                .aggregation_as_xor_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::Xor),
        api::AggregationKind::StdDevAggregation => expression(
            value
                .aggregation_as_std_dev_aggregation()
                .and_then(|value| value.expression()),
        )
        .map(GroupingAggregationIr::StdDev),
        api::AggregationKind::QuantilesAggregation => {
            let value = value
                .aggregation_as_quantiles_aggregation()
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "quantiles aggregation is missing",
                    )
                })?;
            let quantiles: Vec<f64> = value
                .quantiles()
                .map(|values| values.iter().collect())
                .unwrap_or_default();
            if quantiles
                .iter()
                .any(|value: &f64| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(invalid(
                    "invalid_quantile",
                    Some(format!("{path}.quantiles")),
                    "quantiles must be finite values in the inclusive range [0, 1]",
                ));
            }
            Ok(GroupingAggregationIr::Quantiles {
                quantiles,
                expression: expression(value.expression())?,
            })
        }
        api::AggregationKind::SummaryAggregation => Ok(GroupingAggregationIr::Summary(
            value
                .aggregation_as_summary_aggregation()
                .map(|value| normalized_optional_string(value.summary()))
                .ok_or_else(|| {
                    invalid(
                        "invalid_grouping",
                        Some(path),
                        "summary aggregation is missing",
                    )
                })?,
        )),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "aggregation must select a kind",
        )),
    }
}

fn grouping_filter_to_ir(
    value: api::GroupingFilterExpr<'_>,
    path: &str,
) -> Result<GroupingFilterIr> {
    match value.expr_type() {
        api::GroupingFilterExprKind::GroupingFilterAnd => value
            .expr_as_grouping_filter_and()
            .map(|value| {
                value
                    .clauses()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| grouping_filter_to_ir(value, path))
                            .collect()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()))
            })
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "filter-and is missing"))?
            .map(GroupingFilterIr::And),
        api::GroupingFilterExprKind::GroupingFilterOr => value
            .expr_as_grouping_filter_or()
            .map(|value| {
                value
                    .clauses()
                    .map(|values| {
                        values
                            .iter()
                            .map(|value| grouping_filter_to_ir(value, path))
                            .collect()
                    })
                    .unwrap_or_else(|| Ok(Vec::new()))
            })
            .ok_or_else(|| invalid("invalid_grouping", Some(path), "filter-or is missing"))?
            .map(GroupingFilterIr::Or),
        api::GroupingFilterExprKind::GroupingFilterNot => value
            .expr_as_grouping_filter_not()
            .and_then(|value| value.clause())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "filter-not requires a clause",
                )
            })
            .and_then(|value| grouping_filter_to_ir(value, path))
            .map(|value| GroupingFilterIr::Not(Box::new(value))),
        api::GroupingFilterExprKind::GroupingFilterRegex => {
            let value = value.expr_as_grouping_filter_regex().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "regex filter is missing")
            })?;
            Ok(GroupingFilterIr::Regex {
                expression: Box::new(grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "regex filter requires an expression",
                        )
                    })?,
                    path,
                )?),
                pattern: value
                    .pattern()
                    .ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "regex filter requires a pattern",
                        )
                    })?
                    .to_owned(),
            })
        }
        api::GroupingFilterExprKind::GroupingFilterRange => {
            let value = value.expr_as_grouping_filter_range().ok_or_else(|| {
                invalid("invalid_grouping", Some(path), "range filter is missing")
            })?;
            Ok(GroupingFilterIr::Range {
                expression: Box::new(grouping_expression_to_ir(
                    value.expression().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires an expression",
                        )
                    })?,
                    path,
                )?),
                lower: Box::new(grouping_expression_to_ir(
                    value.lower().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires a lower bound",
                        )
                    })?,
                    path,
                )?),
                upper: Box::new(grouping_expression_to_ir(
                    value.upper().ok_or_else(|| {
                        invalid(
                            "invalid_grouping",
                            Some(path),
                            "range filter requires an upper bound",
                        )
                    })?,
                    path,
                )?),
                include_lower: value.include_lower().is_none_or(|value| value.value()),
                include_upper: value.include_upper().is_some_and(|value| value.value()),
            })
        }
        api::GroupingFilterExprKind::GroupingFilterIsTrue => value
            .expr_as_grouping_filter_is_true()
            .and_then(|value| value.expression())
            .ok_or_else(|| {
                invalid(
                    "invalid_grouping",
                    Some(path),
                    "is-true filter requires an expression",
                )
            })
            .and_then(|value| grouping_expression_to_ir(value, path))
            .map(|value| GroupingFilterIr::IsTrue(Box::new(value))),
        _ => Err(invalid(
            "invalid_grouping",
            Some(path),
            "grouping filter must select a kind",
        )),
    }
}

impl RequestValueIr {
    fn from_api(value: api::Value<'_>, path: String) -> Result<Self> {
        match value.kind_type() {
            api::ValueKind::NullValueValue => Ok(Self::Null),
            api::ValueKind::BoolValue => value
                .kind_as_bool_value()
                .map(|value| Self::Bool(value.value()))
                .ok_or_else(|| invalid("missing_value", Some(path), "boolean value is missing")),
            api::ValueKind::Int64Value => value
                .kind_as_int_64_value()
                .map(|value| Self::Int64(value.value()))
                .ok_or_else(|| invalid("missing_value", Some(path), "integer value is missing")),
            api::ValueKind::UInt64Value => value
                .kind_as_uint_64_value()
                .map(|value| Self::UInt64(value.value()))
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path),
                        "unsigned integer value is missing",
                    )
                }),
            api::ValueKind::Float64Value => value
                .kind_as_float_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "float value is missing",
                    )
                })
                .and_then(|value| {
                    value
                        .value()
                        .is_finite()
                        .then_some(Self::Float64(value.value()))
                        .ok_or_else(|| {
                            invalid(
                                "non_finite_number",
                                Some(path),
                                "Float64Value must be finite",
                            )
                        })
                }),
            api::ValueKind::NonFiniteFloatValue => value
                .kind_as_non_finite_float_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "non-finite float value is missing",
                    )
                })
                .and_then(|value| match value.kind() {
                    api::NonFiniteFloatKind::NaN => {
                        Ok(Self::NonFiniteFloat(RequestNonFiniteFloatIr::NaN))
                    }
                    api::NonFiniteFloatKind::PositiveInfinity => Ok(Self::NonFiniteFloat(
                        RequestNonFiniteFloatIr::PositiveInfinity,
                    )),
                    api::NonFiniteFloatKind::NegativeInfinity => Ok(Self::NonFiniteFloat(
                        RequestNonFiniteFloatIr::NegativeInfinity,
                    )),
                    _ => Err(invalid(
                        "invalid_value",
                        Some(path),
                        "non-finite float kind is required",
                    )),
                }),
            api::ValueKind::StringValue => value
                .kind_as_string_value()
                .and_then(|value| value.value())
                .map(|value| Self::String(value.to_owned()))
                .ok_or_else(|| invalid("missing_value", Some(path), "string value is missing")),
            api::ValueKind::BytesValue => value
                .kind_as_bytes_value()
                .and_then(|value| value.value())
                .map(|value| Self::Bytes(value.iter().collect()))
                .ok_or_else(|| invalid("missing_value", Some(path), "bytes value is missing")),
            api::ValueKind::ValueList => {
                let values = value
                    .kind_as_value_list()
                    .ok_or_else(|| {
                        invalid("missing_value", Some(path.clone()), "value list is missing")
                    })?
                    .values()
                    .map(|values| {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, value)| {
                                Self::from_api(value, format!("{path}.values[{index}]"))
                            })
                            .collect()
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(Self::List(values))
            }
            api::ValueKind::ValueObject => {
                let values = value
                    .kind_as_value_object()
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(path.clone()),
                            "value object is missing",
                        )
                    })?
                    .fields()
                    .map(|values| {
                        values
                            .iter()
                            .enumerate()
                            .map(|(index, entry)| {
                                let key = entry.key().to_owned();
                                let value = entry.value().ok_or_else(|| {
                                    invalid(
                                        "missing_value",
                                        Some(format!("{path}.fields[{index}].value")),
                                        "object value is required",
                                    )
                                })?;
                                Ok((
                                    key,
                                    Self::from_api(value, format!("{path}.fields[{index}].value"))?,
                                ))
                            })
                            .collect::<Result<Vec<_>>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                let mut object = BTreeMap::new();
                for (key, value) in values {
                    if object.insert(key.clone(), value).is_some() {
                        return Err(invalid(
                            "duplicate_property",
                            Some(path),
                            format!("object field `{key}` occurs more than once"),
                        ));
                    }
                }
                Ok(Self::Object(object))
            }
            api::ValueKind::TensorValue => request_tensor_to_ir(
                value.kind_as_tensor_value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "tensor value is missing",
                    )
                })?,
                path,
            )
            .map(Self::Tensor),
            _ => Err(invalid(
                "missing_value",
                Some(path),
                "value must select a kind",
            )),
        }
    }
}

fn tensor_address_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::TensorAddressEntry<'_>>>,
    >,
    path: &str,
) -> Result<BTreeMap<String, String>> {
    let mut address = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let dimension = value.dimension().to_owned();
            let label = value
                .label()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}[{index}].label")),
                        "tensor address label is required",
                    )
                })?
                .to_owned();
            if address.insert(dimension.clone(), label).is_some() {
                return Err(invalid(
                    "duplicate_tensor_dimension",
                    Some(format!("{path}[{index}].dimension")),
                    format!("tensor dimension `{dimension}` occurs more than once"),
                ));
            }
        }
    }
    Ok(address)
}

fn request_tensor_to_ir(value: api::TensorValue<'_>, path: String) -> Result<RequestTensorIr> {
    let type_spec = value
        .type_spec()
        .ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.type_spec")),
                "tensor type is required",
            )
        })?
        .to_owned();
    let payload = match value.payload_type() {
        api::TensorPayloadKind::DenseTensorValues => RequestTensorPayloadIr::Dense(
            value
                .payload_as_dense_tensor_values()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "dense tensor payload is missing",
                    )
                })?
                .values()
                .map(|values| values.iter().collect())
                .unwrap_or_default(),
        ),
        api::TensorPayloadKind::SparseTensorCells => RequestTensorPayloadIr::Sparse(
            value
                .payload_as_sparse_tensor_cells()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "sparse tensor payload is missing",
                    )
                })?
                .cells()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(RequestTensorCellIr {
                                address: tensor_address_to_ir(
                                    value.address(),
                                    &format!("{path}.payload.cells[{index}].address"),
                                )?,
                                value: value.value(),
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default(),
        ),
        api::TensorPayloadKind::MixedTensorBlocks => RequestTensorPayloadIr::Mixed(
            value
                .payload_as_mixed_tensor_blocks()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.payload")),
                        "mixed tensor payload is missing",
                    )
                })?
                .blocks()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            Ok(RequestTensorBlockIr {
                                address: tensor_address_to_ir(
                                    value.address(),
                                    &format!("{path}.payload.blocks[{index}].address"),
                                )?,
                                values: value
                                    .values()
                                    .map(|values| values.iter().collect())
                                    .unwrap_or_default(),
                            })
                        })
                        .collect()
                })
                .transpose()?
                .unwrap_or_default(),
        ),
        _ => {
            return Err(invalid(
                "missing_value",
                Some(format!("{path}.payload")),
                "tensor payload is required",
            ))
        }
    };
    let finite = match &payload {
        RequestTensorPayloadIr::Dense(values) => values.iter().all(|value| value.is_finite()),
        RequestTensorPayloadIr::Sparse(cells) => cells.iter().all(|cell| cell.value.is_finite()),
        RequestTensorPayloadIr::Mixed(blocks) => blocks
            .iter()
            .all(|block| block.values.iter().all(|value| value.is_finite())),
    };
    if !finite {
        return Err(invalid(
            "non_finite_tensor_cell",
            Some(format!("{path}.payload")),
            "tensor cell values must be finite",
        ));
    }
    Ok(RequestTensorIr { type_spec, payload })
}

fn normalized_optional_string(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn ranking_request_to_ir(value: Option<api::RankingRequest<'_>>) -> Result<RankingRequestIr> {
    let Some(value) = value else {
        return Ok(RankingRequestIr {
            profile: "default".to_owned(),
            properties: BTreeMap::new(),
        });
    };
    let mut properties = BTreeMap::new();
    if let Some(entries) = value.properties() {
        for (index, entry) in entries.iter().enumerate() {
            let key = entry.key().to_owned();
            if properties
                .insert(key.clone(), entry.value().unwrap_or_default().to_owned())
                .is_some()
            {
                return Err(invalid(
                    "duplicate_ranking_property",
                    Some(format!("ranking.properties[{index}]")),
                    format!("ranking property `{key}` occurs more than once"),
                ));
            }
        }
    }
    Ok(RankingRequestIr {
        profile: value.profile().unwrap_or_default().to_owned(),
        properties,
    })
}

fn query_inputs_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::QueryInput<'_>>>>,
) -> Result<BTreeMap<String, QueryInputIr>> {
    let mut inputs = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let name = value.name().to_owned();
            let input = match value.input_type() {
                api::QueryInputKind::LiteralQueryInput => RequestValueIr::from_api(
                    value
                        .input_as_literal_query_input()
                        .and_then(|input| input.value())
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("inputs[{index}].input.value")),
                                "literal query input requires a value",
                            )
                        })?,
                    format!("inputs[{index}].input.value"),
                )
                .map(QueryInputIr::Literal)?,
                api::QueryInputKind::ExpressionQueryInput => QueryInputIr::Expression(
                    value
                        .input_as_expression_query_input()
                        .and_then(|input| input.expression())
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("inputs[{index}].input.expression")),
                                "expression query input requires an expression",
                            )
                        })?
                        .to_owned(),
                ),
                _ => {
                    return Err(invalid(
                        "invalid_query_input",
                        Some(format!("inputs[{index}].input")),
                        "query input must select a literal or expression",
                    ))
                }
            };
            if inputs.insert(name.clone(), input).is_some() {
                return Err(invalid(
                    "duplicate_query_input",
                    Some(format!("inputs[{index}].name")),
                    format!("query input `{name}` occurs more than once"),
                ));
            }
        }
    }
    Ok(inputs)
}

fn request_values_to_map(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, RequestValueIr>> {
    let mut output = BTreeMap::new();
    if let Some(values) = values {
        for (index, entry) in values.iter().enumerate() {
            let key = entry.key().to_owned();
            let value = entry.value().ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(format!("{path}[{index}].value")),
                    "value is required",
                )
            })?;
            if output
                .insert(
                    key.clone(),
                    RequestValueIr::from_api(value, format!("{path}[{index}].value"))?,
                )
                .is_some()
            {
                return Err(invalid(
                    "duplicate_property",
                    Some(format!("{path}[{index}].key")),
                    format!("property `{key}` occurs more than once"),
                ));
            }
        }
    }
    Ok(output)
}

fn query_model_to_ir(value: Option<api::QueryModel<'_>>) -> Result<QueryModelIr> {
    let Some(value) = value else {
        return Ok(QueryModelIr::default());
    };
    let query_type = value.type_().map(|value| QueryModelTypeIr {
        composite: normalized_optional_string(value.composite()),
        tokenization: normalized_optional_string(value.tokenization()),
        syntax: normalized_optional_string(value.syntax()),
        profile: normalized_optional_string(value.profile()),
        is_yql_default: value.is_yql_default().map(|value| value.value()),
    });
    Ok(QueryModelIr {
        query_string: normalized_optional_string(value.query_string()),
        default_index: normalized_optional_string(value.default_index()),
        encoding: normalized_optional_string(value.encoding()),
        filter: normalized_optional_string(value.filter()),
        language: value
            .language()
            .map(|value| parse_language(value, "query_model.language"))
            .transpose()?,
        locale: normalized_optional_string(value.locale()),
        query_type,
        sources: value
            .sources()
            .map(|values| values.iter().map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        restrict: value
            .restrict()
            .map(|values| values.iter().map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        search_path: normalized_optional_string(value.search_path()),
        search_group: value.search_group().map(|value| value.value()),
    })
}

fn presentation_to_ir(value: Option<api::PresentationRequest<'_>>) -> Result<PresentationIr> {
    let Some(value) = value else {
        return Ok(PresentationIr::default());
    };
    Ok(PresentationIr {
        summary: normalized_optional_string(value.summary()),
        format: normalized_optional_string(value.format()),
        bolding: value.bolding().map(|value| value.value()),
        timing: value.timing().map(|value| value.value()),
    })
}

fn condition_to_ir(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn field_to_ir_at(
    definition: api::FieldDefinition<'_>,
    name: FieldName,
    path: &str,
) -> Result<FieldIr> {
    let data_type = definition.data_type().ok_or_else(|| {
        invalid(
            "invalid_data_type",
            Some(format!("{path}.data_type")),
            "field data type is required",
        )
    })?;
    let data_type = data_type_to_ir(data_type, &format!("{path}.data_type"))?;
    let matching = matching_to_ir(definition.matching(), &format!("{path}.matching"))?;
    let index = index_config_to_ir(definition.index(), &format!("{path}.index"))?;
    let attribute = attribute_config_to_ir(definition.attribute(), &format!("{path}.attribute"))?;
    let summary = summary_config_to_ir(definition.summary(), &format!("{path}.summary"))?;
    let stored = definition.stored().map(|value| value.value());
    let searchable = definition.searchable().map(|value| value.value());
    let filterable = definition.filterable().map(|value| value.value());
    let sortable = definition.sortable().map(|value| value.value());
    let groupable = definition.facetable().map(|value| value.value());
    check_bool_agreement(
        stored,
        summary.as_ref().and_then(|value| value.enabled),
        path,
        "stored",
        "summary.enabled",
    )?;
    check_bool_agreement(
        searchable,
        index.as_ref().and_then(|value| value.enabled),
        path,
        "searchable",
        "index.enabled",
    )?;
    let string_type = data_type.scalar_primitive() == Some(PrimitiveDataTypeIr::String);
    if matching.is_some() && !string_type {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.matching")),
            "matching is valid only for string fields",
        ));
    }
    let analyzer = analyzer_config_to_ir(
        definition.analyzer(),
        string_type,
        &format!("{path}.analyzer"),
    )?;
    let capabilities = FieldCapabilities {
        stored: stored
            .or_else(|| summary.as_ref().and_then(|value| value.enabled))
            .unwrap_or(true),
        searchable: searchable
            .or_else(|| index.as_ref().and_then(|value| value.enabled))
            .unwrap_or(false),
        filterable: filterable.unwrap_or(false),
        sortable: sortable.unwrap_or(false),
        groupable: groupable.unwrap_or(false),
    };
    if data_type.is_multi_valued() && capabilities.sortable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.sortable")),
            "multi-valued fields cannot be sortable",
        ));
    }
    if index.as_ref().is_some_and(|config| config.hnsw.is_some())
        && !matches!(data_type, DataTypeIr::Tensor { .. })
    {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.index.hnsw")),
            "HNSW indexing requires a tensor field",
        ));
    }
    if attribute
        .as_ref()
        .is_some_and(|config| config.distance_metric != DistanceMetricIr::Unspecified)
        && !matches!(data_type, DataTypeIr::Tensor { .. })
    {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.attribute.distance_metric")),
            "a distance metric requires a tensor field",
        ));
    }
    let indexing_script = definition
        .indexing_script()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if indexing_script.is_some()
        && (stored.is_some()
            || searchable.is_some()
            || filterable.is_some()
            || sortable.is_some()
            || groupable.is_some()
            || analyzer.is_some()
            || matching.is_some()
            || index.is_some()
            || attribute.is_some()
            || summary.is_some())
    {
        return Err(invalid("ambiguous_indexing_script", Some(format!("{path}.indexing_script")), "an indexing script cannot be combined with typed field behavior that C2 cannot prove equivalent"));
    }
    Ok(FieldIr {
        name,
        data_type,
        analyzer,
        capabilities,
        matching,
        index,
        attribute,
        summary,
        indexing_script,
        properties: schema_properties_to_ir(
            definition.properties(),
            &format!("{path}.properties"),
        )?,
        struct_fields: struct_field_configs_to_ir(
            definition.struct_fields(),
            &format!("{path}.struct_fields"),
        )?,
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
    analyzer_identity(
        &AnalyzerIr {
            name: required_string(analyzer.name(), path, "analyzer name")?,
            version: analyzer.version(),
            options: schema_properties_to_ir(analyzer.options(), &format!("{path}.options"))?,
        },
        path,
    )
    .map(Some)
}

fn analyzer_config_to_ir(
    analyzer: Option<api::Analyzer<'_>>,
    string_type: bool,
    path: &str,
) -> Result<Option<AnalyzerIr>> {
    if !string_type && analyzer.is_some() {
        return Err(invalid(
            "invalid_analyzer",
            Some(path),
            "only string fields may specify an analyzer",
        ));
    }
    analyzer
        .map(|analyzer| {
            Ok(AnalyzerIr {
                name: required_string(analyzer.name(), path, "analyzer name")?,
                version: analyzer.version(),
                options: schema_properties_to_ir(analyzer.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

fn analyzer_identity(analyzer: &AnalyzerIr, path: &str) -> Result<AnalyzerIdentity> {
    if analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty() {
        Ok(AnalyzerIdentity::StandardV1)
    } else {
        Err(PortableError::unsupported("unsupported_analyzer", Some(path), "analysis execution currently supports only standard analyzer version 1 without options"))
    }
}

fn data_type_to_ir(value: api::DataType<'_>, path: &str) -> Result<DataTypeIr> {
    match value.kind_type() {
        api::DataTypeKind::PrimitiveDataType => {
            let primitive = value.kind_as_primitive_data_type().ok_or_else(|| {
                invalid(
                    "invalid_data_type",
                    Some(path),
                    "primitive data type is missing",
                )
            })?;
            let kind = match primitive.kind() {
                api::PrimitiveDataTypeKind::String => PrimitiveDataTypeIr::String,
                api::PrimitiveDataTypeKind::Bool => PrimitiveDataTypeIr::Bool,
                api::PrimitiveDataTypeKind::Byte => PrimitiveDataTypeIr::Byte,
                api::PrimitiveDataTypeKind::Int32 => PrimitiveDataTypeIr::Int32,
                api::PrimitiveDataTypeKind::Int64 => PrimitiveDataTypeIr::Int64,
                api::PrimitiveDataTypeKind::Float32 => PrimitiveDataTypeIr::Float32,
                api::PrimitiveDataTypeKind::Float64 => PrimitiveDataTypeIr::Float64,
                api::PrimitiveDataTypeKind::Raw => PrimitiveDataTypeIr::Raw,
                _ => {
                    return Err(invalid(
                        "invalid_data_type",
                        Some(path),
                        "primitive data type kind is required",
                    ))
                }
            };
            Ok(DataTypeIr::Primitive(kind))
        }
        api::DataTypeKind::TensorDataType => Ok(DataTypeIr::Tensor {
            type_spec: required_string(
                value.kind_as_tensor_data_type().and_then(|v| v.type_spec()),
                path,
                "tensor type spec",
            )?,
        }),
        api::DataTypeKind::ArrayDataType => Ok(DataTypeIr::Array(Box::new(data_type_to_ir(
            value
                .kind_as_array_data_type()
                .and_then(|v| v.element_type())
                .ok_or_else(|| {
                    invalid(
                        "invalid_data_type",
                        Some(path),
                        "array element type is required",
                    )
                })?,
            path,
        )?))),
        api::DataTypeKind::MapDataType => {
            let map = value.kind_as_map_data_type().ok_or_else(|| {
                invalid("invalid_data_type", Some(path), "map data type is missing")
            })?;
            Ok(DataTypeIr::Map {
                key_type: Box::new(data_type_to_ir(
                    map.key_type().ok_or_else(|| {
                        invalid("invalid_data_type", Some(path), "map key type is required")
                    })?,
                    path,
                )?),
                value_type: Box::new(data_type_to_ir(
                    map.value_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(path),
                            "map value type is required",
                        )
                    })?,
                    path,
                )?),
            })
        }
        api::DataTypeKind::WeightedSetDataType => {
            Ok(DataTypeIr::WeightedSet(Box::new(data_type_to_ir(
                value
                    .kind_as_weighted_set_data_type()
                    .and_then(|v| v.element_type())
                    .ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(path),
                            "weighted-set element type is required",
                        )
                    })?,
                path,
            )?)))
        }
        api::DataTypeKind::StructDataType => Ok(DataTypeIr::Struct {
            fields: struct_fields_to_ir(
                value.kind_as_struct_data_type().and_then(|v| v.fields()),
                &format!("{path}.fields"),
            )?,
        }),
        api::DataTypeKind::StructReferenceDataType => Ok(DataTypeIr::StructReference {
            name: required_string(
                value
                    .kind_as_struct_reference_data_type()
                    .and_then(|v| v.name()),
                path,
                "struct reference name",
            )?,
        }),
        api::DataTypeKind::PredicateDataType => Ok(DataTypeIr::Predicate),
        api::DataTypeKind::PositionDataType => Ok(DataTypeIr::Position),
        api::DataTypeKind::ReferenceDataType => Ok(DataTypeIr::Reference {
            document_type: required_string(
                value
                    .kind_as_reference_data_type()
                    .and_then(|v| v.document_type()),
                path,
                "reference document type",
            )?,
        }),
        _ => Err(invalid(
            "invalid_data_type",
            Some(path),
            "data type must select a kind",
        )),
    }
}

fn struct_fields_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StructFieldDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<StructFieldIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{item_path}.name"),
                "struct field name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{item_path}.name")),
                    "struct field names must be unique",
                ));
            }
            output.push(StructFieldIr {
                name,
                data_type: data_type_to_ir(
                    value.data_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(format!("{item_path}.data_type")),
                            "struct field data type is required",
                        )
                    })?,
                    &format!("{item_path}.data_type"),
                )?,
            });
        }
    }
    Ok(output)
}

fn matching_to_ir(
    value: Option<api::MatchingConfig<'_>>,
    path: &str,
) -> Result<Option<MatchingConfigIr>> {
    value
        .map(|value| {
            let mode = match value.mode() {
                api::MatchingMode::Unspecified => MatchingModeIr::Unspecified,
                api::MatchingMode::Text => MatchingModeIr::Text,
                api::MatchingMode::Word => MatchingModeIr::Word,
                api::MatchingMode::Exact => MatchingModeIr::Exact,
                api::MatchingMode::Gram => MatchingModeIr::Gram,
                _ => {
                    return Err(invalid(
                        "invalid_matching",
                        Some(path),
                        "unknown matching mode",
                    ))
                }
            };
            Ok(MatchingConfigIr {
                mode,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

fn index_config_to_ir(
    value: Option<api::IndexConfig<'_>>,
    path: &str,
) -> Result<Option<IndexConfigIr>> {
    value
        .map(|value| {
            Ok(IndexConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                hnsw: value
                    .hnsw()
                    .map(|hnsw| {
                        Ok(HnswIndexConfigIr {
                            max_links_per_node: hnsw
                                .max_links_per_node()
                                .map(|value| value.value()),
                            neighbors_to_explore_at_insert: hnsw
                                .neighbors_to_explore_at_insert()
                                .map(|value| value.value()),
                            options: schema_properties_to_ir(
                                hnsw.options(),
                                &format!("{path}.hnsw.options"),
                            )?,
                        })
                    })
                    .transpose()?,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

fn attribute_config_to_ir(
    value: Option<api::AttributeConfig<'_>>,
    path: &str,
) -> Result<Option<AttributeConfigIr>> {
    value
        .map(|value| {
            let distance_metric = match value.distance_metric() {
                api::DistanceMetric::Unspecified => DistanceMetricIr::Unspecified,
                api::DistanceMetric::Euclidean => DistanceMetricIr::Euclidean,
                api::DistanceMetric::Angular => DistanceMetricIr::Angular,
                api::DistanceMetric::DotProduct => DistanceMetricIr::DotProduct,
                api::DistanceMetric::PrenormalizedAngular => DistanceMetricIr::PrenormalizedAngular,
                api::DistanceMetric::GeoDegrees => DistanceMetricIr::GeoDegrees,
                api::DistanceMetric::Hamming => DistanceMetricIr::Hamming,
                _ => {
                    return Err(invalid(
                        "invalid_distance_metric",
                        Some(path),
                        "unknown distance metric",
                    ))
                }
            };
            Ok(AttributeConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                fast_search: value.fast_search().map(|value| value.value()),
                fast_access: value.fast_access().map(|value| value.value()),
                distance_metric,
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

fn summary_config_to_ir(
    value: Option<api::SummaryConfig<'_>>,
    path: &str,
) -> Result<Option<SummaryConfigIr>> {
    value
        .map(|value| {
            Ok(SummaryConfigIr {
                enabled: value.enabled().map(|value| value.value()),
                options: schema_properties_to_ir(value.options(), &format!("{path}.options"))?,
            })
        })
        .transpose()
}

fn schema_properties_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
    path: &str,
) -> Result<SchemaPropertiesIr> {
    request_values_to_map(values, path)
}

fn required_string(value: Option<&str>, path: &str, description: &str) -> Result<String> {
    value
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            invalid(
                "invalid_schema",
                Some(path),
                format!("{description} is required"),
            )
        })
}

fn optional_nonempty(value: Option<&str>, path: &str) -> Result<Option<String>> {
    value
        .map(|value| required_string(Some(value), path, "schema identifier"))
        .transpose()
}

fn strings_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<&str>>>,
    path: &str,
) -> Result<Vec<String>> {
    values
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    required_string(Some(value), &format!("{path}[{index}]"), "identifier")
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn check_bool_agreement(
    left: Option<bool>,
    right: Option<bool>,
    path: &str,
    left_name: &str,
    right_name: &str,
) -> Result<()> {
    if left.zip(right).is_some_and(|(left, right)| left != right) {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.{left_name}")),
            format!("{left_name} conflicts with {right_name}"),
        ));
    }
    Ok(())
}

fn struct_field_configs_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StructFieldConfig<'_>>>,
    >,
    path: &str,
) -> Result<Vec<StructFieldConfigIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let field_path = required_string(
                Some(value.path()),
                &format!("{item}.path"),
                "struct field path",
            )?;
            if !names.insert(field_path.clone()) {
                return Err(invalid(
                    "duplicate_struct_field_config",
                    Some(format!("{item}.path")),
                    "struct field paths must be unique",
                ));
            }
            output.push(StructFieldConfigIr {
                path: field_path,
                indexing_script: value
                    .indexing_script()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                matching: matching_to_ir(value.matching(), &format!("{item}.matching"))?,
                index: index_config_to_ir(value.index(), &format!("{item}.index"))?,
                attribute: attribute_config_to_ir(value.attribute(), &format!("{item}.attribute"))?,
                summary: summary_config_to_ir(value.summary(), &format!("{item}.summary"))?,
                rank: value
                    .rank()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                query_commands: strings_to_ir(
                    value.query_commands(),
                    &format!("{item}.query_commands"),
                )?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn named_structs_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::NamedStructDefinition<'_>>>,
    >,
) -> Result<Vec<NamedStructDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("structs[{index}]");
            let name = required_string(Some(value.name()), &format!("{path}.name"), "struct name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_struct",
                    Some(format!("{path}.name")),
                    "struct names must be unique",
                ));
            }
            output.push(NamedStructDefinitionIr {
                name,
                fields: struct_fields_to_ir(value.fields(), &format!("{path}.fields"))?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn validate_struct_references(
    fields: &[FieldIr],
    structs: &[NamedStructDefinitionIr],
) -> Result<()> {
    let names = structs
        .iter()
        .map(|value| value.name.as_str())
        .collect::<BTreeSet<_>>();
    fn visit(value: &DataTypeIr, names: &BTreeSet<&str>) -> bool {
        match value {
            DataTypeIr::StructReference { name } => names.contains(name.as_str()),
            DataTypeIr::Array(value) | DataTypeIr::WeightedSet(value) => visit(value, names),
            DataTypeIr::Map {
                key_type,
                value_type,
            } => visit(key_type, names) && visit(value_type, names),
            DataTypeIr::Struct { fields } => {
                fields.iter().all(|field| visit(&field.data_type, names))
            }
            _ => true,
        }
    }
    for field in fields {
        if !visit(&field.data_type, &names) {
            return Err(invalid(
                "unknown_struct",
                Some("fields"),
                "a struct reference is not declared by this schema",
            ));
        }
    }
    for value in structs {
        for field in &value.fields {
            if !visit(&field.data_type, &names) {
                return Err(invalid(
                    "unknown_struct",
                    Some("structs"),
                    "a struct reference is not declared by this schema",
                ));
            }
        }
    }
    Ok(())
}

fn schema_fields_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::SearchOnlyFieldDefinition<'_>>>,
    >,
    names: &BTreeSet<FieldName>,
) -> Result<Vec<FieldIr>> {
    let mut output = Vec::new();
    let mut all_names = names.clone();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("schema_fields[{index}].field");
            let field = value.field().ok_or_else(|| {
                invalid(
                    "invalid_schema",
                    Some(path.clone()),
                    "schema field is required",
                )
            })?;
            let name = parse_field_name(field.name(), format!("{path}.name"))?;
            if !all_names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_field",
                    Some(format!("{path}.name")),
                    "field names must be unique across document and schema fields",
                ));
            }
            output.push(field_to_ir_at(field, name, &path)?);
        }
    }
    Ok(output)
}

fn fieldsets_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::FieldSetDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<FieldSetDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("fieldsets[{index}]");
            let name =
                required_string(Some(value.name()), &format!("{path}.name"), "fieldset name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_fieldset",
                    Some(format!("{path}.name")),
                    "fieldset names must be unique",
                ));
            }
            let fields = strings_to_ir(value.fields(), &format!("{path}.fields"))?
                .into_iter()
                .enumerate()
                .map(|(field_index, field)| {
                    if !known.contains(field.as_str()) {
                        return Err(invalid(
                            "unknown_field",
                            Some(format!("{path}.fields[{field_index}]")),
                            "fieldset field is not defined",
                        ));
                    }
                    parse_field_name(&field, format!("{path}.fields[{field_index}]"))
                })
                .collect::<Result<Vec<_>>>()?;
            output.push(FieldSetDefinitionIr {
                name,
                fields,
                matching: matching_to_ir(value.matching(), &format!("{path}.matching"))?,
                query_commands: strings_to_ir(
                    value.query_commands(),
                    &format!("{path}.query_commands"),
                )?,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn imported_fields_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ImportedFieldDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<ImportedFieldDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut aliases = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("imported_fields[{index}]");
            let reference_name = required_string(
                value.reference_field(),
                &format!("{path}.reference_field"),
                "reference field",
            )?;
            let reference = parse_field_name(&reference_name, format!("{path}.reference_field"))?;
            let reference_definition = fields
                .iter()
                .find(|field| field.name == reference)
                .ok_or_else(|| {
                    invalid(
                        "unknown_field",
                        Some(format!("{path}.reference_field")),
                        "imported field reference is not defined",
                    )
                })?;
            if !matches!(reference_definition.data_type, DataTypeIr::Reference { .. }) {
                return Err(invalid(
                    "invalid_imported_field",
                    Some(format!("{path}.reference_field")),
                    "an imported field must reference a document-reference field",
                ));
            }
            let target_field = required_string(
                value.target_field(),
                &format!("{path}.target_field"),
                "imported target field",
            )?;
            let alias = parse_field_name(value.alias(), format!("{path}.alias"))?;
            if !aliases.insert(alias.clone()) || known.contains(alias.as_str()) {
                return Err(invalid(
                    "duplicate_imported_field",
                    Some(format!("{path}.alias")),
                    "imported aliases must not collide with schema fields",
                ));
            }
            output.push(ImportedFieldDefinitionIr {
                reference_field: reference,
                target_field,
                alias,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn document_summaries_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::DocumentSummaryDefinition<'_>>>,
    >,
    fields: &[&FieldIr],
) -> Result<Vec<DocumentSummaryDefinitionIr>> {
    let known = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("document_summaries[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{path}.name"),
                "document summary name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_document_summary",
                    Some(format!("{path}.name")),
                    "document summary names must be unique",
                ));
            }
            let mut summary_fields = Vec::new();
            let mut field_names = BTreeSet::new();
            if let Some(items) = value.fields() {
                for (field_index, item) in items.iter().enumerate() {
                    let item_path = format!("{path}.fields[{field_index}]");
                    let field_name = required_string(
                        Some(item.name()),
                        &format!("{item_path}.name"),
                        "summary field name",
                    )?;
                    if !field_names.insert(field_name.clone()) {
                        return Err(invalid(
                            "duplicate_summary_field",
                            Some(format!("{item_path}.name")),
                            "summary field names must be unique",
                        ));
                    }
                    let source = required_string(
                        item.source(),
                        &format!("{item_path}.source"),
                        "summary source",
                    )?;
                    if !known.contains(source.as_str()) {
                        return Err(invalid(
                            "unknown_field",
                            Some(format!("{item_path}.source")),
                            "summary source field is not defined",
                        ));
                    }
                    summary_fields.push(DocumentSummaryFieldDefinitionIr {
                        name: field_name,
                        source,
                        data_type: data_type_to_ir(
                            item.data_type().ok_or_else(|| {
                                invalid(
                                    "invalid_data_type",
                                    Some(format!("{item_path}.data_type")),
                                    "summary field data type is required",
                                )
                            })?,
                            &format!("{item_path}.data_type"),
                        )?,
                        struct_fields: strings_to_ir(
                            item.struct_fields(),
                            &format!("{item_path}.struct_fields"),
                        )?,
                        properties: schema_properties_to_ir(
                            item.properties(),
                            &format!("{item_path}.properties"),
                        )?,
                    });
                }
            }
            output.push(DocumentSummaryDefinitionIr {
                name,
                inherits: strings_to_ir(value.inherits(), &format!("{path}.inherits"))?,
                fields: summary_fields,
                from_disk: value.from_disk().map(|value| value.value()),
                omit_summary_features: value.omit_summary_features().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn constants_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ConstantDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<ConstantDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let name =
                required_string(Some(value.name()), &format!("{item}.name"), "constant name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_constant",
                    Some(format!("{item}.name")),
                    "constant names must be unique",
                ));
            }
            let source = match value.source_type() {
                api::ConstantSourceKind::LiteralConstantSource => {
                    ConstantSourceIr::Literal(RequestValueIr::from_api(
                        value
                            .source_as_literal_constant_source()
                            .and_then(|value| value.value())
                            .ok_or_else(|| {
                                invalid(
                                    "invalid_constant",
                                    Some(format!("{item}.source")),
                                    "literal constant value is required",
                                )
                            })?,
                        format!("{item}.source.value"),
                    )?)
                }
                api::ConstantSourceKind::FileConstantSource => ConstantSourceIr::File {
                    path: required_string(
                        value
                            .source_as_file_constant_source()
                            .and_then(|value| value.path()),
                        &format!("{item}.source.path"),
                        "constant file path",
                    )?,
                },
                _ => {
                    return Err(invalid(
                        "invalid_constant",
                        Some(format!("{item}.source")),
                        "constant source is required",
                    ))
                }
            };
            output.push(ConstantDefinitionIr {
                name,
                data_type: data_type_to_ir(
                    value.data_type().ok_or_else(|| {
                        invalid(
                            "invalid_data_type",
                            Some(format!("{item}.data_type")),
                            "constant data type is required",
                        )
                    })?,
                    &format!("{item}.data_type"),
                )?,
                source,
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn string_map_to_ir(
    values: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::StringEntry<'_>>>>,
    path: &str,
) -> Result<BTreeMap<String, String>> {
    let mut output = BTreeMap::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let key = required_string(
                Some(value.key()),
                &format!("{path}[{index}].key"),
                "map key",
            )?;
            let entry = required_string(
                value.value(),
                &format!("{path}[{index}].value"),
                "map value",
            )?;
            if output.insert(key.clone(), entry).is_some() {
                return Err(invalid(
                    "duplicate_property",
                    Some(format!("{path}[{index}].key")),
                    format!("duplicate key `{key}`"),
                ));
            }
        }
    }
    Ok(output)
}

fn onnx_models_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::OnnxModelDefinition<'_>>>,
    >,
    path: &str,
) -> Result<Vec<OnnxModelDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let item = format!("{path}[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{item}.name"),
                "ONNX model name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_onnx_model",
                    Some(format!("{item}.name")),
                    "ONNX model names must be unique",
                ));
            }
            output.push(OnnxModelDefinitionIr {
                name,
                file: required_string(value.file(), &format!("{item}.file"), "ONNX model file")?,
                inputs: string_map_to_ir(value.inputs(), &format!("{item}.inputs"))?,
                outputs: string_map_to_ir(value.outputs(), &format!("{item}.outputs"))?,
                gpu_device: value.gpu_device().map(|value| value.value()),
                intraop_threads: value.intraop_threads().map(|value| value.value()),
                interop_threads: value.interop_threads().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{item}.properties"),
                )?,
            });
        }
    }
    Ok(output)
}

fn rank_phase_to_ir(
    value: Option<api::RankPhaseDefinition<'_>>,
    path: &str,
) -> Result<Option<RankPhaseDefinitionIr>> {
    value
        .map(|value| {
            Ok(RankPhaseDefinitionIr {
                expression: value
                    .expression()
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                rerank_count: value.rerank_count().map(|value| value.value()),
                rank_score_drop_limit: value.rank_score_drop_limit().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
                total_rerank_count: value.total_rerank_count().map(|value| value.value()),
                keep_rank_count: value.keep_rank_count().map(|value| value.value()),
                total_keep_rank_count: value.total_keep_rank_count().map(|value| value.value()),
            })
        })
        .transpose()
}

fn rank_profiles_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::RankProfileDefinition<'_>>>,
    >,
) -> Result<Vec<RankProfileDefinitionIr>> {
    let mut output = Vec::new();
    let mut names = BTreeSet::new();
    if let Some(values) = values {
        for (index, value) in values.iter().enumerate() {
            let path = format!("rank_profiles[{index}]");
            let name = required_string(
                Some(value.name()),
                &format!("{path}.name"),
                "rank profile name",
            )?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    "duplicate_rank_profile",
                    Some(format!("{path}.name")),
                    "rank profile names must be unique",
                ));
            }
            let mut inputs = Vec::new();
            let mut input_names = BTreeSet::new();
            if let Some(items) = value.inputs() {
                for (input_index, input) in items.iter().enumerate() {
                    let item = format!("{path}.inputs[{input_index}]");
                    let input_name = required_string(
                        Some(input.name()),
                        &format!("{item}.name"),
                        "rank input name",
                    )?;
                    if !input_names.insert(input_name.clone()) {
                        return Err(invalid(
                            "duplicate_rank_input",
                            Some(format!("{item}.name")),
                            "rank input names must be unique",
                        ));
                    }
                    inputs.push(RankProfileInputIr {
                        name: input_name,
                        data_type: data_type_to_ir(
                            input.data_type().ok_or_else(|| {
                                invalid(
                                    "invalid_data_type",
                                    Some(format!("{item}.data_type")),
                                    "rank input data type is required",
                                )
                            })?,
                            &format!("{item}.data_type"),
                        )?,
                        default_value: RequestValueIr::from_api(
                            input.default_value().ok_or_else(|| {
                                invalid(
                                    "missing_value",
                                    Some(format!("{item}.default_value")),
                                    "rank input default value is required",
                                )
                            })?,
                            format!("{item}.default_value"),
                        )?,
                    });
                }
            }
            let mut functions = Vec::new();
            let mut function_names = BTreeSet::new();
            if let Some(items) = value.functions() {
                for (function_index, function) in items.iter().enumerate() {
                    let item = format!("{path}.functions[{function_index}]");
                    let function_name = required_string(
                        Some(function.name()),
                        &format!("{item}.name"),
                        "rank function name",
                    )?;
                    if !function_names.insert(function_name.clone()) {
                        return Err(invalid(
                            "duplicate_rank_function",
                            Some(format!("{item}.name")),
                            "rank function names must be unique",
                        ));
                    }
                    functions.push(RankFunctionDefinitionIr {
                        name: function_name,
                        arguments: strings_to_ir(
                            function.arguments(),
                            &format!("{item}.arguments"),
                        )?,
                        expression: required_string(
                            function.expression(),
                            &format!("{item}.expression"),
                            "rank function expression",
                        )?,
                        inline: function.inline().map(|value| value.value()),
                    });
                }
            }
            output.push(RankProfileDefinitionIr {
                name,
                inherits: strings_to_ir(value.inherits(), &format!("{path}.inherits"))?,
                inputs,
                functions,
                first_phase: rank_phase_to_ir(value.first_phase(), &format!("{path}.first_phase"))?,
                second_phase: rank_phase_to_ir(
                    value.second_phase(),
                    &format!("{path}.second_phase"),
                )?,
                global_phase: rank_phase_to_ir(
                    value.global_phase(),
                    &format!("{path}.global_phase"),
                )?,
                rank_features: strings_to_ir(
                    value.rank_features(),
                    &format!("{path}.rank_features"),
                )?,
                match_features: strings_to_ir(
                    value.match_features(),
                    &format!("{path}.match_features"),
                )?,
                summary_features: strings_to_ir(
                    value.summary_features(),
                    &format!("{path}.summary_features"),
                )?,
                sort_features: strings_to_ir(
                    value.sort_features(),
                    &format!("{path}.sort_features"),
                )?,
                constants: constants_to_ir(value.constants(), &format!("{path}.constants"))?,
                onnx_models: onnx_models_to_ir(
                    value.onnx_models(),
                    &format!("{path}.onnx_models"),
                )?,
                rank_properties: string_map_to_ir(
                    value.rank_properties(),
                    &format!("{path}.rank_properties"),
                )?,
                strict: value.strict().map(|value| value.value()),
                properties: schema_properties_to_ir(
                    value.properties(),
                    &format!("{path}.properties"),
                )?,
            });
        }
    }
    Ok(output)
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
            });
        }
    }
    Ok(search_fields)
}

fn decorate_filter_source_error(mut error: PortableError, clause_index: usize) -> PortableError {
    for detail in &mut error.details {
        detail
            .metadata
            .insert("clause_index".to_owned(), clause_index.to_string());
        // Parser and binder offsets are local to the source clause.
        if let Some(start) = detail.metadata.remove("span_start") {
            detail.metadata.insert("start_offset".to_owned(), start);
        }
        if let Some(end) = detail.metadata.remove("span_end") {
            detail.metadata.insert("end_offset".to_owned(), end);
        }
    }
    error
}
fn ranking_to_ir(r: Option<api::RankingRequest<'_>>) -> Result<RankingPolicyIr> {
    let Some(r) = r else {
        return Ok(RankingPolicyIr::DefaultRelevanceV1);
    };
    let profile = r.profile().ok_or_else(|| {
        invalid(
            "invalid_ranking",
            Some("ranking"),
            "ranking request must select a profile",
        )
    })?;
    match profile {
        "" | "default" | "default-relevance" => Ok(RankingPolicyIr::DefaultRelevanceV1),
        "bm25" => Ok(RankingPolicyIr::Bm25),
        // The concrete profile remains losslessly represented in
        // `RankingRequestIr`; this compatibility classification is used only
        // by the older native lowering path.
        _ => Ok(RankingPolicyIr::DefaultRelevanceV1),
    }
}

fn finite_range_bound<'a>(
    bound: Option<api::RangeBound<'a>>,
    path: &str,
) -> Result<Option<api::Value<'a>>> {
    let Some(bound) = bound else {
        return Ok(None);
    };
    match bound.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => bound
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .map(Some)
            .ok_or_else(|| invalid("invalid_range", Some(path), "finite range bound is missing")),
        api::RangeBoundKind::NegativeInfinityBound
        | api::RangeBoundKind::PositiveInfinityBound
        | api::RangeBoundKind::UnboundedRangeBound => Ok(None),
        _ => Err(invalid("invalid_range", Some(path), "unknown range bound")),
    }
}

fn filter_parameters(
    entries: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::ValueEntry<'_>>>>,
) -> Result<FilterParameters> {
    let mut parameters = BTreeMap::new();
    let Some(entries) = entries else {
        return Ok(parameters);
    };
    for (index, entry) in entries.iter().enumerate() {
        let name = entry.key().to_owned();
        if parameters.contains_key(&name) {
            return Err(invalid(
                "duplicate_parameter",
                Some(format!("filters.parameters[{index}]")),
                format!("parameter `:{name}` occurs more than once"),
            ));
        }
        let value = entry.value().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("filters.parameters[{index}].value")),
                "parameter value is required",
            )
        })?;
        parameters.insert(
            name,
            value_from_api(value, format!("filters.parameters[{index}].value"))?,
        );
    }
    Ok(parameters)
}

fn value_from_api(value: api::Value<'_>, path: String) -> Result<Value> {
    match value.kind_type() {
        api::ValueKind::NullValueValue => Ok(Value::Null),
        api::ValueKind::BoolValue => value
            .kind_as_bool_value()
            .map(|value| Value::Bool(value.value()))
            .ok_or_else(|| invalid("missing_value", Some(path), "boolean value is missing")),
        api::ValueKind::Int64Value => value
            .kind_as_int_64_value()
            .map(|value| Value::Int64(value.value()))
            .ok_or_else(|| invalid("missing_value", Some(path), "integer value is missing")),
        api::ValueKind::UInt64Value => value
            .kind_as_uint_64_value()
            .map(|value| Value::UInt64(value.value()))
            .ok_or_else(|| {
                invalid(
                    "missing_value",
                    Some(path),
                    "unsigned integer value is missing",
                )
            }),
        api::ValueKind::Float64Value => {
            let value = value
                .kind_as_float_64_value()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "float value is missing",
                    )
                })?
                .value();
            if value.is_finite() {
                Ok(Value::Float64(value))
            } else {
                Err(invalid(
                    "non_finite_number",
                    Some(path),
                    "floating-point parameters must be finite",
                ))
            }
        }
        api::ValueKind::StringValue => value
            .kind_as_string_value()
            .and_then(|value| value.value())
            .map(|value| Value::String(value.to_owned()))
            .ok_or_else(|| invalid("missing_value", Some(path), "string value is missing")),
        api::ValueKind::BytesValue => value
            .kind_as_bytes_value()
            .and_then(|value| value.value())
            .map(|value| Value::Bytes(value.iter().collect()))
            .ok_or_else(|| invalid("missing_value", Some(path), "bytes value is missing")),
        api::ValueKind::ValueList => {
            let values = value
                .kind_as_value_list()
                .ok_or_else(|| {
                    invalid("missing_value", Some(path.clone()), "value list is missing")
                })?
                .values()
                .map(|values| {
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            value_from_api(value, format!("{path}.values[{index}]"))
                        })
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?
                .unwrap_or_default();
            Ok(Value::List(values))
        }
        api::ValueKind::ValueObject => {
            let mut fields = BTreeMap::new();
            if let Some(entries) = value
                .kind_as_value_object()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(path.clone()),
                        "value object is missing",
                    )
                })?
                .fields()
            {
                for (index, entry) in entries.iter().enumerate() {
                    let name = entry.key().to_owned();
                    if fields.contains_key(&name) {
                        return Err(invalid(
                            "duplicate_parameter_field",
                            Some(format!("{path}.fields[{index}]")),
                            format!("field `{name}` occurs more than once"),
                        ));
                    }
                    let value = entry.value().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fields[{index}].value")),
                            "object value is missing",
                        )
                    })?;
                    fields.insert(
                        name,
                        value_from_api(value, format!("{path}.fields[{index}].value"))?,
                    );
                }
            }
            Ok(Value::Object(fields))
        }
        api::ValueKind::NONE => Err(invalid(
            "missing_value",
            Some(path),
            "parameter value must select a kind",
        )),
        _ => Err(PortableError::unsupported(
            "unsupported_parameter_value",
            Some(path),
            "parameter value kind is not supported by the embedded IR",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> IndexContext {
        let field = |name, kind| {
            let data_type = match kind {
                FieldKind::Text | FieldKind::Keyword => {
                    DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
                }
                FieldKind::Int64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
                FieldKind::Float64 => DataTypeIr::Primitive(PrimitiveDataTypeIr::Float64),
                FieldKind::Bool => DataTypeIr::Primitive(PrimitiveDataTypeIr::Bool),
            };
            let mut field = FieldIr::new(
                FieldName::parse(name).unwrap(),
                data_type,
                None,
                FieldCapabilities::new(false, false, true, false, false),
            );
            if kind == FieldKind::Keyword {
                field.matching = Some(MatchingConfigIr {
                    mode: MatchingModeIr::Exact,
                    options: BTreeMap::new(),
                });
            }
            field
        };
        let definition = IndexDefinitionIr::new(
            None,
            vec![
                field("brand", FieldKind::Keyword),
                field("price", FieldKind::Int64),
            ],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        let fields = definition
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| (field.name().as_str().to_owned(), index))
            .collect();
        IndexContext { definition, fields }
    }

    #[test]
    fn repeated_source_errors_identify_clause_and_local_offsets() {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let first = builder.create_string("brand = 'Acme'");
        let second = builder.create_string("price <= ");
        let sources = builder.create_vector(&[first, second]);
        let request = api::FilterRequest::create(
            &mut builder,
            &api::FilterRequestArgs {
                sources: Some(sources),
                ..Default::default()
            },
        );
        builder.finish(request, None);
        let request = crate::__flatbuffers::root::<api::FilterRequest>(builder.finished_data())
            .expect("valid filter request");

        let error = context().filters_to_ir(Some(request)).unwrap_err();
        let detail = &error.details[0];
        assert_eq!(detail.code, "syntax_error");
        assert_eq!(detail.metadata.get("clause_index"), Some(&"1".to_owned()));
        assert!(detail.metadata.contains_key("start_offset"));
        assert!(detail.metadata.contains_key("end_offset"));
        assert!(!detail.metadata.contains_key("span_start"));
        assert!(!detail.metadata.contains_key("span_end"));
    }
}
