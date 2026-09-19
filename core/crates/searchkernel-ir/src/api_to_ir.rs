//! FlatBuffers API to owned Search Kernel IR conversion.

use crate::error::{PortableError, Result};
use crate::filter_expression::{bind_filter_expression_with_references, FilterParameters};
use crate::model::{
    AnalyzeRequestIr, AnalyzerIdentity, AnalyzerIr, AttributeConfigIr, ComparisonIr, ComparisonOp,
    ConstantDefinitionIr, ConstantSourceIr, DataTypeIr, DeleteRequestIr, DistanceMetricIr,
    DocumentId, DocumentIr, DocumentSummaryDefinitionIr, DocumentSummaryFieldDefinitionIr,
    FieldCapabilities, FieldCardinality, FieldIr, FieldKind, FieldName, FieldSetDefinitionIr,
    FieldValueIr, FilterIr, FilterRangeBoundIr, GetRequestIr, GroupingAggregationIr, GroupingAllIr,
    GroupingBinaryOperatorIr, GroupingBucketIr, GroupingExpressionIr, GroupingFilterIr,
    GroupingOperationIr, GroupingOutputIr, GroupingRangeBoundIr, GroupingRequestIr, HighlightingIr,
    HnswIndexConfigIr, ImportedFieldDefinitionIr, IndexConfigIr, IndexDefinitionIr, IndexName,
    InsertRequestIr, LanguageHint, MatchingConfigIr, MatchingModeIr, MissingPlacement,
    NamedStructDefinitionIr, OnnxModelDefinitionIr, PatchOperationIr, PatchRequestIr, PrefixPolicy,
    PresentationIr, PrimitiveDataTypeIr, ProjectionIr, QueryAlternativeIr, QueryExprIr,
    QueryExprKindIr, QueryFieldRefIr, QueryInputIr, QueryIr, QueryModelIr, QueryModelTypeIr,
    QueryRangeBoundIr, RangeIr, RankFunctionDefinitionIr, RankPhaseDefinitionIr,
    RankProfileDefinitionIr, RankProfileInputIr, RankingPolicyIr, RankingRequestIr,
    RequestNonFiniteFloatIr, RequestTensorBlockIr, RequestTensorCellIr, RequestTensorIr,
    RequestTensorPayloadIr, RequestValueIr, ScalarValueIr, SchemaPropertiesIr, SearchFieldIr,
    SearchFieldWeight, SearchRequestIr, SortDirection, SortIr, SortTargetIr, StructFieldConfigIr,
    StructFieldIr, SummaryConfigIr, TypoPolicy, UpsertRequestIr, Value, WeightedQueryItemIr,
    WriteRequestIr,
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
    let document_field_refs = fields.iter().collect::<Vec<_>>();
    let searchable_fields = all_fields.clone();
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
    // Imported fields are fed from document/reference fields.  Schema-only
    // fields are query-visible, but are not document values and cannot be the
    // reference anchor for an imported alias.
    let imported_fields =
        imported_fields_to_ir(definition.imported_fields(), &document_field_refs)?;
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
    document_fields: BTreeMap<String, usize>,
    query_fields: BTreeMap<String, QueryFieldRef>,
}

#[derive(Clone, Copy, Debug)]
enum QueryFieldRef {
    Document(usize),
    Schema(usize),
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
        let document_fields = definition
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str().to_owned(), i))
            .collect();
        let mut query_fields = definition
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (f.name.as_str().to_owned(), QueryFieldRef::Document(i)))
            .collect::<BTreeMap<_, _>>();
        query_fields.extend(
            definition
                .schema_fields
                .iter()
                .enumerate()
                .map(|(i, f)| (f.name.as_str().to_owned(), QueryFieldRef::Schema(i))),
        );
        Ok(Self {
            definition,
            document_fields,
            query_fields,
        })
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
        let query_model = query_model_to_ir(request.query_model())?;
        let query =
            self.query_source_to_ir(request.query(), query_model.default_index.as_deref())?;
        let search_fields_wrapper = request.search_fields();
        validate_search_request_combinations(request, &query)?;
        let source_kind = request.query().map(|source| source.source_type());
        let search_fields = match (source_kind, search_fields_wrapper) {
            (Some(api::QuerySourceKind::SearchKernelTextQuery), None) => {
                self.definition.default_search_fields.clone()
            }
            (Some(api::QuerySourceKind::SearchKernelTextQuery), Some(selection)) => {
                self.search_fields_to_ir(selection)?
            }
            (_, None) => Vec::new(),
            (_, Some(_)) => {
                return Err(invalid(
                    "search_fields_not_supported",
                    Some("search_fields"),
                    "search fields apply only to text query sources",
                ));
            }
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
        let language = query_model.language.clone();
        let presentation = presentation_to_ir(request.presentation())?;
        let timeout = request
            .timeout_ms()
            .map(|value| Duration::from_millis(value.value()));
        let inputs = query_inputs_to_ir(request.inputs())?;
        validate_query_input_references(&query, &inputs, "query")?;
        Ok(SearchRequestIr {
            query,
            search_fields,
            filter,
            sort,
            grouping,
            ranking,
            ranking_request: ranking_request_to_ir(request.ranking())?,
            inputs,
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
                let field = self.query_field(
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
        if let Some(fields) = selection.fields() {
            for (index, field) in fields.iter().enumerate() {
                if self
                    .definition
                    .imported_alias(field.field().unwrap_or_default())
                {
                    return Err(PortableError::unsupported(
                        "imported_field_query_not_supported",
                        Some(format!("search_fields.fields[{index}].field")),
                        "imported field aliases are preserved in the schema but have no local type for query binding",
                    ));
                }
            }
        }
        let fields = self.definition.query_fields().collect::<Vec<_>>();
        search_fields_to_ir_at(selection, &fields, "search_fields")
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

    fn query_source_to_ir(
        &self,
        source: Option<api::QuerySource<'_>>,
        default_index: Option<&str>,
    ) -> Result<QueryIr> {
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
                self.query_expr_to_ir_with_default_index(query, "query", default_index)
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
        self.document_fields
            .get(name)
            .map(|&i| &self.definition.fields[i])
            .ok_or_else(|| invalid("unknown_field", Some(path), "field is not defined"))
    }

    fn query_field(&self, name: &str, path: String) -> Result<&FieldIr> {
        match self.query_fields.get(name).copied() {
            Some(QueryFieldRef::Document(index)) => Ok(&self.definition.fields[index]),
            Some(QueryFieldRef::Schema(index)) => Ok(&self.definition.schema_fields[index]),
            None if self.definition.imported_alias(name) => Err(PortableError::unsupported(
                "imported_field_query_not_supported",
                Some(path),
                "imported field aliases are preserved in the schema but have no local type for query binding",
            )),
            None => Err(invalid("unknown_field", Some(path), "field is not defined")),
        }
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
        self.query_expr_to_ir_with_default_index(expression, path, None)
    }

    fn query_expr_to_ir_with_default_index(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
        default_index: Option<&str>,
    ) -> Result<QueryIr> {
        let tag = expression.expr_type();
        match tag {
            api::QueryExprKind::TermQuery => {
                self.typed_query_to_ir(expression, path, default_index)
            }
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
                let field = self.query_field(
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
                let Some(term) = item.expr_as_term_query() else {
                    return self.typed_query_to_ir(expression, path, default_index);
                };
                if expression.annotations().is_some()
                    || item.annotations().is_some()
                    || contains
                        .field()
                        .and_then(|field| field.element_index())
                        .is_some()
                {
                    return self.typed_query_to_ir(expression, path, default_index);
                }
                let value = term.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.contains.item.value")),
                        "query term requires a value",
                    )
                })?;
                match scalar_to_ir(value, field.kind(), format!("{path}.contains.item.value")) {
                    Ok(value) => Ok(QueryIr::Term {
                        field: field.name.clone(),
                        value,
                    }),
                    Err(_) => self
                        .typed_query_expr_to_ir(expression, path)
                        .map(|query| QueryIr::Typed(Box::new(query))),
                }
            }
            api::QueryExprKind::QueryAnd => {
                let node = expression.expr_as_query_and().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "query expression must select an operation",
                    )
                })?;
                let clauses = query_children(node.clauses(), self, path, "all", default_index)?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.all")),
                        "query all requires at least one clause",
                    ));
                }
                if expression.annotations().is_some() || clauses.iter().any(query_ir_is_typed) {
                    return self.typed_query_to_ir(expression, path, default_index);
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
                let clauses = query_children(node.clauses(), self, path, "any", default_index)?;
                if clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.any")),
                        "query any requires at least one clause",
                    ));
                }
                if expression.annotations().is_some() || clauses.iter().any(query_ir_is_typed) {
                    return self.typed_query_to_ir(expression, path, default_index);
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
                let clause = self.query_expr_to_ir_with_default_index(
                    clause,
                    &format!("{path}.negation.clause"),
                    default_index,
                )?;
                if expression.annotations().is_some() || query_ir_is_typed(&clause) {
                    return self.typed_query_to_ir(expression, path, default_index);
                }
                Ok(QueryIr::Not(Box::new(clause)))
            }
            _ => self.typed_query_to_ir(expression, path, default_index),
        }
    }

    fn typed_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
        default_index: Option<&str>,
    ) -> Result<QueryIr> {
        let query = self.typed_query_expr_to_ir(expression, path)?;
        self.validate_typed_query(&query, None, default_index, path)?;
        Ok(QueryIr::Typed(Box::new(query)))
    }

    fn validate_typed_query(
        &self,
        query: &QueryExprIr,
        field_context: Option<&QueryFieldRefIr>,
        default_index: Option<&str>,
        path: &str,
    ) -> Result<()> {
        let require_field_context = |kind: &str| {
            if field_context.is_none() {
                Err(invalid(
                    "missing_field_context",
                    Some(path.to_owned()),
                    format!("{kind} query requires a containing field context"),
                ))
            } else {
                Ok(())
            }
        };
        let child_path = |suffix: &str| format!("{path}.{suffix}");
        let validate_children = |children: &[QueryExprIr], name: &str| {
            if children.is_empty() {
                return Err(invalid(
                    "empty_query_children",
                    Some(child_path(name)),
                    "query operation requires at least one child",
                ));
            }
            for (index, child) in children.iter().enumerate() {
                self.validate_typed_query(
                    child,
                    field_context,
                    default_index,
                    &format!("{path}.{name}[{index}]"),
                )?;
            }
            Ok(())
        };
        let validate_proximity_children = |children: &[QueryExprIr], name: &str| {
            if children.is_empty() {
                return Err(invalid(
                    "empty_query_children",
                    Some(child_path(name)),
                    "phrase/proximity operation requires at least one child",
                ));
            }
            for (index, child) in children.iter().enumerate() {
                if !matches!(
                    child.kind,
                    QueryExprKindIr::Term { .. }
                        | QueryExprKindIr::Contains { .. }
                        | QueryExprKindIr::Phrase(_)
                        | QueryExprKindIr::Near(_)
                        | QueryExprKindIr::ONear(_)
                        | QueryExprKindIr::SameElement(_)
                        | QueryExprKindIr::Equiv(_)
                        | QueryExprKindIr::Alternatives(_)
                        | QueryExprKindIr::Fuzzy { .. }
                        | QueryExprKindIr::Matches { .. }
                        | QueryExprKindIr::Text { .. }
                        | QueryExprKindIr::UserInput { .. }
                        | QueryExprKindIr::UserQuery
                ) {
                    return Err(invalid(
                        "invalid_query_child",
                        Some(format!("{path}.{name}[{index}]")),
                        "phrase/proximity children must be query terms or other term-like nodes",
                    ));
                }
                self.validate_typed_query(
                    child,
                    field_context,
                    default_index,
                    &format!("{path}.{name}[{index}]"),
                )?;
            }
            Ok(())
        };

        match &query.kind {
            QueryExprKindIr::Term { .. }
            | QueryExprKindIr::Fuzzy { .. }
            | QueryExprKindIr::Text { .. } => require_field_context("term-like"),
            QueryExprKindIr::UserInput { .. } => {
                if field_context.is_some()
                    || query.annotations.contains_key("defaultIndex")
                    || default_index.is_some()
                {
                    Ok(())
                } else {
                    require_field_context("user input")
                }
            }
            QueryExprKindIr::Contains { field, item } => self.validate_typed_query(
                item,
                Some(field),
                default_index,
                &child_path("contains.item"),
            ),
            QueryExprKindIr::And(children)
            | QueryExprKindIr::Or(children)
            | QueryExprKindIr::WeakAnd(children) => validate_children(
                children,
                match &query.kind {
                    QueryExprKindIr::And(_) => "and",
                    QueryExprKindIr::Or(_) => "or",
                    _ => "weak_and",
                },
            ),
            QueryExprKindIr::Not(child) | QueryExprKindIr::NonEmpty(child) => {
                self.validate_typed_query(child, field_context, default_index, &child_path("child"))
            }
            QueryExprKindIr::True | QueryExprKindIr::False | QueryExprKindIr::UserQuery => Ok(()),
            QueryExprKindIr::Comparison { .. }
            | QueryExprKindIr::Range { .. }
            | QueryExprKindIr::In { .. }
            | QueryExprKindIr::Matches { .. }
            | QueryExprKindIr::DotProduct { .. }
            | QueryExprKindIr::WeightedSet { .. }
            | QueryExprKindIr::Wand { .. }
            | QueryExprKindIr::GeoLocation { .. }
            | QueryExprKindIr::GeoBoundingBox { .. }
            | QueryExprKindIr::NearestNeighbor { .. } => Ok(()),
            QueryExprKindIr::Phrase(children)
            | QueryExprKindIr::Near(children)
            | QueryExprKindIr::ONear(children)
            | QueryExprKindIr::SameElement(children)
            | QueryExprKindIr::Equiv(children) => {
                require_field_context("phrase/proximity")?;
                validate_proximity_children(
                    children,
                    match &query.kind {
                        QueryExprKindIr::Phrase(_) => "phrase",
                        QueryExprKindIr::Near(_) => "near",
                        QueryExprKindIr::ONear(_) => "onear",
                        QueryExprKindIr::SameElement(_) => "same_element",
                        _ => "equiv",
                    },
                )
            }
            QueryExprKindIr::Alternatives(values) => {
                require_field_context("alternatives")?;
                if values.is_empty() {
                    return Err(invalid(
                        "empty_query_children",
                        Some(child_path("alternatives")),
                        "alternatives requires at least one query",
                    ));
                }
                for (index, value) in values.iter().enumerate() {
                    if value
                        .exactness
                        .is_some_and(|exactness| !(0.0..=1.0).contains(&exactness))
                    {
                        return Err(invalid(
                            "invalid_exactness",
                            Some(format!("{path}.alternatives[{index}].exactness")),
                            "alternative exactness must be between zero and one",
                        ));
                    }
                    self.validate_typed_query(
                        &value.query,
                        field_context,
                        default_index,
                        &format!("{path}.alternatives[{index}].query"),
                    )?;
                }
                Ok(())
            }
            QueryExprKindIr::Rank { matching, ranking } => {
                self.validate_typed_query(
                    matching,
                    field_context,
                    default_index,
                    &child_path("rank.match"),
                )?;
                validate_children(ranking, "rank.ranking")
            }
            QueryExprKindIr::Labeled { query, label, .. } => {
                if label.is_empty() {
                    return Err(invalid(
                        "missing_label",
                        Some(child_path("labeled.label")),
                        "labeled query requires a non-empty label",
                    ));
                }
                self.validate_typed_query(
                    query,
                    field_context,
                    default_index,
                    &child_path("labeled.query"),
                )
            }
            QueryExprKindIr::Predicate {
                features,
                range_features,
                ..
            } => {
                validate_predicate_values(features, &child_path("predicate.features"))?;
                validate_predicate_values(range_features, &child_path("predicate.range_features"))
            }
        }
    }

    fn typed_query_expr_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprIr> {
        let annotations =
            request_values_to_map(expression.annotations(), &format!("{path}.annotations"))?;
        let kind = match expression.expr_type() {
            api::QueryExprKind::TermQuery => {
                let value = expression
                    .expr_as_term_query()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.term.value")),
                            "term query requires a value",
                        )
                    })?;
                QueryExprKindIr::Term {
                    value: RequestValueIr::from_api(value, format!("{path}.term.value"))?,
                }
            }
            api::QueryExprKind::QueryContains => {
                let query = expression.expr_as_query_contains().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "contains query is missing")
                })?;
                let field =
                    self.query_field_ref_to_ir(query.field(), &format!("{path}.contains.field"))?;
                let item = query.item().ok_or_else(|| {
                    invalid(
                        "missing_query_item",
                        Some(format!("{path}.contains.item")),
                        "query contains requires an item",
                    )
                })?;
                QueryExprKindIr::Contains {
                    field,
                    item: Box::new(
                        self.typed_query_expr_to_ir(item, &format!("{path}.contains.item"))?,
                    ),
                }
            }
            api::QueryExprKind::QueryAnd => QueryExprKindIr::And(
                self.typed_query_children(
                    expression
                        .expr_as_query_and()
                        .and_then(|value| value.clauses()),
                    path,
                    "and",
                )?,
            ),
            api::QueryExprKind::QueryOr => QueryExprKindIr::Or(
                self.typed_query_children(
                    expression
                        .expr_as_query_or()
                        .and_then(|value| value.clauses()),
                    path,
                    "or",
                )?,
            ),
            api::QueryExprKind::QueryNot => {
                let query = expression
                    .expr_as_query_not()
                    .and_then(|value| value.clause())
                    .ok_or_else(|| {
                        invalid(
                            "missing_query",
                            Some(format!("{path}.not.clause")),
                            "not query requires a clause",
                        )
                    })?;
                QueryExprKindIr::Not(Box::new(
                    self.typed_query_expr_to_ir(query, &format!("{path}.not.clause"))?,
                ))
            }
            api::QueryExprKind::QueryTrue => QueryExprKindIr::True,
            api::QueryExprKind::QueryFalse => QueryExprKindIr::False,
            api::QueryExprKind::QueryComparison => self.comparison_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryRange => self.range_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryIn => {
                let query = expression
                    .expr_as_query_in()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "in query is missing"))?;
                QueryExprKindIr::In {
                    field: self
                        .query_field_ref_to_ir(query.field(), &format!("{path}.in.field"))?,
                    values: query
                        .values()
                        .map(|values| {
                            values
                                .iter()
                                .enumerate()
                                .map(|(index, value)| {
                                    RequestValueIr::from_api(
                                        value,
                                        format!("{path}.in.values[{index}]"),
                                    )
                                })
                                .collect::<Result<Vec<_>>>()
                        })
                        .transpose()?
                        .unwrap_or_default(),
                }
            }
            api::QueryExprKind::QueryPhrase => QueryExprKindIr::Phrase(
                self.typed_query_children(
                    expression
                        .expr_as_query_phrase()
                        .and_then(|value| value.terms()),
                    path,
                    "phrase",
                )?,
            ),
            api::QueryExprKind::QueryNear => QueryExprKindIr::Near(
                self.typed_query_children(
                    expression
                        .expr_as_query_near()
                        .and_then(|value| value.terms()),
                    path,
                    "near",
                )?,
            ),
            api::QueryExprKind::QueryONear => QueryExprKindIr::ONear(
                self.typed_query_children(
                    expression
                        .expr_as_query_onear()
                        .and_then(|value| value.terms()),
                    path,
                    "onear",
                )?,
            ),
            api::QueryExprKind::QuerySameElement => QueryExprKindIr::SameElement(
                self.typed_query_children(
                    expression
                        .expr_as_query_same_element()
                        .and_then(|value| value.clauses()),
                    path,
                    "same_element",
                )?,
            ),
            api::QueryExprKind::QueryEquiv => QueryExprKindIr::Equiv(
                self.typed_query_children(
                    expression
                        .expr_as_query_equiv()
                        .and_then(|value| value.clauses()),
                    path,
                    "equiv",
                )?,
            ),
            api::QueryExprKind::QueryAlternatives => {
                self.alternatives_query_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryFuzzy => {
                let value = expression
                    .expr_as_query_fuzzy()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fuzzy.value")),
                            "fuzzy query requires a value",
                        )
                    })?;
                QueryExprKindIr::Fuzzy {
                    value: RequestValueIr::from_api(value, format!("{path}.fuzzy.value"))?,
                }
            }
            api::QueryExprKind::QueryMatches => {
                let query = expression.expr_as_query_matches().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "matches query is missing")
                })?;
                QueryExprKindIr::Matches {
                    field: self
                        .query_field_ref_to_ir(query.field(), &format!("{path}.matches.field"))?,
                    pattern: query
                        .pattern()
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("{path}.matches.pattern")),
                                "matches query requires a pattern",
                            )
                        })?
                        .to_owned(),
                }
            }
            api::QueryExprKind::QueryText => QueryExprKindIr::Text {
                text: expression
                    .expr_as_query_text()
                    .and_then(|query| query.text())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.text.text")),
                            "text query requires text",
                        )
                    })?
                    .to_owned(),
            },
            api::QueryExprKind::QueryUserInput => QueryExprKindIr::UserInput {
                value: expression
                    .expr_as_query_user_input()
                    .and_then(|query| query.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.user_input.value")),
                            "user input query requires a value",
                        )
                    })?
                    .to_owned(),
            },
            api::QueryExprKind::QueryUserQuery => QueryExprKindIr::UserQuery,
            api::QueryExprKind::QueryDotProduct
            | api::QueryExprKind::QueryWeightedSet
            | api::QueryExprKind::QueryWand => self.weighted_query_to_ir(expression, path)?,
            api::QueryExprKind::QueryWeakAnd => QueryExprKindIr::WeakAnd(
                self.typed_query_children(
                    expression
                        .expr_as_query_weak_and()
                        .and_then(|value| value.clauses()),
                    path,
                    "weak_and",
                )?,
            ),
            api::QueryExprKind::QueryRank => {
                let query = expression
                    .expr_as_query_rank()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "rank query is missing"))?;
                QueryExprKindIr::Rank {
                    matching: Box::new(self.typed_query_expr_to_ir(
                        query.match_().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.rank.match")),
                                "rank query requires a match",
                            )
                        })?,
                        &format!("{path}.rank.match"),
                    )?),
                    ranking: self.typed_query_children(query.rank(), path, "rank")?,
                }
            }
            api::QueryExprKind::QueryLabeled => {
                let query = expression.expr_as_query_labeled().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "labeled query is missing")
                })?;
                let score = query.score();
                if !score.is_finite() {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.labeled.score")),
                        "labeled score must be finite",
                    ));
                }
                QueryExprKindIr::Labeled {
                    query: Box::new(self.typed_query_expr_to_ir(
                        query.query().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.labeled.query")),
                                "labeled query requires a query",
                            )
                        })?,
                        &format!("{path}.labeled.query"),
                    )?),
                    label: query.label().unwrap_or_default().to_owned(),
                    score,
                }
            }
            api::QueryExprKind::QueryNonEmpty => {
                let query = expression.expr_as_query_non_empty().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "non-empty query is missing")
                })?;
                QueryExprKindIr::NonEmpty(Box::new(self.typed_query_expr_to_ir(
                    query.query().ok_or_else(|| {
                        invalid(
                            "missing_query",
                            Some(format!("{path}.non_empty.query")),
                            "non-empty query requires a query",
                        )
                    })?,
                    &format!("{path}.non_empty.query"),
                )?))
            }
            api::QueryExprKind::QueryGeoLocation | api::QueryExprKind::QueryGeoBoundingBox => {
                self.geo_query_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryNearestNeighbor => {
                self.nearest_neighbor_to_ir(expression, path)?
            }
            api::QueryExprKind::QueryPredicate => self.predicate_query_to_ir(expression, path)?,
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "query expression must select an operation",
                ))
            }
        };
        Ok(QueryExprIr { kind, annotations })
    }

    fn comparison_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_comparison()
            .ok_or_else(|| invalid("invalid_query", Some(path), "comparison query is missing"))?;
        Ok(QueryExprKindIr::Comparison {
            field: self
                .query_field_ref_to_ir(query.field(), &format!("{path}.comparison.field"))?,
            op: query_comparison_op(query.op().0, &format!("{path}.comparison.op"))?,
            value: RequestValueIr::from_api(
                query.value().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.comparison.value")),
                        "comparison requires a value",
                    )
                })?,
                format!("{path}.comparison.value"),
            )?,
        })
    }

    fn range_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_range()
            .ok_or_else(|| invalid("invalid_query", Some(path), "range query is missing"))?;
        Ok(QueryExprKindIr::Range {
            field: self.query_field_ref_to_ir(query.field(), &format!("{path}.range.field"))?,
            lower: query_range_bound_to_ir(query.lower(), &format!("{path}.range.lower"))?,
            upper: query_range_bound_to_ir(query.upper(), &format!("{path}.range.upper"))?,
            include_lower: query.include_lower().map(|value| value.value()),
            include_upper: query.include_upper().map(|value| value.value()),
        })
    }

    fn alternatives_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let values = expression
            .expr_as_query_alternatives()
            .and_then(|value| value.alternatives())
            .map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let query = value.query().ok_or_else(|| {
                            invalid(
                                "missing_query",
                                Some(format!("{path}.alternatives[{index}].query")),
                                "alternative requires a query",
                            )
                        })?;
                        let exactness = value.exactness().map(|value| value.value());
                        if exactness.is_some_and(|value| !value.is_finite()) {
                            return Err(invalid(
                                "non_finite_number",
                                Some(format!("{path}.alternatives[{index}].exactness")),
                                "alternative exactness must be finite",
                            ));
                        }
                        Ok(QueryAlternativeIr {
                            query: self.typed_query_expr_to_ir(
                                query,
                                &format!("{path}.alternatives[{index}].query"),
                            )?,
                            exactness,
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(QueryExprKindIr::Alternatives(values))
    }

    fn weighted_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let (field, items) = match expression.expr_type() {
            api::QueryExprKind::QueryDotProduct => {
                let query = expression.expr_as_query_dot_product().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "dot product query is missing")
                })?;
                (query.field(), query.items())
            }
            api::QueryExprKind::QueryWeightedSet => {
                let query = expression.expr_as_query_weighted_set().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "weighted set query is missing")
                })?;
                (query.field(), query.items())
            }
            api::QueryExprKind::QueryWand => {
                let query = expression
                    .expr_as_query_wand()
                    .ok_or_else(|| invalid("invalid_query", Some(path), "wand query is missing"))?;
                (query.field(), query.items())
            }
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "weighted query must select an operation",
                ));
            }
        };
        let items = weighted_query_items_to_ir(items, path)?;
        let field = self.query_field_ref_to_ir(field, &format!("{path}.field"))?;
        Ok(match expression.expr_type() {
            api::QueryExprKind::QueryDotProduct => QueryExprKindIr::DotProduct { field, items },
            api::QueryExprKind::QueryWeightedSet => QueryExprKindIr::WeightedSet { field, items },
            api::QueryExprKind::QueryWand => QueryExprKindIr::Wand { field, items },
            _ => {
                return Err(invalid(
                    "invalid_query",
                    Some(path),
                    "weighted query must select an operation",
                ));
            }
        })
    }

    fn geo_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        match expression.expr_type() {
            api::QueryExprKind::QueryGeoLocation => {
                let query = expression.expr_as_query_geo_location().ok_or_else(|| {
                    invalid("invalid_query", Some(path), "geo location query is missing")
                })?;
                let values = [query.latitude(), query.longitude()];
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.geo_location")),
                        "geo coordinates must be finite",
                    ));
                }
                Ok(QueryExprKindIr::GeoLocation {
                    field: self.query_field_ref_to_ir(
                        query.field(),
                        &format!("{path}.geo_location.field"),
                    )?,
                    latitude: query.latitude(),
                    longitude: query.longitude(),
                    radius: query
                        .radius()
                        .ok_or_else(|| {
                            invalid(
                                "missing_value",
                                Some(format!("{path}.geo_location.radius")),
                                "geo location query requires a radius",
                            )
                        })?
                        .to_owned(),
                })
            }
            api::QueryExprKind::QueryGeoBoundingBox => {
                let query = expression.expr_as_query_geo_bounding_box().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(path),
                        "geo bounding box query is missing",
                    )
                })?;
                let values = [query.south(), query.west(), query.north(), query.east()];
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(invalid(
                        "non_finite_number",
                        Some(format!("{path}.geo_bounding_box")),
                        "geo coordinates must be finite",
                    ));
                }
                Ok(QueryExprKindIr::GeoBoundingBox {
                    field: self.query_field_ref_to_ir(
                        query.field(),
                        &format!("{path}.geo_bounding_box.field"),
                    )?,
                    south: query.south(),
                    west: query.west(),
                    north: query.north(),
                    east: query.east(),
                })
            }
            _ => Err(invalid(
                "invalid_query",
                Some(path),
                "geo query must select an operation",
            )),
        }
    }

    fn nearest_neighbor_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression.expr_as_query_nearest_neighbor().ok_or_else(|| {
            invalid(
                "invalid_query",
                Some(path),
                "nearest neighbor query is missing",
            )
        })?;
        Ok(QueryExprKindIr::NearestNeighbor {
            document_field: self.query_field_ref_to_ir(
                query.document_field(),
                &format!("{path}.nearest_neighbor.document_field"),
            )?,
            query_tensor: query
                .query_tensor()
                .ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.nearest_neighbor.query_tensor")),
                        "nearest-neighbor query requires a query tensor name",
                    )
                })?
                .to_owned(),
        })
    }

    fn predicate_query_to_ir(
        &self,
        expression: api::QueryExpr<'_>,
        path: &str,
    ) -> Result<QueryExprKindIr> {
        let query = expression
            .expr_as_query_predicate()
            .ok_or_else(|| invalid("invalid_query", Some(path), "predicate query is missing"))?;
        Ok(QueryExprKindIr::Predicate {
            field: self.query_field_ref_to_ir(query.field(), &format!("{path}.predicate.field"))?,
            features: query_value_object_to_ir(
                query.features(),
                &format!("{path}.predicate.features"),
            )?,
            range_features: query_value_object_to_ir(
                query.range_features(),
                &format!("{path}.predicate.range_features"),
            )?,
        })
    }

    fn query_field_ref_to_ir(
        &self,
        value: Option<api::QueryFieldRef<'_>>,
        path: &str,
    ) -> Result<QueryFieldRefIr> {
        let value = value
            .ok_or_else(|| invalid("missing_query_field", Some(path), "query field is required"))?;
        let field_path = value.path().unwrap_or_default();
        let field = self.query_field(field_path, format!("{path}.path"))?;
        Ok(QueryFieldRefIr {
            path: field.name.clone(),
            element_index: value.element_index().map(|value| value.value()),
        })
    }

    fn typed_query_children(
        &self,
        children: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::QueryExpr<'_>>>>,
        parent_path: &str,
        operation_name: &str,
    ) -> Result<Vec<QueryExprIr>> {
        children
            .map(|children| {
                children
                    .iter()
                    .enumerate()
                    .map(|(index, child)| {
                        self.typed_query_expr_to_ir(
                            child,
                            &format!("{parent_path}.{operation_name}.clauses[{index}]"),
                        )
                    })
                    .collect()
            })
            .transpose()
            .map(|value| value.unwrap_or_default())
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
                let f = self.query_field(
                    n.field().and_then(|field| field.path()).unwrap_or_default(),
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
        let f = self.query_field(
            c.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field"),
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
        let f = self.query_field(
            input
                .field()
                .and_then(|field| field.path())
                .unwrap_or_default(),
            format!("{path}.field"),
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
        let f = self.query_field(
            r.field().and_then(|field| field.path()).unwrap_or_default(),
            format!("{path}.field"),
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
        let lower = filter_range_bound_to_ir(r.lower(), f.kind(), &format!("{path}.lower"))?;
        let upper = filter_range_bound_to_ir(r.upper(), f.kind(), &format!("{path}.upper"))?;
        if matches!(lower, FilterRangeBoundIr::Unbounded)
            && matches!(upper, FilterRangeBoundIr::Unbounded)
        {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range requires at least one bound",
            ));
        }
        if matches!(lower, FilterRangeBoundIr::Unbounded)
            && r.include_lower().is_some_and(|v| v.value())
        {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_lower")),
                "lower inclusion requires a lower bound",
            ));
        }
        if matches!(upper, FilterRangeBoundIr::Unbounded)
            && r.include_upper().is_some_and(|v| v.value())
        {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_upper")),
                "upper inclusion requires an upper bound",
            ));
        }
        if filter_bound_greater(&lower, &upper) {
            return Err(invalid(
                "invalid_range",
                Some(path),
                "range lower bound must not exceed upper bound",
            ));
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
        let target = match target.target_type() {
            api::SortTargetKind::FieldSortTarget => {
                let field_path = format!("{path}.target.field");
                let f = self.query_field(
                    target
                        .target_as_field_sort_target()
                        .and_then(|target| target.field())
                        .unwrap_or_default(),
                    field_path.clone(),
                )?;
                if !f.capabilities.sortable {
                    return Err(invalid(
                        "field_not_sortable",
                        Some(field_path.clone()),
                        "sort requires a sortable field",
                    ));
                }
                if f.cardinality() != FieldCardinality::Single {
                    return Err(invalid(
                        "invalid_capability",
                        Some(field_path),
                        "multi-valued fields cannot be sortable",
                    ));
                }
                SortTargetIr::Field(f.name.clone())
            }
            api::SortTargetKind::RelevanceSortTarget => SortTargetIr::Relevance,
            api::SortTargetKind::RankFeatureSortTarget => {
                let feature = target
                    .target_as_rank_feature_sort_target()
                    .and_then(|target| target.feature())
                    .ok_or_else(|| {
                        invalid(
                            "invalid_sort",
                            Some(format!("{path}.target.feature")),
                            "rank-feature sort requires a feature name",
                        )
                    })?;
                if feature.is_empty() {
                    return Err(invalid(
                        "invalid_sort",
                        Some(format!("{path}.target.feature")),
                        "rank-feature sort requires a non-empty feature name",
                    ));
                }
                SortTargetIr::RankFeature(feature.to_owned())
            }
            api::SortTargetKind::RawSortTarget => {
                return Err(PortableError::unsupported(
                    "raw_sort_not_supported",
                    Some(format!("{path}.target")),
                    "raw sort expressions must be normalized before entering resolved IR",
                ));
            }
            _ => {
                return Err(invalid(
                    "invalid_sort",
                    Some(format!("{path}.target")),
                    "sort target must select a known variant",
                ));
            }
        };
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
                api::MissingPolicyKind::MissingAs => {
                    let value = v
                        .policy_as_missing_as()
                        .and_then(|value| value.value())
                        .ok_or_else(|| {
                            invalid(
                                "invalid_sort",
                                Some(format!("{path}.missing")),
                                "missing-as policy requires a value",
                            )
                        })?;
                    MissingPlacement::As(RequestValueIr::from_api(
                        value,
                        format!("{path}.missing.value"),
                    )?)
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
            target,
            direction,
            missing,
            annotations: request_values_to_map(s.annotations(), &format!("{path}.annotations"))?,
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
                if let Some(field) = v.field() {
                    if !seen.insert(field.clone()) {
                        return Err(invalid(
                            "duplicate_sort_field",
                            Some(format!("sort[{i}].field")),
                            "sort fields must be unique",
                        ));
                    }
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
                    .query_fields()
                    .filter(|f| f.capabilities.stored)
                    .map(|f| f.name.clone()),
            ),
            Some(p) => {
                if let Some(fs) = p.fields() {
                    let mut seen = BTreeSet::new();
                    for (i, n) in fs.iter().enumerate() {
                        let f = self.query_field(n, format!("projection.fields[{i}]"))?;
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
                let f = self.query_field(n, format!("highlighting.fields[{i}]"))?;
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

fn validate_query_input_references(
    query: &QueryIr,
    inputs: &BTreeMap<String, QueryInputIr>,
    path: &str,
) -> Result<()> {
    let QueryIr::Typed(expression) = query else {
        return Ok(());
    };
    validate_query_input_references_in_expr(expression, inputs, path)
}

fn validate_query_input_references_in_expr(
    expression: &QueryExprIr,
    inputs: &BTreeMap<String, QueryInputIr>,
    path: &str,
) -> Result<()> {
    let mut children: Vec<(&QueryExprIr, String)> = Vec::new();
    match &expression.kind {
        QueryExprKindIr::Contains { item, .. }
        | QueryExprKindIr::Not(item)
        | QueryExprKindIr::NonEmpty(item)
        | QueryExprKindIr::Labeled { query: item, .. } => {
            children.push((item, path.to_owned()));
        }
        QueryExprKindIr::And(values)
        | QueryExprKindIr::Or(values)
        | QueryExprKindIr::Phrase(values)
        | QueryExprKindIr::Near(values)
        | QueryExprKindIr::ONear(values)
        | QueryExprKindIr::SameElement(values)
        | QueryExprKindIr::Equiv(values)
        | QueryExprKindIr::WeakAnd(values) => {
            children.extend(values.iter().map(|value| (value, path.to_owned())));
        }
        QueryExprKindIr::Alternatives(values) => {
            children.extend(values.iter().map(|value| (&value.query, path.to_owned())));
        }
        QueryExprKindIr::Rank { matching, ranking } => {
            children.push((matching, path.to_owned()));
            children.extend(ranking.iter().map(|value| (value, path.to_owned())));
        }
        QueryExprKindIr::NearestNeighbor { query_tensor, .. }
            if !inputs.contains_key(query_tensor) =>
        {
            return Err(invalid(
                "unknown_query_input",
                Some(format!("{path}.nearest_neighbor.query_tensor")),
                format!("query tensor input {query_tensor} is not defined"),
            ));
        }
        _ => {}
    }
    for (child, child_path) in children {
        validate_query_input_references_in_expr(child, inputs, &child_path)?;
    }
    Ok(())
}

fn validate_predicate_values(values: &BTreeMap<String, RequestValueIr>, path: &str) -> Result<()> {
    for (key, value) in values {
        validate_predicate_value(value, &format!("{path}.{key}"))?;
    }
    Ok(())
}

fn validate_predicate_value(value: &RequestValueIr, path: &str) -> Result<()> {
    match value {
        RequestValueIr::Null
        | RequestValueIr::Bytes(_)
        | RequestValueIr::Tensor(_)
        | RequestValueIr::NonFiniteFloat(_) => Err(invalid(
            "invalid_predicate_value",
            Some(path.to_owned()),
            "predicate values must be finite scalar values, lists, or objects",
        )),
        RequestValueIr::List(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_predicate_value(value, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        RequestValueIr::Object(values) => validate_predicate_values(values, path),
        RequestValueIr::Bool(_)
        | RequestValueIr::Int64(_)
        | RequestValueIr::UInt64(_)
        | RequestValueIr::Float64(_)
        | RequestValueIr::String(_) => Ok(()),
    }
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
    let attribute_fast_search = attribute.as_ref().and_then(|value| value.fast_search);
    let indexing_script = definition
        .indexing_script()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
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
    if attribute_fast_search
        .zip(filterable)
        .is_some_and(|(fast_search, filterable)| fast_search && !filterable)
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.attribute.fast_search")),
            "attribute.fast_search requires a filterable field",
        ));
    }
    let string_type = data_type.scalar_primitive() == Some(PrimitiveDataTypeIr::String);
    if matching.is_some() && !string_type {
        return Err(invalid(
            "invalid_type_combination",
            Some(format!("{path}.matching")),
            "matching is valid only for string fields",
        ));
    }
    let keyword = string_type
        && matches!(
            matching.as_ref().map(|value| value.mode),
            Some(MatchingModeIr::Exact | MatchingModeIr::Word)
        );
    if keyword && searchable == Some(true) {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.searchable")),
            "keyword fields cannot opt into full-text search",
        ));
    }
    if matches!(
        data_type,
        DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
    ) && !keyword
        && filterable == Some(true)
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.filterable")),
            "text fields cannot opt into filtering",
        ));
    }
    let analyzer = analyzer_config_to_ir(
        definition.analyzer(),
        string_type && !keyword,
        &format!("{path}.analyzer"),
    )?;
    let default_searchable = string_type && !keyword && indexing_script.is_none();
    let default_filterable = if indexing_script.is_some() {
        false
    } else if string_type {
        keyword
    } else {
        true
    };
    let default_sortable = if indexing_script.is_some() {
        false
    } else {
        !data_type.is_multi_valued() && (!string_type || keyword)
    };
    let default_groupable = if indexing_script.is_some() {
        false
    } else if string_type {
        keyword
    } else {
        data_type.scalar_primitive() == Some(PrimitiveDataTypeIr::Bool)
    };
    let analyzer = if string_type && !keyword && indexing_script.is_none() {
        Some(analyzer.unwrap_or(AnalyzerIr {
            name: "standard".to_owned(),
            version: 1,
            options: SchemaPropertiesIr::new(),
        }))
    } else {
        analyzer
    };
    let capabilities = FieldCapabilities {
        stored: stored
            .or_else(|| summary.as_ref().and_then(|value| value.enabled))
            .unwrap_or(true),
        searchable: searchable
            .or_else(|| index.as_ref().and_then(|value| value.enabled))
            .unwrap_or(default_searchable),
        filterable: filterable
            .or(attribute_fast_search.filter(|value| *value))
            .unwrap_or(default_filterable),
        sortable: sortable.unwrap_or(default_sortable),
        groupable: groupable.unwrap_or(default_groupable),
    };
    if attribute
        .as_ref()
        .and_then(|value| value.enabled)
        .is_some_and(|enabled| !enabled)
        && (capabilities.filterable
            || capabilities.sortable
            || capabilities.groupable
            || !matches!(
                &data_type,
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String)
            ))
    {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.attribute.enabled")),
            "attribute.enabled=false conflicts with the resolved attribute-backed capabilities",
        ));
    }
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
            "only text fields may specify an analyzer",
        ));
    }
    analyzer
        .map(|analyzer| {
            let analyzer = AnalyzerIr {
                name: required_string(analyzer.name(), path, "analyzer name")?,
                version: analyzer.version(),
                options: schema_properties_to_ir(analyzer.options(), &format!("{path}.options"))?,
            };
            analyzer_identity(&analyzer, path)?;
            Ok(analyzer)
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

#[derive(Clone, Copy)]
enum RankPhaseKind {
    First,
    Second,
    Global,
}

fn validate_rank_phase(
    value: Option<&RankPhaseDefinitionIr>,
    kind: RankPhaseKind,
    path: &str,
) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    if value
        .rank_score_drop_limit
        .is_some_and(|limit| !limit.is_finite())
    {
        return Err(invalid(
            "invalid_rank_phase",
            Some(format!("{path}.rank_score_drop_limit")),
            "rank-score-drop-limit must be finite",
        ));
    }
    if value.rerank_count.is_some() && value.total_rerank_count.is_some() {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "rerank_count and total_rerank_count are mutually exclusive",
        ));
    }
    if value.keep_rank_count.is_some() && value.total_keep_rank_count.is_some() {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "keep_rank_count and total_keep_rank_count are mutually exclusive",
        ));
    }
    let invalid_for_phase = match kind {
        RankPhaseKind::First => value.rerank_count.is_some() || value.total_rerank_count.is_some(),
        RankPhaseKind::Second => {
            value.keep_rank_count.is_some() || value.total_keep_rank_count.is_some()
        }
        RankPhaseKind::Global => {
            value.keep_rank_count.is_some()
                || value.total_keep_rank_count.is_some()
                || value.total_rerank_count.is_some()
        }
    };
    if invalid_for_phase {
        return Err(invalid(
            "invalid_rank_phase",
            Some(path),
            "rank-phase controls are not applicable to this phase",
        ));
    }
    Ok(())
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
            let first_phase =
                rank_phase_to_ir(value.first_phase(), &format!("{path}.first_phase"))?;
            let second_phase =
                rank_phase_to_ir(value.second_phase(), &format!("{path}.second_phase"))?;
            let global_phase =
                rank_phase_to_ir(value.global_phase(), &format!("{path}.global_phase"))?;
            validate_rank_phase(
                first_phase.as_ref(),
                RankPhaseKind::First,
                &format!("{path}.first_phase"),
            )?;
            validate_rank_phase(
                second_phase.as_ref(),
                RankPhaseKind::Second,
                &format!("{path}.second_phase"),
            )?;
            validate_rank_phase(
                global_phase.as_ref(),
                RankPhaseKind::Global,
                &format!("{path}.global_phase"),
            )?;
            output.push(RankProfileDefinitionIr {
                name,
                inherits: strings_to_ir(value.inherits(), &format!("{path}.inherits"))?,
                inputs,
                functions,
                first_phase,
                second_phase,
                global_phase,
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
    fields: &[&FieldIr],
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
                .copied()
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

fn filter_range_bound_to_ir<'a>(
    bound: Option<api::RangeBound<'a>>,
    kind: FieldKind,
    path: &str,
) -> Result<FilterRangeBoundIr> {
    let Some(bound) = bound else {
        return Ok(FilterRangeBoundIr::Unbounded);
    };
    match bound.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => bound
            .bound_as_finite_range_bound()
            .and_then(|value| value.value())
            .ok_or_else(|| invalid("invalid_range", Some(path), "finite range bound is missing"))
            .and_then(|value| {
                RequestValueIr::from_api(value, path.to_owned())
                    .and_then(|value| scalar_request_value_to_ir(value, kind, path))
                    .map(FilterRangeBoundIr::Finite)
            }),
        api::RangeBoundKind::NegativeInfinityBound => Ok(FilterRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(FilterRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(FilterRangeBoundIr::Unbounded),
        _ => Err(invalid("invalid_range", Some(path), "unknown range bound")),
    }
}

fn scalar_request_value_to_ir(
    value: RequestValueIr,
    kind: FieldKind,
    path: &str,
) -> Result<ScalarValueIr> {
    let scalar = match value {
        RequestValueIr::Bool(value) => ScalarValueIr::Bool(value),
        RequestValueIr::Int64(value) => ScalarValueIr::Int64(value),
        RequestValueIr::Float64(value) if value.is_finite() => ScalarValueIr::Float64(value),
        RequestValueIr::String(value) => ScalarValueIr::String(value),
        _ => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "range bound must be a scalar value",
            ))
        }
    };
    if !matches!(
        (&scalar, kind),
        (ScalarValueIr::Int64(_), FieldKind::Int64)
            | (ScalarValueIr::Float64(_), FieldKind::Float64)
    ) {
        return Err(invalid(
            "type_mismatch",
            Some(path),
            "range bound type does not match field kind",
        ));
    }
    Ok(scalar)
}

fn filter_bound_greater(lower: &FilterRangeBoundIr, upper: &FilterRangeBoundIr) -> bool {
    match (lower, upper) {
        (FilterRangeBoundIr::Unbounded, _) => false,
        (_, FilterRangeBoundIr::Unbounded) => false,
        (FilterRangeBoundIr::PositiveInfinity, FilterRangeBoundIr::PositiveInfinity)
        | (FilterRangeBoundIr::NegativeInfinity, FilterRangeBoundIr::NegativeInfinity) => false,
        (FilterRangeBoundIr::PositiveInfinity, _) => true,
        (_, FilterRangeBoundIr::NegativeInfinity) => true,
        (FilterRangeBoundIr::Finite(a), FilterRangeBoundIr::Finite(b)) => numeric_gt(a, b),
        (FilterRangeBoundIr::NegativeInfinity, _) => false,
        (_, FilterRangeBoundIr::PositiveInfinity) => false,
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
    default_index: Option<&str>,
) -> Result<Vec<QueryIr>> {
    let mut converted = Vec::new();
    if let Some(children) = children {
        for (index, child) in children.iter().enumerate() {
            let child_path = format!("{parent_path}.{operation_name}.clauses[{index}]");
            converted.push(context.query_expr_to_ir_with_default_index(
                child,
                &child_path,
                default_index,
            )?);
        }
    }
    Ok(converted)
}

fn query_ir_is_typed(value: &QueryIr) -> bool {
    matches!(value, QueryIr::Typed(_))
}

fn query_comparison_op(value: u8, path: &str) -> Result<ComparisonOp> {
    match value {
        1 => Ok(ComparisonOp::Eq),
        2 => Ok(ComparisonOp::Ne),
        3 => Ok(ComparisonOp::Lt),
        4 => Ok(ComparisonOp::Lte),
        5 => Ok(ComparisonOp::Gt),
        6 => Ok(ComparisonOp::Gte),
        _ => Err(invalid(
            "invalid_query",
            Some(path),
            "comparison operator is unspecified or unknown",
        )),
    }
}

fn query_range_bound_to_ir(
    value: Option<api::RangeBound<'_>>,
    path: &str,
) -> Result<QueryRangeBoundIr> {
    let value = value
        .ok_or_else(|| invalid("missing_range_bound", Some(path), "range bound is required"))?;
    match value.bound_type() {
        api::RangeBoundKind::FiniteRangeBound => {
            Ok(QueryRangeBoundIr::Finite(RequestValueIr::from_api(
                value
                    .bound_as_finite_range_bound()
                    .and_then(|bound| bound.value())
                    .ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(path),
                            "finite range bound requires a value",
                        )
                    })?,
                format!("{path}.value"),
            )?))
        }
        api::RangeBoundKind::NegativeInfinityBound => Ok(QueryRangeBoundIr::NegativeInfinity),
        api::RangeBoundKind::PositiveInfinityBound => Ok(QueryRangeBoundIr::PositiveInfinity),
        api::RangeBoundKind::UnboundedRangeBound => Ok(QueryRangeBoundIr::Unbounded),
        _ => Err(invalid(
            "invalid_query",
            Some(path),
            "range bound must select a kind",
        )),
    }
}

fn weighted_query_items_to_ir(
    values: Option<
        flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<api::WeightedQueryItem<'_>>>,
    >,
    path: &str,
) -> Result<Vec<WeightedQueryItemIr>> {
    values
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    Ok(WeightedQueryItemIr {
                        key: RequestValueIr::from_api(
                            value.key().ok_or_else(|| {
                                invalid(
                                    "missing_value",
                                    Some(format!("{path}.items[{index}].key")),
                                    "weighted query item requires a key",
                                )
                            })?,
                            format!("{path}.items[{index}].key"),
                        )?,
                        weight: value.weight(),
                    })
                })
                .collect()
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn query_value_object_to_ir(
    value: Option<api::ValueObject<'_>>,
    path: &str,
) -> Result<BTreeMap<String, RequestValueIr>> {
    let mut output = BTreeMap::new();
    if let Some(value) = value {
        if let Some(fields) = value.fields() {
            for (index, entry) in fields.iter().enumerate() {
                let key = entry.key().to_owned();
                let value = RequestValueIr::from_api(
                    entry.value().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.fields[{index}].value")),
                            "predicate feature value is required",
                        )
                    })?,
                    format!("{path}.fields[{index}].value"),
                )?;
                if output.insert(key.clone(), value).is_some() {
                    return Err(invalid(
                        "duplicate_property",
                        Some(format!("{path}.fields[{index}].key")),
                        format!("predicate feature {key} occurs more than once"),
                    ));
                }
            }
        }
    }
    Ok(output)
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
        let document_fields = definition
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| (field.name().as_str().to_owned(), index))
            .collect();
        let query_fields = definition
            .fields()
            .iter()
            .enumerate()
            .map(|(index, field)| {
                (
                    field.name().as_str().to_owned(),
                    QueryFieldRef::Document(index),
                )
            })
            .collect();
        IndexContext {
            definition,
            document_fields,
            query_fields,
        }
    }

    #[test]
    fn schema_only_fields_are_query_visible_but_not_feedable() {
        let mut definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                FieldName::parse("title").unwrap(),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                None,
                FieldCapabilities::new(true, true, false, false, false),
            )],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        definition.set_schema_fields(vec![FieldIr::new(
            FieldName::parse("derived").unwrap(),
            DataTypeIr::Primitive(PrimitiveDataTypeIr::Int64),
            None,
            FieldCapabilities::new(true, true, true, true, false),
        )]);
        let bytes = crate::index_definition_ir_to_api(&definition);
        let context = IndexContext::from_api(
            crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
        )
        .unwrap();

        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let name = builder.create_string("derived");
        let weight = api::OptionalDouble::new(1.0);
        let search_field = api::SearchField::create(
            &mut builder,
            &api::SearchFieldArgs {
                field: Some(name),
                weight: Some(&weight),
            },
        );
        let fields = builder.create_vector(&[search_field]);
        let selection = api::SearchFieldSelection::create(
            &mut builder,
            &api::SearchFieldSelectionArgs {
                fields: Some(fields),
            },
        );
        builder.finish(selection, None);
        let selection =
            crate::__flatbuffers::root::<api::SearchFieldSelection>(builder.finished_data())
                .unwrap();
        assert_eq!(context.search_fields_to_ir(selection).unwrap().len(), 1);

        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 3 });
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::Int64Value,
                kind: Some(value.as_union_value()),
            },
        );
        builder.finish(value, None);
        let value = crate::__flatbuffers::root::<api::Value>(builder.finished_data()).unwrap();
        let error = context.value_to_ir("derived", value).unwrap_err();
        assert_eq!(error.details[0].code, "unknown_field");
    }

    #[test]
    fn imported_aliases_are_preserved_but_explicitly_not_query_bound() {
        let mut definition = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                FieldName::parse("parent").unwrap(),
                DataTypeIr::Reference {
                    document_type: "parent".to_owned(),
                },
                None,
                FieldCapabilities::new(false, false, false, false, false),
            )],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        definition.set_imported_fields(vec![ImportedFieldDefinitionIr {
            reference_field: FieldName::parse("parent").unwrap(),
            target_field: "name".to_owned(),
            alias: FieldName::parse("parent_name").unwrap(),
            properties: BTreeMap::new(),
        }]);
        let bytes = crate::index_definition_ir_to_api(&definition);
        let context = IndexContext::from_api(
            crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
        )
        .unwrap();
        let error = crate::bind_filter_expression(
            "parent_name = 'Acme'",
            context.definition(),
            &BTreeMap::new(),
        )
        .unwrap_err();
        assert_eq!(error.details[0].code, "imported_field_query_not_supported");

        let mut invalid = IndexDefinitionIr::new(
            None,
            vec![FieldIr::new(
                FieldName::parse("title").unwrap(),
                DataTypeIr::Primitive(PrimitiveDataTypeIr::String),
                None,
                FieldCapabilities::new(true, true, false, false, false),
            )],
            Vec::new(),
            RankingPolicyIr::DefaultRelevanceV1,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        )
        .unwrap();
        invalid.set_schema_fields(vec![FieldIr::new(
            FieldName::parse("derived_ref").unwrap(),
            DataTypeIr::Reference {
                document_type: "parent".to_owned(),
            },
            None,
            FieldCapabilities::new(false, false, false, false, false),
        )]);
        invalid.set_imported_fields(vec![ImportedFieldDefinitionIr {
            reference_field: FieldName::parse("derived_ref").unwrap(),
            target_field: "name".to_owned(),
            alias: FieldName::parse("derived_name").unwrap(),
            properties: BTreeMap::new(),
        }]);
        let bytes = crate::index_definition_ir_to_api(&invalid);
        let error = crate::index_definition_to_ir(
            crate::__flatbuffers::root::<api::IndexDefinition>(&bytes).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error.details[0].code, "unknown_field");
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

    #[test]
    fn v2_query_variant_malformed_payloads_are_rejected_explicitly() {
        fn bare_expression(kind: api::QueryExprKind) -> Vec<u8> {
            let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
            macro_rules! empty_payload {
                ($table:ident, $args:ident) => {
                    api::$table::create(&mut builder, &api::$args::default()).as_union_value()
                };
            }
            let payload = match kind {
                api::QueryExprKind::TermQuery => empty_payload!(TermQuery, TermQueryArgs),
                api::QueryExprKind::QueryContains => {
                    empty_payload!(QueryContains, QueryContainsArgs)
                }
                api::QueryExprKind::QueryAnd => empty_payload!(QueryAnd, QueryAndArgs),
                api::QueryExprKind::QueryOr => empty_payload!(QueryOr, QueryOrArgs),
                api::QueryExprKind::QueryNot => empty_payload!(QueryNot, QueryNotArgs),
                api::QueryExprKind::QueryTrue => empty_payload!(QueryTrue, QueryTrueArgs),
                api::QueryExprKind::QueryFalse => empty_payload!(QueryFalse, QueryFalseArgs),
                api::QueryExprKind::QueryComparison => {
                    empty_payload!(QueryComparison, QueryComparisonArgs)
                }
                api::QueryExprKind::QueryRange => empty_payload!(QueryRange, QueryRangeArgs),
                api::QueryExprKind::QueryIn => empty_payload!(QueryIn, QueryInArgs),
                api::QueryExprKind::QueryPhrase => empty_payload!(QueryPhrase, QueryPhraseArgs),
                api::QueryExprKind::QueryNear => empty_payload!(QueryNear, QueryNearArgs),
                api::QueryExprKind::QueryONear => empty_payload!(QueryONear, QueryONearArgs),
                api::QueryExprKind::QuerySameElement => {
                    empty_payload!(QuerySameElement, QuerySameElementArgs)
                }
                api::QueryExprKind::QueryEquiv => empty_payload!(QueryEquiv, QueryEquivArgs),
                api::QueryExprKind::QueryAlternatives => {
                    empty_payload!(QueryAlternatives, QueryAlternativesArgs)
                }
                api::QueryExprKind::QueryFuzzy => empty_payload!(QueryFuzzy, QueryFuzzyArgs),
                api::QueryExprKind::QueryMatches => empty_payload!(QueryMatches, QueryMatchesArgs),
                api::QueryExprKind::QueryText => empty_payload!(QueryText, QueryTextArgs),
                api::QueryExprKind::QueryUserInput => {
                    empty_payload!(QueryUserInput, QueryUserInputArgs)
                }
                api::QueryExprKind::QueryUserQuery => {
                    empty_payload!(QueryUserQuery, QueryUserQueryArgs)
                }
                api::QueryExprKind::QueryDotProduct => {
                    empty_payload!(QueryDotProduct, QueryDotProductArgs)
                }
                api::QueryExprKind::QueryWeightedSet => {
                    empty_payload!(QueryWeightedSet, QueryWeightedSetArgs)
                }
                api::QueryExprKind::QueryWand => empty_payload!(QueryWand, QueryWandArgs),
                api::QueryExprKind::QueryWeakAnd => empty_payload!(QueryWeakAnd, QueryWeakAndArgs),
                api::QueryExprKind::QueryRank => empty_payload!(QueryRank, QueryRankArgs),
                api::QueryExprKind::QueryLabeled => empty_payload!(QueryLabeled, QueryLabeledArgs),
                api::QueryExprKind::QueryNonEmpty => {
                    empty_payload!(QueryNonEmpty, QueryNonEmptyArgs)
                }
                api::QueryExprKind::QueryGeoLocation => {
                    empty_payload!(QueryGeoLocation, QueryGeoLocationArgs)
                }
                api::QueryExprKind::QueryGeoBoundingBox => {
                    empty_payload!(QueryGeoBoundingBox, QueryGeoBoundingBoxArgs)
                }
                api::QueryExprKind::QueryNearestNeighbor => {
                    empty_payload!(QueryNearestNeighbor, QueryNearestNeighborArgs)
                }
                api::QueryExprKind::QueryPredicate => {
                    empty_payload!(QueryPredicate, QueryPredicateArgs)
                }
                _ => unreachable!("unknown QueryExprKind in test fixture"),
            };
            let expression = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: kind,
                    expr: Some(payload),
                    annotations: None,
                },
            );
            builder.finish(expression, None);
            builder.finished_data().to_vec()
        }

        fn contains_expression() -> Vec<u8> {
            let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
            let field_name = builder.create_string("brand");
            let field = api::QueryFieldRef::create(
                &mut builder,
                &api::QueryFieldRefArgs {
                    path: Some(field_name),
                    element_index: None,
                },
            );
            let term_value = builder.create_string("Acme");
            let term_value = api::StringValue::create(
                &mut builder,
                &api::StringValueArgs {
                    value: Some(term_value),
                },
            );
            let term_value = api::Value::create(
                &mut builder,
                &api::ValueArgs {
                    kind_type: api::ValueKind::StringValue,
                    kind: Some(term_value.as_union_value()),
                },
            );
            let term = api::TermQuery::create(
                &mut builder,
                &api::TermQueryArgs {
                    value: Some(term_value),
                },
            );
            let item = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::TermQuery,
                    expr: Some(term.as_union_value()),
                    annotations: None,
                },
            );
            let contains = api::QueryContains::create(
                &mut builder,
                &api::QueryContainsArgs {
                    field: Some(field),
                    item: Some(item),
                },
            );
            let expression = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::QueryContains,
                    expr: Some(contains.as_union_value()),
                    annotations: None,
                },
            );
            builder.finish(expression, None);
            builder.finished_data().to_vec()
        }

        fn boolean_expression(kind: api::QueryExprKind) -> Vec<u8> {
            let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
            let child = {
                let field_name = builder.create_string("brand");
                let field = api::QueryFieldRef::create(
                    &mut builder,
                    &api::QueryFieldRefArgs {
                        path: Some(field_name),
                        element_index: None,
                    },
                );
                let value = builder.create_string("Acme");
                let value = api::StringValue::create(
                    &mut builder,
                    &api::StringValueArgs { value: Some(value) },
                );
                let value = api::Value::create(
                    &mut builder,
                    &api::ValueArgs {
                        kind_type: api::ValueKind::StringValue,
                        kind: Some(value.as_union_value()),
                    },
                );
                let term = api::TermQuery::create(
                    &mut builder,
                    &api::TermQueryArgs { value: Some(value) },
                );
                let item = api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: api::QueryExprKind::TermQuery,
                        expr: Some(term.as_union_value()),
                        annotations: None,
                    },
                );
                let contains = api::QueryContains::create(
                    &mut builder,
                    &api::QueryContainsArgs {
                        field: Some(field),
                        item: Some(item),
                    },
                );
                api::QueryExpr::create(
                    &mut builder,
                    &api::QueryExprArgs {
                        expr_type: api::QueryExprKind::QueryContains,
                        expr: Some(contains.as_union_value()),
                        annotations: None,
                    },
                )
            };
            let clauses = builder.create_vector(&[child]);
            let expression = match kind {
                api::QueryExprKind::QueryAnd => {
                    let value = api::QueryAnd::create(
                        &mut builder,
                        &api::QueryAndArgs {
                            clauses: Some(clauses),
                        },
                    );
                    api::QueryExpr::create(
                        &mut builder,
                        &api::QueryExprArgs {
                            expr_type: kind,
                            expr: Some(value.as_union_value()),
                            annotations: None,
                        },
                    )
                }
                api::QueryExprKind::QueryOr => {
                    let value = api::QueryOr::create(
                        &mut builder,
                        &api::QueryOrArgs {
                            clauses: Some(clauses),
                        },
                    );
                    api::QueryExpr::create(
                        &mut builder,
                        &api::QueryExprArgs {
                            expr_type: kind,
                            expr: Some(value.as_union_value()),
                            annotations: None,
                        },
                    )
                }
                api::QueryExprKind::QueryNot => {
                    let value = api::QueryNot::create(
                        &mut builder,
                        &api::QueryNotArgs {
                            clause: Some(child),
                        },
                    );
                    api::QueryExpr::create(
                        &mut builder,
                        &api::QueryExprArgs {
                            expr_type: kind,
                            expr: Some(value.as_union_value()),
                            annotations: None,
                        },
                    )
                }
                _ => unreachable!(),
            };
            builder.finish(expression, None);
            builder.finished_data().to_vec()
        }

        let mut query_context = context();
        query_context.definition.fields[0].capabilities.searchable = true;
        let supported = [
            ("QueryContains", contains_expression()),
            ("QueryAnd", boolean_expression(api::QueryExprKind::QueryAnd)),
            ("QueryOr", boolean_expression(api::QueryExprKind::QueryOr)),
            ("QueryNot", boolean_expression(api::QueryExprKind::QueryNot)),
        ];
        for (name, bytes) in &supported {
            let expression = crate::__flatbuffers::root::<api::QueryExpr>(bytes).unwrap();
            assert!(
                query_context.query_expr_to_ir(expression, "query").is_ok(),
                "{name} should be in the characterized supported subset"
            );
        }

        let unsupported = [
            ("TermQuery", api::QueryExprKind::TermQuery),
            ("QueryTrue", api::QueryExprKind::QueryTrue),
            ("QueryFalse", api::QueryExprKind::QueryFalse),
            ("QueryComparison", api::QueryExprKind::QueryComparison),
            ("QueryRange", api::QueryExprKind::QueryRange),
            ("QueryIn", api::QueryExprKind::QueryIn),
            ("QueryPhrase", api::QueryExprKind::QueryPhrase),
            ("QueryNear", api::QueryExprKind::QueryNear),
            ("QueryONear", api::QueryExprKind::QueryONear),
            ("QuerySameElement", api::QueryExprKind::QuerySameElement),
            ("QueryEquiv", api::QueryExprKind::QueryEquiv),
            ("QueryAlternatives", api::QueryExprKind::QueryAlternatives),
            ("QueryFuzzy", api::QueryExprKind::QueryFuzzy),
            ("QueryMatches", api::QueryExprKind::QueryMatches),
            ("QueryText", api::QueryExprKind::QueryText),
            ("QueryUserInput", api::QueryExprKind::QueryUserInput),
            ("QueryUserQuery", api::QueryExprKind::QueryUserQuery),
            ("QueryDotProduct", api::QueryExprKind::QueryDotProduct),
            ("QueryWeightedSet", api::QueryExprKind::QueryWeightedSet),
            ("QueryWand", api::QueryExprKind::QueryWand),
            ("QueryWeakAnd", api::QueryExprKind::QueryWeakAnd),
            ("QueryRank", api::QueryExprKind::QueryRank),
            ("QueryLabeled", api::QueryExprKind::QueryLabeled),
            ("QueryNonEmpty", api::QueryExprKind::QueryNonEmpty),
            ("QueryGeoLocation", api::QueryExprKind::QueryGeoLocation),
            (
                "QueryGeoBoundingBox",
                api::QueryExprKind::QueryGeoBoundingBox,
            ),
            (
                "QueryNearestNeighbor",
                api::QueryExprKind::QueryNearestNeighbor,
            ),
            ("QueryPredicate", api::QueryExprKind::QueryPredicate),
        ];
        assert_eq!(supported.len() + unsupported.len(), 32);
        for (name, kind) in unsupported {
            let bytes = bare_expression(kind);
            let expression = crate::__flatbuffers::root::<api::QueryExpr>(&bytes).unwrap();
            if let Err(error) = query_context.query_expr_to_ir(expression, "query") {
                assert!(
                    error
                        .details
                        .iter()
                        .all(|detail| detail.code != "unsupported_query_expression"),
                    "{name} must be rejected by validation, not an unsupported-variant fallback"
                );
            }
        }
    }

    #[test]
    fn every_v2_query_variant_round_trips_through_typed_ir() {
        fn field(name: &str) -> QueryFieldRefIr {
            QueryFieldRefIr {
                path: FieldName::parse(name).unwrap(),
                element_index: Some(2),
            }
        }
        fn leaf() -> QueryExprIr {
            QueryExprIr {
                kind: QueryExprKindIr::True,
                annotations: BTreeMap::new(),
            }
        }
        fn term() -> QueryExprIr {
            QueryExprIr {
                kind: QueryExprKindIr::Term {
                    value: RequestValueIr::String("Acme".to_owned()),
                },
                annotations: BTreeMap::new(),
            }
        }
        fn annotated(kind: QueryExprKindIr) -> QueryIr {
            let mut annotations = BTreeMap::new();
            annotations.insert("label".to_owned(), RequestValueIr::String("v2".to_owned()));
            QueryIr::Typed(Box::new(QueryExprIr { kind, annotations }))
        }
        fn request(query: QueryIr) -> SearchRequestIr {
            let mut request = SearchRequestIr::new(
                query,
                Vec::new(),
                None,
                Vec::new(),
                RankingPolicyIr::DefaultRelevanceV1,
                10,
                0,
                ProjectionIr::new(Vec::new()),
                HighlightingIr::Disabled,
                None,
                PrefixPolicy::Disabled,
                TypoPolicy::Disabled,
            );
            request.inputs.insert(
                "q".to_owned(),
                QueryInputIr::Literal(RequestValueIr::String("tensor".to_owned())),
            );
            request
        }

        let cases = vec![
            (
                "TermQuery",
                QueryExprKindIr::Term {
                    value: RequestValueIr::String("x".to_owned()),
                },
            ),
            (
                "QueryContains",
                QueryExprKindIr::Contains {
                    field: field("brand"),
                    item: Box::new(term()),
                },
            ),
            ("QueryAnd", QueryExprKindIr::And(vec![leaf()])),
            ("QueryOr", QueryExprKindIr::Or(vec![leaf()])),
            ("QueryNot", QueryExprKindIr::Not(Box::new(leaf()))),
            ("QueryTrue", QueryExprKindIr::True),
            ("QueryFalse", QueryExprKindIr::False),
            (
                "QueryComparison",
                QueryExprKindIr::Comparison {
                    field: field("price"),
                    op: ComparisonOp::Gt,
                    value: RequestValueIr::Int64(3),
                },
            ),
            (
                "QueryRange",
                QueryExprKindIr::Range {
                    field: field("price"),
                    lower: QueryRangeBoundIr::Finite(RequestValueIr::Int64(1)),
                    upper: QueryRangeBoundIr::PositiveInfinity,
                    include_lower: Some(true),
                    include_upper: Some(false),
                },
            ),
            (
                "QueryIn",
                QueryExprKindIr::In {
                    field: field("brand"),
                    values: vec![RequestValueIr::String("Acme".to_owned())],
                },
            ),
            ("QueryPhrase", QueryExprKindIr::Phrase(vec![term()])),
            ("QueryNear", QueryExprKindIr::Near(vec![term()])),
            ("QueryONear", QueryExprKindIr::ONear(vec![term()])),
            (
                "QuerySameElement",
                QueryExprKindIr::SameElement(vec![term()]),
            ),
            ("QueryEquiv", QueryExprKindIr::Equiv(vec![term()])),
            (
                "QueryAlternatives",
                QueryExprKindIr::Alternatives(vec![QueryAlternativeIr {
                    query: term(),
                    exactness: Some(0.75),
                }]),
            ),
            (
                "QueryFuzzy",
                QueryExprKindIr::Fuzzy {
                    value: RequestValueIr::String("acm".to_owned()),
                },
            ),
            (
                "QueryMatches",
                QueryExprKindIr::Matches {
                    field: field("brand"),
                    pattern: "A.*".to_owned(),
                },
            ),
            (
                "QueryText",
                QueryExprKindIr::Text {
                    text: "Acme".to_owned(),
                },
            ),
            (
                "QueryUserInput",
                QueryExprKindIr::UserInput {
                    value: "query".to_owned(),
                },
            ),
            ("QueryUserQuery", QueryExprKindIr::UserQuery),
            (
                "QueryDotProduct",
                QueryExprKindIr::DotProduct {
                    field: field("price"),
                    items: vec![WeightedQueryItemIr {
                        key: RequestValueIr::String("x".to_owned()),
                        weight: 2,
                    }],
                },
            ),
            (
                "QueryWeightedSet",
                QueryExprKindIr::WeightedSet {
                    field: field("brand"),
                    items: vec![WeightedQueryItemIr {
                        key: RequestValueIr::String("x".to_owned()),
                        weight: 2,
                    }],
                },
            ),
            (
                "QueryWand",
                QueryExprKindIr::Wand {
                    field: field("brand"),
                    items: vec![WeightedQueryItemIr {
                        key: RequestValueIr::String("x".to_owned()),
                        weight: 2,
                    }],
                },
            ),
            ("QueryWeakAnd", QueryExprKindIr::WeakAnd(vec![term()])),
            (
                "QueryRank",
                QueryExprKindIr::Rank {
                    matching: Box::new(term()),
                    ranking: vec![leaf()],
                },
            ),
            (
                "QueryLabeled",
                QueryExprKindIr::Labeled {
                    query: Box::new(term()),
                    label: "my_label".to_owned(),
                    score: 1.25,
                },
            ),
            ("QueryNonEmpty", QueryExprKindIr::NonEmpty(Box::new(term()))),
            (
                "QueryGeoLocation",
                QueryExprKindIr::GeoLocation {
                    field: field("brand"),
                    latitude: 59.9,
                    longitude: 10.7,
                    radius: "10 km".to_owned(),
                },
            ),
            (
                "QueryGeoBoundingBox",
                QueryExprKindIr::GeoBoundingBox {
                    field: field("brand"),
                    south: 59.0,
                    west: 10.0,
                    north: 60.0,
                    east: 11.0,
                },
            ),
            (
                "QueryNearestNeighbor",
                QueryExprKindIr::NearestNeighbor {
                    document_field: field("brand"),
                    query_tensor: "q".to_owned(),
                },
            ),
            (
                "QueryPredicate",
                QueryExprKindIr::Predicate {
                    field: field("brand"),
                    features: BTreeMap::from([(
                        String::from("hobby"),
                        RequestValueIr::String("ski".to_owned()),
                    )]),
                    range_features: BTreeMap::new(),
                },
            ),
        ];
        assert_eq!(cases.len(), 32);

        let mut query_context = context();
        query_context.definition.fields[0].capabilities.searchable = true;
        for (name, kind) in cases {
            let expected = annotated(kind);
            let expected = if matches!(
                name,
                "TermQuery"
                    | "QueryPhrase"
                    | "QueryNear"
                    | "QueryONear"
                    | "QuerySameElement"
                    | "QueryEquiv"
                    | "QueryAlternatives"
                    | "QueryFuzzy"
                    | "QueryText"
                    | "QueryUserInput"
                    | "QueryWeakAnd"
                    | "QueryRank"
                    | "QueryLabeled"
                    | "QueryNonEmpty"
            ) {
                let QueryIr::Typed(inner) = expected else {
                    unreachable!()
                };
                QueryIr::Typed(Box::new(QueryExprIr {
                    kind: QueryExprKindIr::Contains {
                        field: field("brand"),
                        item: inner,
                    },
                    annotations: BTreeMap::new(),
                }))
            } else {
                expected
            };
            let bytes = crate::search_request_ir_to_api(&request(expected.clone()));
            let api_request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
            let actual = query_context.search_to_ir(api_request).unwrap();
            assert_eq!(
                actual.query(),
                &expected,
                "{name} lost typed query semantics"
            );
        }
    }

    #[test]
    fn query_user_input_accepts_annotation_or_query_model_default_index() {
        fn request_bytes(annotation: bool, query_model: bool) -> Vec<u8> {
            let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
            let input = builder.create_string("free form input");
            let input = api::QueryUserInput::create(
                &mut builder,
                &api::QueryUserInputArgs { value: Some(input) },
            );
            let annotations = if annotation {
                let key = builder.create_string("defaultIndex");
                let value = builder.create_string("title");
                let value = api::StringValue::create(
                    &mut builder,
                    &api::StringValueArgs { value: Some(value) },
                );
                let value = api::Value::create(
                    &mut builder,
                    &api::ValueArgs {
                        kind_type: api::ValueKind::StringValue,
                        kind: Some(value.as_union_value()),
                    },
                );
                let entry = api::ValueEntry::create(
                    &mut builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                );
                Some(builder.create_vector(&[entry]))
            } else {
                None
            };
            let expression = api::QueryExpr::create(
                &mut builder,
                &api::QueryExprArgs {
                    expr_type: api::QueryExprKind::QueryUserInput,
                    expr: Some(input.as_union_value()),
                    annotations,
                },
            );
            let typed = api::TypedQuery::create(
                &mut builder,
                &api::TypedQueryArgs {
                    query: Some(expression),
                },
            );
            let source = api::QuerySource::create(
                &mut builder,
                &api::QuerySourceArgs {
                    source_type: api::QuerySourceKind::TypedQuery,
                    source: Some(typed.as_union_value()),
                },
            );
            let query_model = if query_model {
                let default_index = builder.create_string("title");
                Some(api::QueryModel::create(
                    &mut builder,
                    &api::QueryModelArgs {
                        default_index: Some(default_index),
                        ..Default::default()
                    },
                ))
            } else {
                None
            };
            let request = api::SearchRequest::create(
                &mut builder,
                &api::SearchRequestArgs {
                    query: Some(source),
                    query_model,
                    ..Default::default()
                },
            );
            builder.finish(request, None);
            builder.finished_data().to_vec()
        }

        for (name, bytes) in [
            ("query expression annotation", request_bytes(true, false)),
            ("query model", request_bytes(false, true)),
        ] {
            let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
            let converted = context().search_to_ir(request);
            assert!(
                converted.is_ok(),
                "{name} should provide user input context"
            );
        }

        let bytes = request_bytes(false, false);
        let request = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let error = context().search_to_ir(request).unwrap_err();
        assert_eq!(error.details[0].code, "missing_field_context");
    }

    #[test]
    fn sort_missing_as_is_not_rejected_or_dropped() {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field = builder.create_string("price");
        let field = api::FieldSortTarget::create(
            &mut builder,
            &api::FieldSortTargetArgs { field: Some(field) },
        );
        let target = api::SortTarget::create(
            &mut builder,
            &api::SortTargetArgs {
                target_type: api::SortTargetKind::FieldSortTarget,
                target: Some(field.as_union_value()),
            },
        );
        let value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: -1 });
        let value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::Int64Value,
                kind: Some(value.as_union_value()),
            },
        );
        let missing =
            api::MissingAs::create(&mut builder, &api::MissingAsArgs { value: Some(value) });
        let missing = api::MissingPolicy::create(
            &mut builder,
            &api::MissingPolicyArgs {
                policy_type: api::MissingPolicyKind::MissingAs,
                policy: Some(missing.as_union_value()),
            },
        );
        let key = builder.create_string("locale");
        let annotation_value = builder.create_string("en-US");
        let annotation_value = api::StringValue::create(
            &mut builder,
            &api::StringValueArgs {
                value: Some(annotation_value),
            },
        );
        let annotation_value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::StringValue,
                kind: Some(annotation_value.as_union_value()),
            },
        );
        let annotation = api::ValueEntry::create(
            &mut builder,
            &api::ValueEntryArgs {
                key: Some(key),
                value: Some(annotation_value),
            },
        );
        let annotations = builder.create_vector(&[annotation]);
        let sort = api::SortExpr::create(
            &mut builder,
            &api::SortExprArgs {
                target: Some(target),
                direction: api::SortDirection::Asc,
                missing: Some(missing),
                annotations: Some(annotations),
            },
        );
        builder.finish(sort, None);
        let sort = crate::__flatbuffers::root::<api::SortExpr>(builder.finished_data()).unwrap();

        let mut query_context = context();
        query_context.definition.fields[1].capabilities.sortable = true;
        let converted = query_context
            .sort_to_ir(sort)
            .expect("MissingAs and sort annotations must have an IR representation");
        assert_eq!(
            converted.missing(),
            MissingPlacement::As(RequestValueIr::Int64(-1))
        );
        assert_eq!(
            converted.annotations().get("locale"),
            Some(&RequestValueIr::String("en-US".to_owned()))
        );

        let request = SearchRequestIr::new(
            QueryIr::MatchAll,
            Vec::new(),
            None,
            vec![converted],
            RankingPolicyIr::DefaultRelevanceV1,
            10,
            0,
            ProjectionIr::new(Vec::new()),
            HighlightingIr::Disabled,
            None,
            PrefixPolicy::Disabled,
            TypoPolicy::Disabled,
        );
        let bytes = crate::search_request_ir_to_api(&request);
        let encoded = crate::__flatbuffers::root::<api::SearchRequest>(&bytes).unwrap();
        let encoded_sort = encoded.sort().unwrap().get(0);
        assert_eq!(
            encoded_sort
                .missing()
                .and_then(|value| value.policy_as_missing_as())
                .and_then(|value| value.value())
                .and_then(|value| value.kind_as_int_64_value())
                .map(|value| value.value()),
            Some(-1)
        );
        assert_eq!(encoded_sort.annotations().unwrap().get(0).key(), "locale");
    }

    #[test]
    fn positive_infinity_filter_lower_bound_is_not_treated_as_unbounded() {
        let mut builder = crate::__flatbuffers::FlatBufferBuilder::new();
        let field = builder.create_string("price");
        let field = api::QueryFieldRef::create(
            &mut builder,
            &api::QueryFieldRefArgs {
                path: Some(field),
                element_index: None,
            },
        );
        let lower =
            api::PositiveInfinityBound::create(&mut builder, &api::PositiveInfinityBoundArgs {});
        let lower = api::RangeBound::create(
            &mut builder,
            &api::RangeBoundArgs {
                bound_type: api::RangeBoundKind::PositiveInfinityBound,
                bound: Some(lower.as_union_value()),
            },
        );
        let upper_value = api::Int64Value::create(&mut builder, &api::Int64ValueArgs { value: 10 });
        let upper_value = api::Value::create(
            &mut builder,
            &api::ValueArgs {
                kind_type: api::ValueKind::Int64Value,
                kind: Some(upper_value.as_union_value()),
            },
        );
        let upper = api::FiniteRangeBound::create(
            &mut builder,
            &api::FiniteRangeBoundArgs {
                value: Some(upper_value),
            },
        );
        let upper = api::RangeBound::create(
            &mut builder,
            &api::RangeBoundArgs {
                bound_type: api::RangeBoundKind::FiniteRangeBound,
                bound: Some(upper.as_union_value()),
            },
        );
        let range = api::FilterRange::create(
            &mut builder,
            &api::FilterRangeArgs {
                field: Some(field),
                lower: Some(lower),
                upper: Some(upper),
                ..Default::default()
            },
        );
        let expression = api::FilterExpr::create(
            &mut builder,
            &api::FilterExprArgs {
                expr_type: api::FilterExprKind::FilterRange,
                expr: Some(range.as_union_value()),
            },
        );
        builder.finish(expression, None);
        let expression =
            crate::__flatbuffers::root::<api::FilterExpr>(builder.finished_data()).unwrap();

        assert!(
            context().filter_to_ir(expression).is_err(),
            "a lower +infinity bound must not collapse into an absent lower bound"
        );
    }
}
