use crate::error::{PortableError, Result};
use crate::model::*;
use searchkernel_api::v1 as api;
use std::collections::{BTreeMap, BTreeSet};

pub trait Backend: Send + Sync {
    fn insert(&self, request: ResolvedInsertRequest) -> Result<api::InsertResult>;
    fn upsert(&self, request: ResolvedUpsertRequest) -> Result<api::UpsertResult>;
    fn patch(&self, request: ResolvedPatchRequest) -> Result<api::PatchResult>;
    fn delete(&self, request: ResolvedDeleteRequest) -> Result<api::DeleteResult>;
    fn get(&self, request: ResolvedGetRequest) -> Result<api::GetResult>;
    fn search(&self, request: ResolvedSearchRequest) -> Result<api::SearchResult>;
    fn analyze(&self, request: ResolvedAnalyzeRequest) -> Result<api::AnalyzeResult>;
}

#[derive(Debug)]
pub struct IndexSemantics {
    definition: ResolvedIndexDefinition,
}

pub fn resolve_index_definition(
    definition: &api::IndexDefinition,
) -> Result<ResolvedIndexDefinition> {
    if definition.fields.is_empty() {
        return Err(invalid(
            "invalid_field_kind",
            Some("fields"),
            "an index must define at least one field",
        ));
    }

    let mut fields = Vec::with_capacity(definition.fields.len());
    let mut names = BTreeSet::new();
    for (index, field) in definition.fields.iter().enumerate() {
        let path = format!("fields[{index}]");
        let name = parse_field_name(&field.name, format!("{path}.name"))?;
        if !names.insert(name.clone()) {
            return Err(invalid(
                "duplicate_field",
                Some(format!("{path}.name")),
                "field names must be unique",
            ));
        }
        fields.push(resolve_field(field, name, &path)?);
    }

    let name = definition
        .name
        .as_deref()
        .map(|value| {
            IndexName::parse(value).map_err(|_| {
                invalid(
                    "invalid_index_name",
                    Some("name"),
                    "index name must not be empty",
                )
            })
        })
        .transpose()?;

    let default_search_fields = match definition.default_search_fields.as_ref() {
        None => fields
            .iter()
            .filter(|field| field.capabilities.searchable)
            .map(|field| ResolvedSearchField {
                field: field.name.clone(),
                weight: SearchFieldWeight::new(1.0).expect("default weight is valid"),
            })
            .collect(),
        Some(selection) => resolve_search_fields(selection, &fields, "default_search_fields")?,
    };

    Ok(ResolvedIndexDefinition {
        name,
        fields,
        default_search_fields,
        default_ranking: ResolvedRankingPolicy::DefaultRelevanceV1,
        default_prefix_policy: PrefixPolicy::Disabled,
        default_typo_policy: TypoPolicy::Disabled,
    })
}

impl IndexSemantics {
    pub fn new(definition: &api::IndexDefinition) -> Result<Self> {
        Ok(Self {
            definition: resolve_index_definition(definition)?,
        })
    }

    pub fn definition(&self) -> &ResolvedIndexDefinition {
        &self.definition
    }

    pub fn resolve_insert(&self, request: &api::InsertRequest) -> Result<ResolvedInsertRequest> {
        Ok(ResolvedInsertRequest {
            document: self.resolve_document(request.document.as_ref())?,
        })
    }

    pub fn resolve_upsert(&self, request: &api::UpsertRequest) -> Result<ResolvedUpsertRequest> {
        Ok(ResolvedUpsertRequest {
            document: self.resolve_document(request.document.as_ref())?,
        })
    }

    pub fn resolve_patch(&self, request: &api::PatchRequest) -> Result<ResolvedPatchRequest> {
        let id = DocumentId::parse(request.id.clone()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some("id"),
                "document ID must not be empty",
            )
        })?;
        if request.operations.is_empty() {
            return Err(invalid(
                "empty_patch",
                Some("operations"),
                "patch must contain at least one operation",
            ));
        }

        let mut seen = BTreeSet::new();
        let mut operations = Vec::with_capacity(request.operations.len());
        for (index, operation) in request.operations.iter().enumerate() {
            let path = format!("operations[{index}]");
            let operation = operation.operation.as_ref().ok_or_else(|| {
                invalid(
                    "invalid_patch_operation",
                    Some(path.clone()),
                    "patch operation must select set or unset",
                )
            })?;
            let (field_name, resolved) = match operation {
                api::patch_operation::Operation::Set(set) => {
                    let field = self.field(&set.field, format!("{path}.set.field"))?;
                    let name = field.name.clone();
                    if !seen.insert(name.clone()) {
                        return Err(invalid(
                            "duplicate_patch_field",
                            Some(format!("{path}.set.field")),
                            "a patch may mention each field at most once",
                        ));
                    }
                    let value = set.value.as_ref().ok_or_else(|| {
                        invalid(
                            "missing_value",
                            Some(format!("{path}.set.value")),
                            "set operation requires a value",
                        )
                    })?;
                    (
                        name,
                        ResolvedPatchOperation::Set {
                            field: field.name.clone(),
                            value: resolve_field_value(
                                value,
                                field.kind,
                                field.cardinality,
                                format!("{path}.set.value"),
                            )?,
                        },
                    )
                }
                api::patch_operation::Operation::Unset(unset) => {
                    let field = self.field(&unset.field, format!("{path}.unset.field"))?;
                    let name = field.name.clone();
                    if !seen.insert(name.clone()) {
                        return Err(invalid(
                            "duplicate_patch_field",
                            Some(format!("{path}.unset.field")),
                            "a patch may mention each field at most once",
                        ));
                    }
                    (
                        name,
                        ResolvedPatchOperation::Unset {
                            field: field.name.clone(),
                        },
                    )
                }
            };
            let _ = field_name;
            operations.push(resolved);
        }
        Ok(ResolvedPatchRequest { id, operations })
    }

    pub fn resolve_delete(&self, request: &api::DeleteRequest) -> Result<ResolvedDeleteRequest> {
        let id = DocumentId::parse(request.id.clone()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some("id"),
                "document ID must not be empty",
            )
        })?;
        Ok(ResolvedDeleteRequest { id })
    }

    pub fn resolve_get(&self, request: &api::GetRequest) -> Result<ResolvedGetRequest> {
        let id = DocumentId::parse(request.id.clone()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some("id"),
                "document ID must not be empty",
            )
        })?;
        Ok(ResolvedGetRequest {
            id,
            projection: self.resolve_projection(request.projection.as_ref())?,
        })
    }

    pub fn resolve_search(&self, request: &api::SearchRequest) -> Result<ResolvedSearchRequest> {
        let query = match request.query_expr.as_ref() {
            Some(expression) => {
                if !request.query.trim().is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some("query_expr"),
                        "query and query_expr cannot both be supplied",
                    ));
                }
                self.resolve_query_expr(expression, "query_expr")?
            }
            None if request.query.trim().is_empty() => ResolvedQuery::MatchAll,
            None => ResolvedQuery::Text(request.query.clone()),
        };
        let search_fields = match request.search_fields.as_ref() {
            None => self.definition.default_search_fields.clone(),
            Some(selection) => {
                resolve_search_fields(selection, &self.definition.fields, "search_fields")?
            }
        };
        if matches!(&query, ResolvedQuery::Text(_)) && search_fields.is_empty() {
            return Err(PortableError::failed_precondition(
                "no_searchable_fields",
                Some("search_fields"),
                "a text query requires at least one searchable field",
            ));
        }

        let filter = request
            .filter
            .as_ref()
            .map(|filter| self.resolve_filter(filter, "filter"))
            .transpose()?;
        let sort = self.resolve_sort(&request.sort)?;
        let facets = self.resolve_facets(&request.facets)?;
        let ranking = resolve_ranking(request.ranking.as_ref())?;
        let language = request
            .language
            .as_deref()
            .map(|value| parse_language(value, "language"))
            .transpose()?;

        Ok(ResolvedSearchRequest {
            query,
            search_fields,
            filter,
            sort,
            facets,
            ranking,
            limit: request.limit.unwrap_or(20),
            offset: request.offset.unwrap_or(0),
            projection: self.resolve_projection(request.projection.as_ref())?,
            highlighting: self.resolve_highlighting(request.highlighting.as_ref())?,
            language,
            prefix_policy: PrefixPolicy::Disabled,
            typo_policy: TypoPolicy::Disabled,
        })
    }

    pub fn resolve_analyze(&self, request: &api::AnalyzeRequest) -> Result<ResolvedAnalyzeRequest> {
        let analyzer = match request.target.as_ref().ok_or_else(|| {
            invalid(
                "invalid_analyzer",
                Some("target"),
                "analyze request must select an analyzer or field",
            )
        })? {
            api::analyze_request::Target::Analyzer(analyzer) => {
                resolve_analyzer(Some(analyzer), FieldKind::Text, "target.analyzer")?
                    .expect("text analyzer is always present")
            }
            api::analyze_request::Target::Field(field_name) => {
                let field = self.field(field_name, "target.field".to_string())?;
                if field.kind != FieldKind::Text {
                    return Err(invalid(
                        "invalid_analyzer",
                        Some("target.field"),
                        "analysis requires a text field",
                    ));
                }
                field.analyzer.ok_or_else(|| {
                    invalid(
                        "invalid_analyzer",
                        Some("target.field"),
                        "field has no analyzer",
                    )
                })?
            }
        };
        Ok(ResolvedAnalyzeRequest {
            analyzer,
            text: request.text.clone(),
            language: request
                .language
                .as_deref()
                .map(|value| parse_language(value, "language"))
                .transpose()?,
        })
    }

    pub fn describe(&self) -> api::DescribeResult {
        api::DescribeResult {
            index: Some(index_to_api(&self.definition)),
        }
    }

    pub fn inspect_query(
        &self,
        request: &api::InspectQueryRequest,
    ) -> Result<api::InspectQueryResult> {
        let request = request.request.as_ref().ok_or_else(|| {
            invalid(
                "missing_value",
                Some("request"),
                "inspect query requires a search request",
            )
        })?;
        let resolved = self.resolve_search(request)?;
        Ok(api::InspectQueryResult {
            resolved_request: Some(search_to_api(&resolved)),
        })
    }

    fn resolve_document(&self, document: Option<&api::Document>) -> Result<ResolvedDocument> {
        let document = document.ok_or_else(|| {
            invalid(
                "missing_value",
                Some("document"),
                "document operation requires a document",
            )
        })?;
        let id = DocumentId::parse(document.id.clone()).map_err(|_| {
            invalid(
                "invalid_document_id",
                Some("document.id"),
                "document ID must not be empty",
            )
        })?;
        let mut fields = BTreeMap::new();
        let mut input_fields: Vec<_> = document.fields.iter().collect();
        input_fields.sort_by_key(|(name, _)| *name);
        for (name, value) in input_fields {
            let field = self.field(name, format!("document.fields.{name}"))?;
            let resolved = resolve_field_value(
                value,
                field.kind,
                field.cardinality,
                format!("document.fields.{name}"),
            )?;
            fields.insert(field.name.clone(), resolved);
        }
        Ok(ResolvedDocument { id, fields })
    }

    fn resolve_query_expr(&self, expression: &api::QueryExpr, path: &str) -> Result<ResolvedQuery> {
        let expression = expression.expr.as_ref().ok_or_else(|| {
            invalid(
                "invalid_query",
                Some(path.to_string()),
                "query expression must select an operation",
            )
        })?;
        match expression {
            api::query_expr::Expr::Term(term) => {
                let field = self.field(&term.field, format!("{path}.term.field"))?;
                if !field.capabilities.searchable {
                    return Err(invalid(
                        "field_not_searchable",
                        Some(format!("{path}.term.field")),
                        "query term requires a searchable field",
                    ));
                }
                let value = term.value.as_ref().ok_or_else(|| {
                    invalid(
                        "missing_value",
                        Some(format!("{path}.term.value")),
                        "query term requires a value",
                    )
                })?;
                Ok(ResolvedQuery::Term {
                    field: field.name.clone(),
                    value: resolve_scalar(value, field.kind, format!("{path}.term.value"))?,
                })
            }
            api::query_expr::Expr::All(all) => {
                if all.clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.all.clauses")),
                        "query all requires at least one clause",
                    ));
                }
                let mut clauses = Vec::with_capacity(all.clauses.len());
                for (index, clause) in all.clauses.iter().enumerate() {
                    clauses.push(
                        self.resolve_query_expr(clause, &format!("{path}.all.clauses[{index}]"))?,
                    );
                }
                Ok(flatten_query_and(clauses))
            }
            api::query_expr::Expr::Any(any) => {
                if any.clauses.is_empty() {
                    return Err(invalid(
                        "invalid_query",
                        Some(format!("{path}.any.clauses")),
                        "query any requires at least one clause",
                    ));
                }
                let mut clauses = Vec::with_capacity(any.clauses.len());
                for (index, clause) in any.clauses.iter().enumerate() {
                    clauses.push(
                        self.resolve_query_expr(clause, &format!("{path}.any.clauses[{index}]"))?,
                    );
                }
                Ok(flatten_query_or(clauses))
            }
            api::query_expr::Expr::Negation(negation) => {
                let clause = negation.clause.as_ref().ok_or_else(|| {
                    invalid(
                        "invalid_query",
                        Some(format!("{path}.negation.clause")),
                        "query negation requires a clause",
                    )
                })?;
                Ok(ResolvedQuery::Not(Box::new(self.resolve_query_expr(
                    clause,
                    &format!("{path}.negation.clause"),
                )?)))
            }
        }
    }

    fn field(&self, name: &str, path: String) -> Result<&ResolvedField> {
        self.definition
            .fields
            .iter()
            .find(|field| field.name.as_str() == name)
            .ok_or_else(|| invalid("unknown_field", Some(path), "field is not defined"))
    }

    fn resolve_projection(
        &self,
        projection: Option<&api::FieldProjection>,
    ) -> Result<ResolvedProjection> {
        let fields = match projection {
            None => self
                .definition
                .fields
                .iter()
                .filter(|field| field.capabilities.stored)
                .map(|field| field.name.clone())
                .collect(),
            Some(projection) => {
                let mut seen = BTreeSet::new();
                let mut fields = Vec::with_capacity(projection.fields.len());
                for (index, name) in projection.fields.iter().enumerate() {
                    let field = self.field(name, format!("projection.fields[{index}]"))?;
                    if !field.capabilities.stored {
                        return Err(invalid(
                            "field_not_stored",
                            Some(format!("projection.fields[{index}]")),
                            "projection requires a stored field",
                        ));
                    }
                    if !seen.insert(field.name.clone()) {
                        return Err(invalid(
                            "duplicate_projection_field",
                            Some(format!("projection.fields[{index}]")),
                            "projection fields must be unique",
                        ));
                    }
                    fields.push(field.name.clone());
                }
                fields
            }
        };
        Ok(ResolvedProjection { fields })
    }

    fn resolve_highlighting(
        &self,
        highlighting: Option<&api::HighlightRequest>,
    ) -> Result<ResolvedHighlighting> {
        let Some(highlighting) = highlighting else {
            return Ok(ResolvedHighlighting::Disabled);
        };
        let mut seen = BTreeSet::new();
        let mut fields = Vec::with_capacity(highlighting.fields.len());
        for (index, name) in highlighting.fields.iter().enumerate() {
            let field = self.field(name, format!("highlighting.fields[{index}]"))?;
            if field.kind != FieldKind::Text {
                return Err(invalid(
                    "field_not_searchable",
                    Some(format!("highlighting.fields[{index}]")),
                    "highlighting requires a text field",
                ));
            }
            if !field.capabilities.searchable {
                return Err(invalid(
                    "field_not_searchable",
                    Some(format!("highlighting.fields[{index}]")),
                    "highlighting requires a searchable field",
                ));
            }
            if !field.capabilities.stored {
                return Err(invalid(
                    "field_not_stored",
                    Some(format!("highlighting.fields[{index}]")),
                    "highlighting requires a stored field",
                ));
            }
            if !seen.insert(field.name.clone()) {
                return Err(invalid(
                    "duplicate_highlight_field",
                    Some(format!("highlighting.fields[{index}]")),
                    "highlight fields must be unique",
                ));
            }
            fields.push(field.name.clone());
        }
        Ok(ResolvedHighlighting::Requested { fields })
    }

    fn resolve_filter(&self, filter: &api::FilterExpr, path: &str) -> Result<ResolvedFilterExpr> {
        let expression = filter.expr.as_ref().ok_or_else(|| {
            invalid(
                "invalid_filter",
                Some(path.to_string()),
                "filter expression must select an operation",
            )
        })?;
        match expression {
            api::filter_expr::Expr::All(all) => {
                if all.clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.all.clauses")),
                        "all requires at least one clause",
                    ));
                }
                let mut clauses = Vec::with_capacity(all.clauses.len());
                for (index, clause) in all.clauses.iter().enumerate() {
                    clauses.push(
                        self.resolve_filter(clause, &format!("{path}.all.clauses[{index}]"))?,
                    );
                }
                Ok(flatten_and(clauses))
            }
            api::filter_expr::Expr::Any(any) => {
                if any.clauses.is_empty() {
                    return Err(invalid(
                        "empty_boolean_filter",
                        Some(format!("{path}.any.clauses")),
                        "any requires at least one clause",
                    ));
                }
                let mut clauses = Vec::with_capacity(any.clauses.len());
                for (index, clause) in any.clauses.iter().enumerate() {
                    clauses.push(
                        self.resolve_filter(clause, &format!("{path}.any.clauses[{index}]"))?,
                    );
                }
                Ok(flatten_or(clauses))
            }
            api::filter_expr::Expr::Negation(negation) => {
                let clause = negation.clause.as_ref().ok_or_else(|| {
                    invalid(
                        "invalid_filter",
                        Some(format!("{path}.negation.clause")),
                        "negation requires a clause",
                    )
                })?;
                Ok(ResolvedFilterExpr::Not(Box::new(self.resolve_filter(
                    clause,
                    &format!("{path}.negation.clause"),
                )?)))
            }
            api::filter_expr::Expr::Comparison(comparison) => {
                self.resolve_comparison(comparison, &format!("{path}.comparison"))
            }
            api::filter_expr::Expr::Range(range) => {
                self.resolve_range(range, &format!("{path}.range"))
            }
            api::filter_expr::Expr::Exists(exists) => {
                let field = self.field(&exists.field, format!("{path}.exists.field"))?;
                if !field.capabilities.filterable {
                    return Err(invalid(
                        "field_not_filterable",
                        Some(format!("{path}.exists.field")),
                        "exists requires a filterable field",
                    ));
                }
                Ok(ResolvedFilterExpr::Exists {
                    field: field.name.clone(),
                })
            }
        }
    }

    fn resolve_comparison(
        &self,
        comparison: &api::FilterComparison,
        path: &str,
    ) -> Result<ResolvedFilterExpr> {
        let field = self.field(&comparison.field, format!("{path}.field"))?;
        if !field.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "comparison requires a filterable field",
            ));
        }
        let op = match comparison.op {
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
        ) && !matches!(field.kind, FieldKind::Int64 | FieldKind::Float64)
        {
            return Err(invalid(
                "invalid_filter",
                Some(format!("{path}.op")),
                "ordered comparison requires a numeric field",
            ));
        }
        let value = comparison.value.as_ref().ok_or_else(|| {
            invalid(
                "missing_value",
                Some(format!("{path}.value")),
                "comparison requires a value",
            )
        })?;
        Ok(ResolvedFilterExpr::Comparison(ResolvedComparison {
            field: field.name.clone(),
            op,
            value: resolve_scalar(value, field.kind, format!("{path}.value"))?,
        }))
    }

    fn resolve_range(&self, range: &api::FilterRange, path: &str) -> Result<ResolvedFilterExpr> {
        let field = self.field(&range.field, format!("{path}.field"))?;
        if !field.capabilities.filterable {
            return Err(invalid(
                "field_not_filterable",
                Some(format!("{path}.field")),
                "range requires a filterable field",
            ));
        }
        if !matches!(field.kind, FieldKind::Int64 | FieldKind::Float64) {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.field")),
                "range requires an Int64 or Float64 field",
            ));
        }
        if range.lower.is_none() && range.upper.is_none() {
            return Err(invalid(
                "invalid_range",
                Some(path.to_string()),
                "range requires at least one bound",
            ));
        }
        if range.lower.is_none() && range.include_lower.is_some() {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_lower")),
                "lower inclusion requires a lower bound",
            ));
        }
        if range.upper.is_none() && range.include_upper.is_some() {
            return Err(invalid(
                "invalid_range",
                Some(format!("{path}.include_upper")),
                "upper inclusion requires an upper bound",
            ));
        }
        let lower = range
            .lower
            .as_ref()
            .map(|value| resolve_scalar(value, field.kind, format!("{path}.lower")))
            .transpose()?;
        let upper = range
            .upper
            .as_ref()
            .map(|value| resolve_scalar(value, field.kind, format!("{path}.upper")))
            .transpose()?;
        if let (Some(lower), Some(upper)) = (&lower, &upper) {
            if numeric_gt(lower, upper) {
                return Err(invalid(
                    "invalid_range",
                    Some(path.to_string()),
                    "range lower bound must not exceed upper bound",
                ));
            }
        }
        Ok(ResolvedFilterExpr::Range(ResolvedRange {
            field: field.name.clone(),
            lower,
            upper,
            include_lower: range.include_lower.unwrap_or(true),
            include_upper: range.include_upper.unwrap_or(true),
        }))
    }

    fn resolve_sort(&self, sorts: &[api::SortExpr]) -> Result<Vec<ResolvedSortExpr>> {
        let mut seen = BTreeSet::new();
        let mut resolved = Vec::with_capacity(sorts.len());
        for (index, sort) in sorts.iter().enumerate() {
            let path = format!("sort[{index}]");
            let field = self.field(&sort.field, format!("{path}.field"))?;
            if !field.capabilities.sortable {
                return Err(invalid(
                    "field_not_sortable",
                    Some(format!("{path}.field")),
                    "sort requires a sortable field",
                ));
            }
            if field.cardinality != FieldCardinality::Single {
                return Err(invalid(
                    "field_not_sortable",
                    Some(format!("{path}.field")),
                    "multi-valued fields cannot be sorted",
                ));
            }
            if !seen.insert(field.name.clone()) {
                return Err(invalid(
                    "duplicate_sort_field",
                    Some(format!("{path}.field")),
                    "sort fields must be unique",
                ));
            }
            let direction = match sort.direction {
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
            let missing = match sort.missing {
                None => MissingPlacement::Last,
                Some(1) => MissingPlacement::First,
                Some(2) => MissingPlacement::Last,
                Some(_) => {
                    return Err(invalid(
                        "invalid_filter",
                        Some(format!("{path}.missing")),
                        "missing placement is unknown",
                    ))
                }
            };
            resolved.push(ResolvedSortExpr {
                field: field.name.clone(),
                direction,
                missing,
            });
        }
        Ok(resolved)
    }

    fn resolve_facets(&self, facets: &[api::FacetRequest]) -> Result<Vec<ResolvedFacetRequest>> {
        let mut seen = BTreeSet::new();
        let mut resolved = Vec::with_capacity(facets.len());
        for (index, facet) in facets.iter().enumerate() {
            let path = format!("facets[{index}]");
            let field = self.field(&facet.field, format!("{path}.field"))?;
            if !field.capabilities.facetable {
                return Err(invalid(
                    "field_not_facetable",
                    Some(format!("{path}.field")),
                    "facet requires a facetable field",
                ));
            }
            if !seen.insert(field.name.clone()) {
                return Err(invalid(
                    "duplicate_facet_field",
                    Some(format!("{path}.field")),
                    "facet fields must be unique",
                ));
            }
            let sort = match facet.sort {
                None => FacetSort::CountDesc,
                Some(1) => FacetSort::CountDesc,
                Some(2) => FacetSort::CountAsc,
                Some(3) => FacetSort::ValueAsc,
                Some(4) => FacetSort::ValueDesc,
                Some(_) => {
                    return Err(invalid(
                        "invalid_filter",
                        Some(format!("{path}.sort")),
                        "facet sort is unknown",
                    ))
                }
            };
            resolved.push(ResolvedFacetRequest {
                field: field.name.clone(),
                limit: facet.limit.unwrap_or(10),
                sort,
            });
        }
        Ok(resolved)
    }
}

fn resolve_field(
    definition: &api::FieldDefinition,
    name: FieldName,
    path: &str,
) -> Result<ResolvedField> {
    let kind = match definition.kind {
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
    let cardinality = match definition.cardinality {
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
        stored: definition.stored.unwrap_or(defaults.stored),
        searchable: definition.searchable.unwrap_or(defaults.searchable),
        filterable: definition.filterable.unwrap_or(defaults.filterable),
        sortable: definition.sortable.unwrap_or(defaults.sortable),
        facetable: definition.facetable.unwrap_or(defaults.facetable),
    };
    if cardinality == FieldCardinality::Multi && definition.sortable.is_none() {
        capabilities.sortable = false;
    }
    if kind == FieldKind::Text && capabilities.filterable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.filterable")),
            "text fields cannot be filterable",
        ));
    }
    if kind == FieldKind::Text && capabilities.sortable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.sortable")),
            "text fields cannot be sortable",
        ));
    }
    if kind == FieldKind::Text && capabilities.facetable {
        return Err(invalid(
            "invalid_capability",
            Some(format!("{path}.facetable")),
            "text fields cannot be facetable",
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
    let analyzer = resolve_analyzer(
        definition.analyzer.as_ref(),
        kind,
        &format!("{path}.analyzer"),
    )?;
    Ok(ResolvedField {
        name,
        kind,
        cardinality,
        analyzer,
        capabilities,
    })
}

fn resolve_analyzer(
    analyzer: Option<&api::Analyzer>,
    kind: FieldKind,
    path: &str,
) -> Result<Option<AnalyzerIdentity>> {
    if kind != FieldKind::Text {
        if analyzer.is_some() {
            return Err(invalid(
                "invalid_analyzer",
                Some(path.to_string()),
                "only text fields may specify an analyzer",
            ));
        }
        return Ok(None);
    }
    let Some(analyzer) = analyzer else {
        return Ok(Some(AnalyzerIdentity::StandardV1));
    };
    if analyzer.name == "standard" && analyzer.version == 1 && analyzer.options.is_empty() {
        Ok(Some(AnalyzerIdentity::StandardV1))
    } else {
        Err(PortableError::unsupported(
            "unsupported_analyzer",
            Some(path.to_string()),
            "only standard analyzer version 1 without options is supported",
        ))
    }
}

fn resolve_search_fields(
    selection: &api::SearchFieldSelection,
    fields: &[ResolvedField],
    path: &str,
) -> Result<Vec<ResolvedSearchField>> {
    let mut seen = BTreeSet::new();
    let mut resolved = Vec::with_capacity(selection.fields.len());
    for (index, search_field) in selection.fields.iter().enumerate() {
        let field_path = format!("{path}.fields[{index}].field");
        let field = fields
            .iter()
            .find(|field| field.name.as_str() == search_field.field)
            .ok_or_else(|| {
                invalid(
                    "unknown_field",
                    Some(field_path.clone()),
                    "field is not defined",
                )
            })?;
        if !field.capabilities.searchable {
            return Err(invalid(
                "field_not_searchable",
                Some(field_path),
                "search field must be searchable",
            ));
        }
        if !seen.insert(field.name.clone()) {
            return Err(invalid(
                "duplicate_search_field",
                Some(format!("{path}.fields[{index}].field")),
                "search fields must be unique",
            ));
        }
        let weight = SearchFieldWeight::new(search_field.weight.unwrap_or(1.0)).map_err(|_| {
            invalid(
                "invalid_search_weight",
                Some(format!("{path}.fields[{index}].weight")),
                "search-field weight must be finite and non-negative",
            )
        })?;
        resolved.push(ResolvedSearchField {
            field: field.name.clone(),
            weight,
        });
    }
    Ok(resolved)
}

fn parse_field_name(value: &str, path: String) -> Result<FieldName> {
    FieldName::parse(value).map_err(|_| {
        invalid(
            "invalid_field_name",
            Some(path),
            "field name must match [A-Za-z][A-Za-z0-9_]*",
        )
    })
}

fn parse_language(value: &str, path: &str) -> Result<LanguageHint> {
    let valid = !value.is_empty()
        && value.split('-').enumerate().all(|(index, part)| {
            let length = part.len();
            let length_valid = if index == 0 {
                (2..=8).contains(&length)
            } else {
                (1..=8).contains(&length)
            };
            length_valid
                && part.is_ascii()
                && if index == 0 {
                    part.bytes().all(|byte| byte.is_ascii_alphabetic())
                } else {
                    part.bytes().all(|byte| byte.is_ascii_alphanumeric())
                }
        });
    if !valid {
        return Err(invalid(
            "invalid_language",
            Some(path.to_string()),
            "language must be a basic BCP-47-style ASCII tag",
        ));
    }
    LanguageHint::parse(value.to_ascii_lowercase()).map_err(|_| {
        invalid(
            "invalid_language",
            Some(path.to_string()),
            "language must not be empty",
        )
    })
}

fn resolve_ranking(ranking: Option<&api::RankingPolicy>) -> Result<ResolvedRankingPolicy> {
    let Some(ranking) = ranking else {
        return Ok(ResolvedRankingPolicy::DefaultRelevanceV1);
    };
    match ranking.policy.as_ref() {
        Some(api::ranking_policy::Policy::DefaultRelevanceV1(_)) => {
            Ok(ResolvedRankingPolicy::DefaultRelevanceV1)
        }
        Some(api::ranking_policy::Policy::Bm25(_)) => Ok(ResolvedRankingPolicy::Bm25),
        None => Err(invalid(
            "invalid_filter",
            Some("ranking"),
            "ranking policy must select a policy",
        )),
    }
}

fn resolve_scalar(value: &api::Value, kind: FieldKind, path: String) -> Result<OwnedScalarValue> {
    let scalar = match value.kind.as_ref() {
        Some(api::value::Kind::BoolValue(value)) => OwnedScalarValue::Bool(*value),
        Some(api::value::Kind::Int64Value(value)) => OwnedScalarValue::Int64(*value),
        Some(api::value::Kind::Float64Value(value)) => {
            if !value.is_finite() {
                return Err(invalid(
                    "non_finite_number",
                    Some(path),
                    "floating-point values must be finite",
                ));
            }
            OwnedScalarValue::Float64(*value)
        }
        Some(api::value::Kind::StringValue(value)) => OwnedScalarValue::String(value.clone()),
        Some(_) => {
            return Err(invalid(
                "type_mismatch",
                Some(path),
                "value form is not supported for this field",
            ))
        }
        None => {
            return Err(invalid(
                "missing_value",
                Some(path),
                "value must select a scalar kind",
            ))
        }
    };
    let matches = matches!(
        (&scalar, kind),
        (
            OwnedScalarValue::String(_),
            FieldKind::Text | FieldKind::Keyword
        ) | (OwnedScalarValue::Int64(_), FieldKind::Int64)
            | (OwnedScalarValue::Float64(_), FieldKind::Float64)
            | (OwnedScalarValue::Bool(_), FieldKind::Bool)
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

fn resolve_field_value(
    value: &api::Value,
    kind: FieldKind,
    cardinality: FieldCardinality,
    path: String,
) -> Result<ResolvedFieldValue> {
    match cardinality {
        FieldCardinality::Single => {
            if matches!(value.kind, Some(api::value::Kind::ListValue(_))) {
                return Err(invalid(
                    "cardinality_mismatch",
                    Some(path),
                    "single-valued fields require a scalar value",
                ));
            }
            Ok(ResolvedFieldValue::Single(resolve_scalar(
                value, kind, path,
            )?))
        }
        FieldCardinality::Multi => {
            let Some(api::value::Kind::ListValue(list)) = value.kind.as_ref() else {
                return Err(invalid(
                    "cardinality_mismatch",
                    Some(path),
                    "multi-valued fields require a list value",
                ));
            };
            let values = list
                .values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    resolve_scalar(value, kind, format!("{path}.values[{index}]"))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(ResolvedFieldValue::Multi(values))
        }
    }
}

fn flatten_and(clauses: Vec<ResolvedFilterExpr>) -> ResolvedFilterExpr {
    let mut flattened = Vec::new();
    for clause in clauses {
        match clause {
            ResolvedFilterExpr::And(children) => flattened.extend(children),
            clause => flattened.push(clause),
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause exists")
    } else {
        ResolvedFilterExpr::And(flattened)
    }
}

fn flatten_or(clauses: Vec<ResolvedFilterExpr>) -> ResolvedFilterExpr {
    let mut flattened = Vec::new();
    for clause in clauses {
        match clause {
            ResolvedFilterExpr::Or(children) => flattened.extend(children),
            clause => flattened.push(clause),
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause exists")
    } else {
        ResolvedFilterExpr::Or(flattened)
    }
}

fn flatten_query_and(clauses: Vec<ResolvedQuery>) -> ResolvedQuery {
    let mut flattened = Vec::new();
    for clause in clauses {
        match clause {
            ResolvedQuery::And(children) => flattened.extend(children),
            clause => flattened.push(clause),
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause exists")
    } else {
        ResolvedQuery::And(flattened)
    }
}

fn flatten_query_or(clauses: Vec<ResolvedQuery>) -> ResolvedQuery {
    let mut flattened = Vec::new();
    for clause in clauses {
        match clause {
            ResolvedQuery::Or(children) => flattened.extend(children),
            clause => flattened.push(clause),
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one clause exists")
    } else {
        ResolvedQuery::Or(flattened)
    }
}

fn numeric_gt(left: &OwnedScalarValue, right: &OwnedScalarValue) -> bool {
    match (left, right) {
        (OwnedScalarValue::Int64(left), OwnedScalarValue::Int64(right)) => left > right,
        (OwnedScalarValue::Float64(left), OwnedScalarValue::Float64(right)) => left > right,
        _ => false,
    }
}

fn invalid(
    code: impl Into<String>,
    path: Option<impl Into<String>>,
    message: impl Into<String>,
) -> PortableError {
    PortableError::invalid(code, path, message)
}

fn index_to_api(index: &ResolvedIndexDefinition) -> api::IndexDefinition {
    api::IndexDefinition {
        name: index.name.as_ref().map(|name| name.as_str().to_owned()),
        fields: index.fields.iter().map(field_to_api).collect(),
        default_search_fields: Some(api::SearchFieldSelection {
            fields: index
                .default_search_fields
                .iter()
                .map(search_field_to_api)
                .collect(),
        }),
    }
}

fn field_to_api(field: &ResolvedField) -> api::FieldDefinition {
    api::FieldDefinition {
        name: field.name.as_str().to_owned(),
        kind: match field.kind {
            FieldKind::Text => 1,
            FieldKind::Keyword => 2,
            FieldKind::Int64 => 3,
            FieldKind::Float64 => 4,
            FieldKind::Bool => 5,
        },
        cardinality: match field.cardinality {
            FieldCardinality::Single => 1,
            FieldCardinality::Multi => 2,
        },
        stored: Some(field.capabilities.stored),
        searchable: Some(field.capabilities.searchable),
        filterable: Some(field.capabilities.filterable),
        sortable: Some(field.capabilities.sortable),
        facetable: Some(field.capabilities.facetable),
        analyzer: field.analyzer.map(|_| api::Analyzer {
            name: "standard".to_owned(),
            version: 1,
            options: std::collections::HashMap::new(),
        }),
    }
}

fn search_field_to_api(field: &ResolvedSearchField) -> api::SearchField {
    api::SearchField {
        field: field.field.as_str().to_owned(),
        weight: Some(field.weight.get()),
    }
}

fn search_to_api(request: &ResolvedSearchRequest) -> api::SearchRequest {
    api::SearchRequest {
        query: match &request.query {
            ResolvedQuery::MatchAll => String::new(),
            ResolvedQuery::Text(query) => query.clone(),
            ResolvedQuery::Term { .. }
            | ResolvedQuery::And(_)
            | ResolvedQuery::Or(_)
            | ResolvedQuery::Not(_) => String::new(),
        },
        query_expr: match &request.query {
            ResolvedQuery::Term { .. }
            | ResolvedQuery::And(_)
            | ResolvedQuery::Or(_)
            | ResolvedQuery::Not(_) => Some(query_to_api(&request.query)),
            ResolvedQuery::MatchAll | ResolvedQuery::Text(_) => None,
        },
        search_fields: Some(api::SearchFieldSelection {
            fields: request
                .search_fields
                .iter()
                .map(search_field_to_api)
                .collect(),
        }),
        filter: request.filter.as_ref().map(filter_to_api),
        sort: request.sort.iter().map(sort_to_api).collect(),
        facets: request.facets.iter().map(facet_to_api).collect(),
        ranking: Some(ranking_to_api(request.ranking)),
        limit: Some(request.limit),
        offset: Some(request.offset),
        projection: Some(api::FieldProjection {
            fields: request
                .projection
                .fields
                .iter()
                .map(|field| field.as_str().to_owned())
                .collect(),
        }),
        highlighting: match &request.highlighting {
            ResolvedHighlighting::Disabled => None,
            ResolvedHighlighting::Requested { fields } => Some(api::HighlightRequest {
                fields: fields
                    .iter()
                    .map(|field| field.as_str().to_owned())
                    .collect(),
            }),
        },
        language: request
            .language
            .as_ref()
            .map(|language| language.as_str().to_owned()),
    }
}

fn query_to_api(query: &ResolvedQuery) -> api::QueryExpr {
    let expr = match query {
        ResolvedQuery::Term { field, value } => api::query_expr::Expr::Term(api::TermQuery {
            field: field.as_str().to_owned(),
            value: Some(value.clone().into()),
        }),
        ResolvedQuery::And(clauses) => api::query_expr::Expr::All(api::QueryAnd {
            clauses: clauses.iter().map(query_to_api).collect(),
        }),
        ResolvedQuery::Or(clauses) => api::query_expr::Expr::Any(api::QueryOr {
            clauses: clauses.iter().map(query_to_api).collect(),
        }),
        ResolvedQuery::Not(clause) => api::query_expr::Expr::Negation(Box::new(api::QueryNot {
            clause: Some(Box::new(query_to_api(clause))),
        })),
        ResolvedQuery::MatchAll | ResolvedQuery::Text(_) => {
            unreachable!("only structured queries are serialized as query_expr")
        }
    };
    api::QueryExpr { expr: Some(expr) }
}

fn filter_to_api(filter: &ResolvedFilterExpr) -> api::FilterExpr {
    let expr = match filter {
        ResolvedFilterExpr::And(clauses) => api::filter_expr::Expr::All(api::FilterAnd {
            clauses: clauses.iter().map(filter_to_api).collect(),
        }),
        ResolvedFilterExpr::Or(clauses) => api::filter_expr::Expr::Any(api::FilterOr {
            clauses: clauses.iter().map(filter_to_api).collect(),
        }),
        ResolvedFilterExpr::Not(clause) => {
            api::filter_expr::Expr::Negation(Box::new(api::FilterNot {
                clause: Some(Box::new(filter_to_api(clause))),
            }))
        }
        ResolvedFilterExpr::Comparison(comparison) => {
            api::filter_expr::Expr::Comparison(api::FilterComparison {
                field: comparison.field.as_str().to_owned(),
                op: comparison.op as i32 + 1,
                value: Some(comparison.value.clone().into()),
            })
        }
        ResolvedFilterExpr::Range(range) => api::filter_expr::Expr::Range(api::FilterRange {
            field: range.field.as_str().to_owned(),
            lower: range.lower.clone().map(Into::into),
            upper: range.upper.clone().map(Into::into),
            include_lower: Some(range.include_lower),
            include_upper: Some(range.include_upper),
        }),
        ResolvedFilterExpr::Exists { field } => api::filter_expr::Expr::Exists(api::FilterExists {
            field: field.as_str().to_owned(),
        }),
    };
    api::FilterExpr { expr: Some(expr) }
}

fn sort_to_api(sort: &ResolvedSortExpr) -> api::SortExpr {
    api::SortExpr {
        field: sort.field.as_str().to_owned(),
        direction: match sort.direction {
            SortDirection::Asc => 1,
            SortDirection::Desc => 2,
        },
        missing: Some(match sort.missing {
            MissingPlacement::First => 1,
            MissingPlacement::Last => 2,
        }),
    }
}

fn facet_to_api(facet: &ResolvedFacetRequest) -> api::FacetRequest {
    api::FacetRequest {
        field: facet.field.as_str().to_owned(),
        limit: Some(facet.limit),
        sort: Some(match facet.sort {
            FacetSort::CountDesc => 1,
            FacetSort::CountAsc => 2,
            FacetSort::ValueAsc => 3,
            FacetSort::ValueDesc => 4,
        }),
    }
}

fn ranking_to_api(ranking: ResolvedRankingPolicy) -> api::RankingPolicy {
    api::RankingPolicy {
        policy: Some(match ranking {
            ResolvedRankingPolicy::DefaultRelevanceV1 => {
                api::ranking_policy::Policy::DefaultRelevanceV1(api::DefaultRelevanceV1 {})
            }
            ResolvedRankingPolicy::Bm25 => api::ranking_policy::Policy::Bm25(api::Bm25Ranking {}),
        }),
    }
}
