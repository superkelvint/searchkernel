use super::*;

/// Resolve a field projection using the index schema.  `None` means the
/// effective stored-field projection; `Some(empty)` remains an explicit empty
/// projection.
#[doc(hidden)]
pub fn resolve_projection(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<ProjectionIr> {
    resolve_projection_inner(context, fields, false)
}

pub(super) fn resolve_get_projection(
    context: &IndexContext,
    fields: Option<&[String]>,
) -> Result<ProjectionIr> {
    resolve_projection_inner(context, fields, true)
}

fn resolve_projection_inner(
    context: &IndexContext,
    fields: Option<&[String]>,
    allow_tensor: bool,
) -> Result<ProjectionIr> {
    let mut output = Vec::new();
    match fields {
        None => output.extend(
            context
                .definition()
                .query_fields()
                .filter(|field| {
                    field.capabilities().stored()
                        && (allow_tensor
                            || !matches!(
                                field.data_type(),
                                crate::model::DataTypeIr::Position
                                    | crate::model::DataTypeIr::Tensor { .. }
                            ))
                })
                .map(|field| field.name().clone()),
        ),
        Some(fields) => {
            let mut seen = std::collections::BTreeSet::new();
            for (index, name) in fields.iter().enumerate() {
                let field = context
                    .query_field_for_resolver(name, format!("projection.fields[{index}]"))?;
                if !field.capabilities().stored() {
                    return Err(invalid(
                        "field_not_stored",
                        Some(format!("projection.fields[{index}]")),
                        "projection requires a stored field",
                    ));
                }
                if !allow_tensor
                    && matches!(field.data_type(), crate::model::DataTypeIr::Tensor { .. })
                {
                    return Err(PortableError::unsupported(
                        "tensor_search_projection_not_supported",
                        Some(format!("projection.fields[{index}]")),
                        "tensor fields are readable through get but are not native summary fields",
                    ));
                }
                if !seen.insert(field.name().clone()) {
                    return Err(invalid(
                        "duplicate_projection_field",
                        Some(format!("projection.fields[{index}]")),
                        "projection fields must be unique",
                    ));
                }
                output.push(field.name().clone());
            }
        }
    }
    Ok(ProjectionIr { fields: output })
}
