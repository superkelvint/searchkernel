use crate::normalized::{Model, NormalizedField, NormalizedType};
use std::collections::BTreeSet;

pub fn generate(model: &Model, template: &str) -> Result<String, String> {
    let start_marker = "    pub(crate) fn metadata(name: &str)";
    let end_marker = "    pub(crate) fn request_operation(tag: u8)";
    let start = template
        .find(start_marker)
        .ok_or_else(|| "JSON template schema metadata function is missing".to_owned())?;
    let end = template[start..]
        .find(end_marker)
        .map(|offset| start + offset)
        .ok_or_else(|| "JSON template schema dispatch marker is missing".to_owned())?;
    let inventory = render_inventory(model);
    let mut output = template.to_owned();
    output.replace_range(start..end, &inventory);
    Ok(format!("{}\n", output.trim_end()))
}

fn render_inventory(model: &Model) -> String {
    let mut output = String::new();
    output
        .push_str("    pub(crate) fn metadata(name: &str) -> Option<&'static [FieldMetadata]> {\n");
    output.push_str("        match name {\n");
    for object in model.objects.values() {
        output.push_str(&format!(
            "            {:?} => Some(&[\n",
            schema_name(&object.name)
        ));
        for field in &object.fields {
            render_metadata_field(&mut output, model, object.name.as_str(), field);
        }
        output.push_str("            ]),\n");
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    output.push_str(
        "    pub(crate) fn enum_values(name: &str) -> Option<&'static [&'static str]> {\n",
    );
    output.push_str("        match name {\n");
    for enumeration in model.enums.values() {
        output.push_str(&format!(
            "            {:?} => Some(&[\n",
            schema_name(&enumeration.name)
        ));
        for variant in &enumeration.variants {
            output.push_str(&format!("                {:?},\n", variant.name));
        }
        output.push_str("            ]),\n");
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");

    output.push_str("    pub(crate) fn fields(name: &str) -> Option<&'static [&'static str]> {\n");
    output.push_str("        match name {\n");
    for object in model.objects.values() {
        let mut names = object
            .fields
            .iter()
            .map(|field| public_field_name(model, &object.name, field))
            .collect::<BTreeSet<_>>();
        if let Some(aliases) = model.policy.json_field_aliases.get(&object.name) {
            names.extend(aliases.iter().cloned());
        }
        output.push_str(&format!(
            "            {:?} => Some(&[",
            schema_name(&object.name)
        ));
        if names.is_empty() {
            output.push_str("]),\n");
        } else {
            output.push('\n');
            for name in names {
                output.push_str(&format!("                {:?},\n", name));
            }
            output.push_str("            ]),\n");
        }
    }
    output.push_str("            _ => None,\n        }\n    }\n\n");
    output
}

fn render_metadata_field(
    output: &mut String,
    model: &Model,
    object_name: &str,
    field: &NormalizedField,
) {
    let path = format!("{object_name}.{}", field.name);
    let name = public_field_name(model, object_name, field);
    let schema = model
        .policy
        .json_field_schemas
        .get(&path)
        .cloned()
        .unwrap_or_else(|| metadata_schema(&field.ty));
    output.push_str(&format!(
        "                FieldMetadata {{ name: {:?}, schema: {:?} }},\n",
        name, schema
    ));
}

fn public_field_name(model: &Model, object_name: &str, field: &NormalizedField) -> String {
    model
        .policy
        .json_field_names
        .get(&format!("{object_name}.{}", field.name))
        .cloned()
        .unwrap_or_else(|| field.name.clone())
}

fn schema_name(name: &str) -> String {
    name.replace('.', "_")
}

fn metadata_schema(ty: &NormalizedType) -> String {
    match ty.kind.as_str() {
        "scalar" => match ty.base.as_deref() {
            Some("bool") => "boolean".to_owned(),
            Some("float" | "double") => "number".to_owned(),
            Some("string") => "string".to_owned(),
            Some(_) => "integer".to_owned(),
            None => "object".to_owned(),
        },
        "enum" | "object" => format!(
            "ref:#/definitions/{}",
            schema_name(ty.name.as_deref().unwrap_or("unknown"))
        ),
        "union" => "union".to_owned(),
        "vector" => format!(
            "array:{}",
            ty.element
                .as_deref()
                .map(metadata_schema)
                .unwrap_or_else(|| "object".to_owned())
        ),
        "map" => "object".to_owned(),
        _ => "object".to_owned(),
    }
}
