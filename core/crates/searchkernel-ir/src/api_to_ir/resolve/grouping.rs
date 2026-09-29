use super::*;

/// Resolve the facet shorthand through the same grouping semantics used by a
/// complete search request. This is exposed for verifier/component callers so
/// they cannot reimplement schema and duplicate-field validation.
#[doc(hidden)]
pub fn resolve_facets(
    context: &IndexContext,
    facets: Vec<FacetInput>,
) -> Result<Vec<GroupingRequestIr>> {
    resolve_facets_at(context, facets, "facet")
}

#[doc(hidden)]
pub fn resolve_facets_at(
    context: &IndexContext,
    facets: Vec<FacetInput>,
    path: &str,
) -> Result<Vec<GroupingRequestIr>> {
    resolve_grouping_at(
        context,
        &facets
            .into_iter()
            .map(GroupingInput::Facet)
            .collect::<Vec<_>>(),
        path,
    )
}

pub(super) fn resolve_grouping(
    context: &IndexContext,
    input: &[GroupingInput],
) -> Result<Vec<GroupingRequestIr>> {
    resolve_grouping_at(context, input, "facets")
}

fn resolve_grouping_at(
    context: &IndexContext,
    input: &[GroupingInput],
    facet_path: &str,
) -> Result<Vec<GroupingRequestIr>> {
    let mut facet_fields = BTreeSet::new();
    let mut root_output_names = BTreeSet::new();
    input
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let (resolved, duplicate_path) = match value {
                GroupingInput::Facet(value) => {
                    let path = if facet_path == "facet" {
                        "facet.field".to_owned()
                    } else {
                        format!("{facet_path}[{index}].field")
                    };
                    validate_grouping_field(context, &value.field, &path)?;
                    (resolve_facet(value)?, path)
                }
                GroupingInput::Structured(value) => {
                    let path = format!("grouping[{index}]");
                    validate_grouping_request(context, value, &path)?;
                    validate_root_output_names(value, &path, &mut root_output_names)?;
                    (
                        value.clone(),
                        format!("{path}.root.operations[0].expression"),
                    )
                }
                GroupingInput::Raw(_) => return Err(PortableError::unsupported(
                    "raw_grouping_not_supported",
                    Some(format!("grouping[{index}].source")),
                    "raw grouping text requires a grouping parser before it can enter canonical IR",
                )),
            };
            if let Some(field) = canonical_facet_field(&resolved) {
                if !facet_fields.insert(field.to_owned()) {
                    return Err(invalid(
                        "duplicate_facet_field",
                        Some(duplicate_path),
                        "facet fields must be unique",
                    ));
                }
            }
            Ok(resolved)
        })
        .collect()
}

/// Root aggregation outputs from separate grouping requests are flattened into
/// one portable result-field map by the adapter. Keep this namespace check at
/// the canonical semantic boundary so a later native pass cannot silently
/// overwrite an earlier field. Outputs inside `each()`/`all()` remain scoped
/// to their grouping tree and are intentionally not included here.
fn validate_root_output_names(
    value: &GroupingRequestIr,
    path: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut aliases = BTreeMap::new();
    for (index, operation) in value.root.operations.iter().enumerate() {
        let operation_path = format!("{path}.root.operations[{index}]");
        match operation {
            GroupingOperationIr::Alias { name, expression } => {
                aliases.insert(name.clone(), expression.clone());
            }
            GroupingOperationIr::Output(outputs) => {
                for (output_index, output) in outputs.iter().enumerate() {
                    let output_path = format!("{operation_path}.outputs[{output_index}]");
                    if let Some(name) =
                        crate::model::effective_grouping_output_name(output, &aliases)
                    {
                        if !names.insert(name) {
                            return Err(invalid(
                                "grouping_output_name_collision",
                                Some(output_path),
                                "root grouping output names must be unique across grouping requests",
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_grouping_request(
    context: &IndexContext,
    value: &GroupingRequestIr,
    path: &str,
) -> Result<()> {
    let root_path = format!("{path}.root");
    validate_grouping_all(context, &value.root, &root_path)?;
    validate_grouping_semantics(&value.root, &root_path)
}

#[derive(Default)]
struct GroupingSemanticScope {
    aliases: BTreeMap<String, GroupingExpressionIr>,
    output_names: BTreeSet<String>,
    has_max: bool,
    has_summary: bool,
}

fn validate_grouping_semantics(value: &GroupingAllIr, path: &str) -> Result<()> {
    let mut scope = GroupingSemanticScope::default();
    validate_grouping_scope_operations(value, path, &mut scope)
}

fn validate_grouping_scope_operations(
    value: &GroupingAllIr,
    path: &str,
    scope: &mut GroupingSemanticScope,
) -> Result<()> {
    for (index, operation) in value.operations.iter().enumerate() {
        let operation_path = format!("{path}.operations[{index}]");
        match operation {
            // each(...) materializes the enclosing group and therefore belongs
            // to the same semantic level. all(...) starts a nested level with
            // its own aliases, outputs, and max/summary constraints.
            GroupingOperationIr::Each(child) => {
                validate_grouping_scope_operations(child, &operation_path, scope)?;
            }
            GroupingOperationIr::All(child) => {
                validate_grouping_semantics(child, &operation_path)?;
            }
            GroupingOperationIr::Alias { name, expression } => {
                validate_grouping_alias_references(
                    expression,
                    &scope.aliases,
                    &format!("{operation_path}.expression"),
                )?;
                if scope.aliases.contains_key(name) {
                    return Err(invalid(
                        "duplicate_grouping_alias",
                        Some(format!("{operation_path}.name")),
                        "grouping alias names must be unique within their scope",
                    ));
                }
                if grouping_expression_contains_summary(expression, &scope.aliases) {
                    scope.has_summary = true;
                    reject_grouping_summary_max_conflict(scope.has_max, &operation_path)?;
                }
                scope.aliases.insert(name.clone(), expression.clone());
            }
            GroupingOperationIr::Output(outputs) => {
                for (output_index, output) in outputs.iter().enumerate() {
                    let output_path = format!("{operation_path}.outputs[{output_index}]");
                    validate_grouping_alias_references(
                        &output.expression,
                        &scope.aliases,
                        &format!("{output_path}.expression"),
                    )?;
                    if grouping_expression_contains_summary(&output.expression, &scope.aliases) {
                        scope.has_summary = true;
                        reject_grouping_summary_max_conflict(scope.has_max, &output_path)?;
                    }
                    if let Some(name) =
                        crate::model::effective_grouping_output_name(output, &scope.aliases)
                    {
                        if !scope.output_names.insert(name) {
                            return Err(invalid(
                                "grouping_output_name_collision",
                                Some(output_path),
                                "grouping output names must be unique within their scope",
                            ));
                        }
                    }
                }
            }
            GroupingOperationIr::Group(expression) | GroupingOperationIr::Where(expression) => {
                validate_grouping_alias_references(
                    expression,
                    &scope.aliases,
                    &format!("{operation_path}.expression"),
                )?;
                if grouping_expression_contains_summary(expression, &scope.aliases) {
                    scope.has_summary = true;
                    reject_grouping_summary_max_conflict(scope.has_max, &operation_path)?;
                }
            }
            GroupingOperationIr::Order(expressions) => {
                for (expression_index, expression) in expressions.iter().enumerate() {
                    validate_grouping_alias_references(
                        expression,
                        &scope.aliases,
                        &format!("{operation_path}.expressions[{expression_index}]"),
                    )?;
                    if grouping_expression_contains_summary(expression, &scope.aliases) {
                        scope.has_summary = true;
                        reject_grouping_summary_max_conflict(scope.has_max, &operation_path)?;
                    }
                }
            }
            GroupingOperationIr::Filter(filter) | GroupingOperationIr::Keep(filter) => {
                validate_grouping_filter_alias_references(filter, &scope.aliases, &operation_path)?;
                if grouping_filter_contains_summary(filter, &scope.aliases) {
                    scope.has_summary = true;
                    reject_grouping_summary_max_conflict(scope.has_max, &operation_path)?;
                }
            }
            GroupingOperationIr::Max(_) => {
                scope.has_max = true;
                reject_grouping_summary_max_conflict(scope.has_summary, &operation_path)?;
            }
            GroupingOperationIr::Precision(_) => {}
        }
    }
    Ok(())
}

fn reject_grouping_summary_max_conflict(conflict: bool, path: &str) -> Result<()> {
    if conflict {
        return Err(invalid(
            "grouping_summary_max_ambiguous",
            Some(path),
            "summary() cannot share a grouping scope with max(); the portable API has no distinct per-group hit limit",
        ));
    }
    Ok(())
}

fn validate_grouping_alias_references(
    value: &GroupingExpressionIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
    path: &str,
) -> Result<()> {
    match value {
        GroupingExpressionIr::Alias(name) => {
            if !aliases.contains_key(name) {
                return Err(invalid(
                    "grouping_alias_unknown",
                    Some(path),
                    format!("grouping alias `{name}` is not defined in this scope"),
                ));
            }
            Ok(())
        }
        GroupingExpressionIr::Function { arguments, .. } => {
            for (index, argument) in arguments.iter().enumerate() {
                validate_grouping_alias_references(
                    argument,
                    aliases,
                    &format!("{path}.arguments[{index}]"),
                )?;
            }
            Ok(())
        }
        GroupingExpressionIr::Positive(expression) | GroupingExpressionIr::Negative(expression) => {
            validate_grouping_alias_references(expression, aliases, &format!("{path}.expression"))
        }
        GroupingExpressionIr::Aggregation(aggregation) => {
            validate_grouping_aggregation_alias_references(
                aggregation,
                aliases,
                &format!("{path}.aggregation"),
            )
        }
        GroupingExpressionIr::Binary { left, right, .. } => {
            validate_grouping_alias_references(left, aliases, &format!("{path}.left"))?;
            validate_grouping_alias_references(right, aliases, &format!("{path}.right"))
        }
        GroupingExpressionIr::Member { base, .. } => {
            validate_grouping_alias_references(base, aliases, &format!("{path}.base"))
        }
        GroupingExpressionIr::MapLookup { base, key } => {
            validate_grouping_alias_references(base, aliases, &format!("{path}.base"))?;
            validate_grouping_alias_references(key, aliases, &format!("{path}.key"))
        }
        GroupingExpressionIr::Attribute(_)
        | GroupingExpressionIr::Literal(_)
        | GroupingExpressionIr::Bucket(_) => Ok(()),
    }
}

fn validate_grouping_aggregation_alias_references(
    value: &GroupingAggregationIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
    path: &str,
) -> Result<()> {
    match value {
        GroupingAggregationIr::Count | GroupingAggregationIr::Summary(_) => Ok(()),
        GroupingAggregationIr::Sum(expression)
        | GroupingAggregationIr::Avg(expression)
        | GroupingAggregationIr::Min(expression)
        | GroupingAggregationIr::Max(expression)
        | GroupingAggregationIr::Xor(expression)
        | GroupingAggregationIr::StdDev(expression) => {
            validate_grouping_alias_references(expression, aliases, &format!("{path}.expression"))
        }
        GroupingAggregationIr::Quantiles { expression, .. } => {
            validate_grouping_alias_references(expression, aliases, &format!("{path}.expression"))
        }
    }
}

fn validate_grouping_filter_alias_references(
    value: &GroupingFilterIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
    path: &str,
) -> Result<()> {
    match value {
        GroupingFilterIr::And(values) | GroupingFilterIr::Or(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_grouping_filter_alias_references(
                    value,
                    aliases,
                    &format!("{path}.filters[{index}]"),
                )?;
            }
            Ok(())
        }
        GroupingFilterIr::Not(value) => {
            validate_grouping_filter_alias_references(value, aliases, &format!("{path}.filter"))
        }
        GroupingFilterIr::Regex { expression, .. } | GroupingFilterIr::IsTrue(expression) => {
            validate_grouping_alias_references(expression, aliases, &format!("{path}.expression"))
        }
        GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            ..
        } => {
            validate_grouping_alias_references(expression, aliases, &format!("{path}.expression"))?;
            validate_grouping_alias_references(lower, aliases, &format!("{path}.lower"))?;
            validate_grouping_alias_references(upper, aliases, &format!("{path}.upper"))
        }
    }
}

fn grouping_expression_contains_summary(
    value: &GroupingExpressionIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
) -> bool {
    match value {
        GroupingExpressionIr::Alias(name) => aliases
            .get(name)
            .is_some_and(|value| grouping_expression_contains_summary(value, aliases)),
        GroupingExpressionIr::Function { arguments, .. } => arguments
            .iter()
            .any(|value| grouping_expression_contains_summary(value, aliases)),
        GroupingExpressionIr::Positive(expression) | GroupingExpressionIr::Negative(expression) => {
            grouping_expression_contains_summary(expression, aliases)
        }
        GroupingExpressionIr::Aggregation(aggregation) => {
            grouping_aggregation_contains_summary(aggregation, aliases)
        }
        GroupingExpressionIr::Binary { left, right, .. } => {
            grouping_expression_contains_summary(left, aliases)
                || grouping_expression_contains_summary(right, aliases)
        }
        GroupingExpressionIr::Member { base, .. } => {
            grouping_expression_contains_summary(base, aliases)
        }
        GroupingExpressionIr::MapLookup { base, key } => {
            grouping_expression_contains_summary(base, aliases)
                || grouping_expression_contains_summary(key, aliases)
        }
        GroupingExpressionIr::Attribute(_)
        | GroupingExpressionIr::Literal(_)
        | GroupingExpressionIr::Bucket(_) => false,
    }
}

fn grouping_aggregation_contains_summary(
    value: &GroupingAggregationIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
) -> bool {
    match value {
        GroupingAggregationIr::Summary(_) => true,
        GroupingAggregationIr::Count => false,
        GroupingAggregationIr::Sum(expression)
        | GroupingAggregationIr::Avg(expression)
        | GroupingAggregationIr::Min(expression)
        | GroupingAggregationIr::Max(expression)
        | GroupingAggregationIr::Xor(expression)
        | GroupingAggregationIr::StdDev(expression) => {
            grouping_expression_contains_summary(expression, aliases)
        }
        GroupingAggregationIr::Quantiles { expression, .. } => {
            grouping_expression_contains_summary(expression, aliases)
        }
    }
}

fn grouping_filter_contains_summary(
    value: &GroupingFilterIr,
    aliases: &BTreeMap<String, GroupingExpressionIr>,
) -> bool {
    match value {
        GroupingFilterIr::And(values) | GroupingFilterIr::Or(values) => values
            .iter()
            .any(|value| grouping_filter_contains_summary(value, aliases)),
        GroupingFilterIr::Not(value) => grouping_filter_contains_summary(value, aliases),
        GroupingFilterIr::Regex { expression, .. } | GroupingFilterIr::IsTrue(expression) => {
            grouping_expression_contains_summary(expression, aliases)
        }
        GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            ..
        } => {
            grouping_expression_contains_summary(expression, aliases)
                || grouping_expression_contains_summary(lower, aliases)
                || grouping_expression_contains_summary(upper, aliases)
        }
    }
}

fn validate_grouping_all(context: &IndexContext, value: &GroupingAllIr, path: &str) -> Result<()> {
    for (index, operation) in value.operations.iter().enumerate() {
        let operation_path = format!("{path}.operations[{index}]");
        match operation {
            GroupingOperationIr::All(value) | GroupingOperationIr::Each(value) => {
                validate_grouping_all(context, value, &operation_path)?;
            }
            GroupingOperationIr::Group(expression) | GroupingOperationIr::Where(expression) => {
                validate_grouping_expression(
                    context,
                    expression,
                    &format!("{operation_path}.expression"),
                )?;
            }
            GroupingOperationIr::Output(outputs) => {
                for (index, output) in outputs.iter().enumerate() {
                    validate_grouping_output_expression(
                        context,
                        &output.expression,
                        &format!("{operation_path}.outputs[{index}].expression"),
                    )?;
                }
            }
            GroupingOperationIr::Order(expressions) => {
                for (index, expression) in expressions.iter().enumerate() {
                    validate_grouping_expression(
                        context,
                        expression,
                        &format!("{operation_path}.expressions[{index}]"),
                    )?;
                }
            }
            GroupingOperationIr::Alias { name, expression } => {
                validate_non_empty_grouping_name(name, &format!("{operation_path}.name"), "alias")?;
                validate_grouping_expression(
                    context,
                    expression,
                    &format!("{operation_path}.expression"),
                )?;
            }
            GroupingOperationIr::Filter(filter) | GroupingOperationIr::Keep(filter) => {
                validate_grouping_filter(context, filter, &operation_path)?;
            }
            GroupingOperationIr::Max(_) | GroupingOperationIr::Precision(_) => {}
        }
    }
    Ok(())
}

fn validate_grouping_expression(
    context: &IndexContext,
    value: &GroupingExpressionIr,
    path: &str,
) -> Result<()> {
    match value {
        GroupingExpressionIr::Attribute(name) => validate_grouping_field(context, name, path),
        GroupingExpressionIr::Literal(_) | GroupingExpressionIr::Bucket(_) => Ok(()),
        GroupingExpressionIr::Function { name, arguments } => {
            validate_non_empty_grouping_name(name, &format!("{path}.name"), "function")?;
            if !supported_grouping_function(name) {
                return Err(PortableError::unsupported(
                    "unsupported_grouping_function",
                    Some(format!("{path}.name")),
                    format!("grouping function `{name}` is not supported by the portable grouping grammar"),
                ));
            }
            for (index, argument) in arguments.iter().enumerate() {
                validate_grouping_expression(
                    context,
                    argument,
                    &format!("{path}.arguments[{index}]"),
                )?;
            }
            Ok(())
        }
        GroupingExpressionIr::Positive(expression) | GroupingExpressionIr::Negative(expression) => {
            validate_grouping_expression(context, expression, &format!("{path}.expression"))
        }
        GroupingExpressionIr::Aggregation(aggregation) => {
            validate_grouping_aggregation(context, aggregation, &format!("{path}.aggregation"))
        }
        GroupingExpressionIr::Binary { left, right, .. } => {
            validate_grouping_expression(context, left, &format!("{path}.left"))?;
            validate_grouping_expression(context, right, &format!("{path}.right"))
        }
        GroupingExpressionIr::Alias(name) => validate_non_empty_grouping_name(name, path, "alias"),
        GroupingExpressionIr::Member { base, member } => {
            validate_grouping_expression(context, base, &format!("{path}.base"))?;
            validate_non_empty_grouping_name(member, &format!("{path}.member"), "member")
        }
        GroupingExpressionIr::MapLookup { base, key } => {
            validate_grouping_expression(context, base, &format!("{path}.base"))?;
            validate_grouping_expression(context, key, &format!("{path}.key"))
        }
    }
}

fn validate_grouping_output_expression(
    context: &IndexContext,
    value: &GroupingExpressionIr,
    path: &str,
) -> Result<()> {
    match value {
        GroupingExpressionIr::Aggregation(aggregation) => {
            validate_grouping_aggregation(context, aggregation, &format!("{path}.aggregation"))
        }
        _ => validate_grouping_expression(context, value, path),
    }
}

fn validate_grouping_aggregation(
    context: &IndexContext,
    value: &GroupingAggregationIr,
    path: &str,
) -> Result<()> {
    match value {
        GroupingAggregationIr::Count | GroupingAggregationIr::Summary(_) => Ok(()),
        GroupingAggregationIr::Sum(expression)
        | GroupingAggregationIr::Avg(expression)
        | GroupingAggregationIr::Min(expression)
        | GroupingAggregationIr::Max(expression)
        | GroupingAggregationIr::Xor(expression)
        | GroupingAggregationIr::StdDev(expression) => validate_grouping_aggregation_expression(
            context,
            expression,
            &format!("{path}.expression"),
        ),
        GroupingAggregationIr::Quantiles {
            quantiles,
            expression,
        } => {
            if quantiles
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            {
                return Err(invalid(
                    "invalid_quantile",
                    Some(format!("{path}.quantiles")),
                    "quantiles must be finite values in the inclusive range [0, 1]",
                ));
            }
            validate_grouping_aggregation_expression(
                context,
                expression,
                &format!("{path}.expression"),
            )
        }
    }
}

fn validate_grouping_aggregation_expression(
    context: &IndexContext,
    value: &GroupingExpressionIr,
    path: &str,
) -> Result<()> {
    match value {
        GroupingExpressionIr::Attribute(name) => {
            validate_non_empty_grouping_name(name, path, "attribute")?;
            let field = context.field_for_resolver(name, path.to_owned())?;
            if !matches!(field.kind(), FieldKind::Int64 | FieldKind::Float64) {
                return Err(invalid(
                    "field_not_numeric",
                    Some(path),
                    "numeric grouping aggregations require an integer or floating-point field",
                ));
            }
            Ok(())
        }
        GroupingExpressionIr::Literal(_)
        | GroupingExpressionIr::Bucket(_)
        | GroupingExpressionIr::Alias(_) => Ok(()),
        GroupingExpressionIr::Function { name, arguments } => {
            validate_non_empty_grouping_name(name, &format!("{path}.name"), "function")?;
            if !supported_grouping_function(name) {
                return Err(PortableError::unsupported(
                    "unsupported_grouping_function",
                    Some(format!("{path}.name")),
                    format!("grouping function `{name}` is not supported by the portable grouping grammar"),
                ));
            }
            for (index, argument) in arguments.iter().enumerate() {
                validate_grouping_aggregation_expression(
                    context,
                    argument,
                    &format!("{path}.arguments[{index}]"),
                )?;
            }
            Ok(())
        }
        GroupingExpressionIr::Positive(expression) | GroupingExpressionIr::Negative(expression) => {
            validate_grouping_aggregation_expression(
                context,
                expression,
                &format!("{path}.expression"),
            )
        }
        GroupingExpressionIr::Aggregation(aggregation) => {
            validate_grouping_aggregation(context, aggregation, &format!("{path}.aggregation"))
        }
        GroupingExpressionIr::Binary { left, right, .. } => {
            validate_grouping_aggregation_expression(context, left, &format!("{path}.left"))?;
            validate_grouping_aggregation_expression(context, right, &format!("{path}.right"))
        }
        GroupingExpressionIr::Member { base, member } => {
            validate_grouping_aggregation_expression(context, base, &format!("{path}.base"))?;
            validate_non_empty_grouping_name(member, &format!("{path}.member"), "member")
        }
        GroupingExpressionIr::MapLookup { base, key } => {
            validate_grouping_aggregation_expression(context, base, &format!("{path}.base"))?;
            validate_grouping_aggregation_expression(context, key, &format!("{path}.key"))
        }
    }
}

fn validate_grouping_filter(
    context: &IndexContext,
    value: &GroupingFilterIr,
    path: &str,
) -> Result<()> {
    match value {
        GroupingFilterIr::And(values) | GroupingFilterIr::Or(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_grouping_filter(context, value, &format!("{path}.filters[{index}]"))?;
            }
            Ok(())
        }
        GroupingFilterIr::Not(value) => {
            validate_grouping_filter(context, value, &format!("{path}.filter"))
        }
        GroupingFilterIr::Regex { expression, .. } => {
            validate_grouping_expression(context, expression, &format!("{path}.expression"))
        }
        GroupingFilterIr::Range {
            expression,
            lower,
            upper,
            ..
        } => {
            validate_grouping_expression(context, expression, &format!("{path}.expression"))?;
            validate_grouping_expression(context, lower, &format!("{path}.lower"))?;
            validate_grouping_expression(context, upper, &format!("{path}.upper"))
        }
        GroupingFilterIr::IsTrue(expression) => {
            validate_grouping_expression(context, expression, &format!("{path}.expression"))
        }
    }
}

fn validate_grouping_field(context: &IndexContext, name: &str, path: &str) -> Result<()> {
    validate_non_empty_grouping_name(name, path, "attribute")?;
    let field = context.field_for_resolver(name, path.to_owned())?;
    if !field.capabilities().groupable() {
        return Err(invalid(
            "field_not_facetable",
            Some(path),
            "grouping requires a facetable field",
        ));
    }
    Ok(())
}

fn validate_non_empty_grouping_name(value: &str, path: &str, kind: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(invalid(
            "invalid_grouping",
            Some(path),
            format!("grouping {kind} must not be empty"),
        ));
    }
    Ok(())
}

#[doc(hidden)]
pub fn facet_grouping_input(input: &FacetInput) -> GroupingRequestIr {
    let field = GroupingExpressionIr::Attribute(input.field.clone());
    let count = GroupingExpressionIr::Aggregation(Box::new(GroupingAggregationIr::Count));
    let order = match input.sort.unwrap_or(FacetSortInput::CountDesc) {
        FacetSortInput::CountDesc | FacetSortInput::ValueDesc => GroupingExpressionIr::Negative(
            Box::new(if matches!(input.sort, Some(FacetSortInput::ValueDesc)) {
                field.clone()
            } else {
                count.clone()
            }),
        ),
        FacetSortInput::CountAsc => count.clone(),
        FacetSortInput::ValueAsc => field.clone(),
    };
    let operations = vec![
        GroupingOperationIr::Group(field.clone()),
        GroupingOperationIr::Each(GroupingAllIr {
            operations: vec![
                GroupingOperationIr::Max(Some(input.limit.unwrap_or(10))),
                GroupingOperationIr::Order(vec![order]),
                GroupingOperationIr::Output(vec![GroupingOutputIr {
                    label: None,
                    expression: count,
                }]),
            ],
            label: None,
        }),
    ];
    GroupingRequestIr {
        root: GroupingAllIr {
            operations,
            label: Some(
                input
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(&input.field)
                    .to_owned(),
            ),
        },
        continuations: Vec::new(),
    }
}

fn resolve_facet(input: &FacetInput) -> Result<GroupingRequestIr> {
    Ok(facet_grouping_input(input))
}

fn canonical_facet_field(value: &GroupingRequestIr) -> Option<&str> {
    if !value.continuations.is_empty() {
        return None;
    }
    let [GroupingOperationIr::Group(GroupingExpressionIr::Attribute(field)), GroupingOperationIr::Each(each)] =
        value.root.operations.as_slice()
    else {
        return None;
    };
    if each.label.is_some() {
        return None;
    }
    let [GroupingOperationIr::Max(Some(_)), GroupingOperationIr::Order(order), GroupingOperationIr::Output(outputs)] =
        each.operations.as_slice()
    else {
        return None;
    };
    let [order] = order.as_slice() else {
        return None;
    };
    let [output] = outputs.as_slice() else {
        return None;
    };
    if output.label.is_some()
        || !matches!(
            output.expression,
            GroupingExpressionIr::Aggregation(ref aggregation)
                if matches!(aggregation.as_ref(), GroupingAggregationIr::Count)
        )
    {
        return None;
    }
    let count = |expression: &GroupingExpressionIr| {
        matches!(
            expression,
            GroupingExpressionIr::Aggregation(aggregation)
                if matches!(aggregation.as_ref(), GroupingAggregationIr::Count)
        )
    };
    let valid_order = match order {
        GroupingExpressionIr::Negative(expression) => {
            count(expression)
                || matches!(
                    expression.as_ref(),
                    GroupingExpressionIr::Attribute(value) if value == field
                )
        }
        expression => {
            count(expression)
                || matches!(expression, GroupingExpressionIr::Attribute(value) if value == field)
        }
    };
    valid_order.then_some(field.as_str())
}
