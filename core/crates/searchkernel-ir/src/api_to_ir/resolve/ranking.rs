use super::*;

pub(super) fn validate_ranking_profile(context: &IndexContext, profile: &str) -> Result<()> {
    let configured = context
        .definition()
        .rank_profiles()
        .iter()
        .any(|candidate| candidate.name == profile);
    let generated_bm25 = profile == "bm25"
        && !context
            .definition()
            .rank_profiles()
            .iter()
            .any(|candidate| candidate.name == "bm25")
        && context
            .definition()
            .fields()
            .iter()
            .any(|field| field.kind() == FieldKind::Text);
    if profile == "default" || configured || generated_bm25 {
        return Ok(());
    }
    Err(invalid(
        "invalid_rank_profile",
        Some("ranking"),
        format!("rank profile '{profile}' is not configured for this index"),
    ))
}

pub(crate) fn resolve_sort(
    context: &IndexContext,
    input: &SortInput,
    path: &str,
) -> Result<SortIr> {
    let target = match &input.target {
        SortTargetInput::Field(name) => {
            let field = context.query_field_for_resolver(name, format!("{path}.field"))?;
            if !field.capabilities().sortable() {
                return Err(invalid(
                    "field_not_sortable",
                    Some(format!("{path}.field")),
                    "sort requires a sortable field",
                ));
            }
            if field.cardinality() != FieldCardinality::Single {
                return Err(invalid(
                    "invalid_capability",
                    Some(format!("{path}.field")),
                    "multi-valued fields cannot be sortable",
                ));
            }
            SortTargetIr::Field(field.name().clone())
        }
        SortTargetInput::Relevance => SortTargetIr::Relevance,
        SortTargetInput::RankFeature(feature) if !feature.is_empty() => {
            SortTargetIr::RankFeature(feature.clone())
        }
        SortTargetInput::RankFeature(_) => {
            return Err(invalid(
                "invalid_sort",
                Some(format!("{path}.target.feature")),
                "rank-feature sort requires a non-empty feature name",
            ));
        }
        SortTargetInput::Raw => {
            return Err(PortableError::unsupported(
                "raw_sort_not_supported",
                Some(format!("{path}.target")),
                "raw sort expressions must be normalized before entering resolved IR",
            ));
        }
    };
    Ok(SortIr {
        target,
        direction: input.direction,
        missing: input.missing.clone().unwrap_or(MissingPlacement::Last),
        annotations: input.annotations.clone(),
    })
}

pub(crate) fn resolve_sorts(context: &IndexContext, sorts: &[SortInput]) -> Result<Vec<SortIr>> {
    let mut seen = BTreeSet::new();
    sorts
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let value = resolve_sort(context, input, &format!("sort[{index}]"))?;
            if value.field().is_some() && !seen.insert(value.field().cloned()) {
                return Err(invalid(
                    "duplicate_sort_field",
                    Some(format!("sort[{index}].field")),
                    "sort fields must be unique",
                ));
            }
            Ok(value)
        })
        .collect()
}

pub(crate) fn resolve_highlighting(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<HighlightingIr> {
    let Some(fields) = fields else {
        return Ok(HighlightingIr::Disabled);
    };
    let mut output = Vec::new();
    let mut multi_valued_fields = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, name) in fields.iter().enumerate() {
        let field =
            context.query_field_for_resolver(name, format!("highlighting.fields[{index}]"))?;
        if field.kind() != FieldKind::Text || !field.capabilities().searchable() {
            return Err(invalid(
                "invalid_highlight",
                Some(format!("highlighting.fields[{index}]")),
                "highlighting requires a searchable text field",
            ));
        }
        if !field.capabilities().stored() {
            return Err(invalid(
                "field_not_stored",
                Some(format!("highlighting.fields[{index}]")),
                "highlighting requires a stored field",
            ));
        }
        if !seen.insert(field.name().clone()) {
            return Err(invalid(
                "duplicate_highlight_field",
                Some(format!("highlighting.fields[{index}]")),
                "highlight fields must be unique",
            ));
        }
        if field.cardinality() == FieldCardinality::Multi {
            multi_valued_fields.push(field.name().clone());
        }
        output.push(field.name().clone());
    }
    Ok(HighlightingIr::Requested {
        fields: output,
        multi_valued_fields,
    })
}
