use crate::normalized::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn generate(model: &Model, template: &str) -> Result<String, String> {
    let projection = Projection::new(model)?;
    let declarations = projection.declarations()?;
    let sync_methods = client_methods(model, false);
    let async_methods = client_methods(model, true);
    let codecs = projection.codecs()?;
    let error = short_type_name(&model.error_payload);
    Ok(template
        .replace("// __MODEL_DECLARATIONS__", &declarations)
        .replace("// __SYNC_METHODS__", &sync_methods)
        .replace("// __ASYNC_METHODS__", &async_methods)
        .replace("__PORTABLE_ERROR__", error)
        .replace("// __CODECS__", &codecs))
}

struct Projection<'a> {
    model: &'a Model,
    wrappers: BTreeMap<String, String>,
    direct_unions: BTreeSet<String>,
    hidden_payloads: BTreeSet<String>,
    hidden_entries: BTreeSet<String>,
    input_objects: BTreeSet<String>,
    response_objects: BTreeSet<String>,
}

impl<'a> Projection<'a> {
    fn new(model: &'a Model) -> Result<Self, String> {
        let mut wrappers = BTreeMap::new();
        for object in model.objects.values() {
            if let Some((_, payload)) = union_wrapper_fields(object) {
                wrappers.insert(
                    object.name.clone(),
                    payload.ty.name.clone().ok_or_else(|| {
                        format!("union wrapper {} has no union name", object.name)
                    })?,
                );
            }
        }
        let root_unions = model
            .roots
            .iter()
            .map(|root| &root.union)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut hidden_payloads = BTreeSet::new();
        for enumeration in model.enums.values().filter(|item| item.kind == "union") {
            if !root_unions.contains(&enumeration.name) {
                hidden_payloads.extend(
                    enumeration
                        .variants
                        .iter()
                        .filter_map(|variant| variant.payload.clone()),
                );
            }
        }
        let externally_referenced = model
            .objects
            .values()
            .flat_map(|object| object.fields.iter())
            .filter(|field| field.ty.kind != "union")
            .filter_map(|field| type_object_name(&field.ty))
            .collect::<BTreeSet<_>>();
        hidden_payloads.retain(|name| !externally_referenced.contains(name));
        let hidden_entries = model
            .objects
            .values()
            .flat_map(|object| object.fields.iter())
            .filter_map(|field| field.ty.map_entry.clone())
            .collect::<BTreeSet<_>>();
        let mut direct_unions = BTreeSet::new();
        for object in model.objects.values() {
            if wrappers.contains_key(&object.name) {
                continue;
            }
            for field in &object.fields {
                if field.ty.kind == "union" {
                    if let Some(name) = &field.ty.name {
                        if !root_unions.contains(name) {
                            direct_unions.insert(name.clone());
                        }
                    }
                }
            }
        }
        let mut input_objects = BTreeSet::new();
        for operation in &model.operations {
            visit_object(model, &operation.request, &mut input_objects);
        }
        for object in model.policy.standalone_encoders.keys() {
            visit_object(model, object, &mut input_objects);
        }
        let mut response_objects = BTreeSet::new();
        for operation in &model.operations {
            visit_object(model, &operation.response, &mut response_objects);
        }
        visit_object(model, &model.error_payload, &mut response_objects);
        Ok(Self {
            model,
            wrappers,
            direct_unions,
            hidden_payloads,
            hidden_entries,
            input_objects,
            response_objects,
        })
    }

    fn is_optional_wrapper(&self, name: &str) -> bool {
        self.model.objects.get(name).is_some_and(|object| {
            object.kind == "struct"
                && object.fields.len() == 1
                && matches!(object.fields[0].ty.kind.as_str(), "scalar" | "enum")
        })
    }

    fn is_root(&self, name: &str) -> bool {
        self.model.roots.iter().any(|root| root.table == name)
    }

    fn is_public_object(&self, name: &str) -> bool {
        !self.is_root(name)
            && !self.is_optional_wrapper(name)
            && !self.hidden_payloads.contains(name)
            && !self.hidden_entries.contains(name)
    }

    fn declarations(&self) -> Result<String, String> {
        let mut output = String::new();
        for enumeration in self
            .model
            .enums
            .values()
            .filter(|enumeration| enumeration.kind == "enum")
        {
            output.push_str(&ordinary_enum_declaration(enumeration));
        }
        for (object, union) in &self.wrappers {
            output.push_str(&self.union_declaration(object, union, false)?);
        }
        for union in &self.direct_unions {
            output.push_str(&self.union_declaration(union, union, true)?);
        }
        for object in self
            .model
            .objects
            .values()
            .filter(|object| self.is_public_object(&object.name))
            .filter(|object| !self.wrappers.contains_key(&object.name))
        {
            output.push_str(&self.object_declaration(object, None)?);
        }
        for (path, alias) in &self.model.policy.response_aliases {
            let (owner, field) = path.rsplit_once('.').expect("validated alias path");
            let target = self.model.objects[owner]
                .fields
                .iter()
                .find(|item| item.name == field)
                .and_then(|item| item.ty.name.as_ref())
                .expect("validated alias target");
            output.push_str(&self.object_declaration(&self.model.objects[target], Some(alias))?);
        }
        Ok(output)
    }

    fn union_declaration(
        &self,
        public_name: &str,
        union_name: &str,
        include_none: bool,
    ) -> Result<String, String> {
        let enumeration = self
            .model
            .enums
            .get(union_name)
            .ok_or_else(|| format!("missing union {union_name}"))?;
        let short = short_type_name(public_name);
        let default = self.model.policy.union_defaults.get(public_name);
        let derives_default = include_none || default.is_some();
        let mut output = format!(
            "    #[derive(Debug, Clone, PartialEq{})]\n    pub enum {short} {{\n",
            if derives_default { ", Default" } else { "" }
        );
        if include_none {
            output.push_str("        #[default]\n        None,\n");
        }
        for variant in enumeration
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            let name =
                public_union_variant_name(public_name, enumeration, variant, &self.model.policy);
            if default.is_some_and(|value| value == &name) {
                output.push_str("        #[default]\n");
            }
            let path = format!("{}.{}", enumeration.name, variant.name);
            let payload = &self.model.objects[variant.payload.as_deref().expect("payload")];
            if self.model.policy.unit_union_variants.contains(&path) || payload.fields.is_empty() {
                output.push_str(&format!("        {name},\n"));
            } else if self.is_public_object(&payload.name) {
                output.push_str(&format!(
                    "        {name}({}),\n",
                    short_type_name(&payload.name)
                ));
            } else if payload.fields.len() == 1
                && !self.model.policy.named_union_variants.contains(&path)
            {
                output.push_str(&format!(
                    "        {name}({}),\n",
                    self.model_type(&payload.name, &payload.fields[0], None)?
                ));
            } else {
                output.push_str(&format!("        {name} {{\n"));
                for field in payload
                    .fields
                    .iter()
                    .filter(|field| !is_union_tag(payload, field))
                {
                    output.push_str(&format!(
                        "            {}: {},\n",
                        rust_field(&field.name),
                        self.model_type(&payload.name, field, None)?
                    ));
                }
                output.push_str("        },\n");
            }
        }
        output.push_str("    }\n\n");
        Ok(output)
    }

    fn object_declaration(
        &self,
        object: &NormalizedObject,
        alias: Option<&ResponseAlias>,
    ) -> Result<String, String> {
        let name = alias.map_or_else(
            || short_type_name(&object.name),
            |value| value.name.as_str(),
        );
        let non_exhaustive = alias.is_some()
            || (self.response_objects.contains(&object.name)
                && !self.input_objects.contains(&object.name));
        let mut output = String::from("    #[derive(Debug, Clone, PartialEq, Default)]\n");
        if non_exhaustive {
            output.push_str("    #[non_exhaustive]\n");
        }
        if object
            .fields
            .iter()
            .all(|field| is_union_tag(object, field))
        {
            output.push_str(&format!("    pub struct {name};\n\n"));
            return Ok(output);
        }
        output.push_str(&format!("    pub struct {name} {{\n"));
        for field in object
            .fields
            .iter()
            .filter(|field| !is_union_tag(object, field))
        {
            output.push_str(&format!(
                "        pub {}: {},\n",
                rust_field(&field.name),
                self.model_type(&object.name, field, alias)?
            ));
        }
        output.push_str("    }\n\n");
        Ok(output)
    }

    fn model_type(
        &self,
        owner: &str,
        field: &NormalizedField,
        alias: Option<&ResponseAlias>,
    ) -> Result<String, String> {
        let required = field.required
            || self
                .model
                .policy
                .required_fields
                .contains(&format!("{owner}.{}", field.name))
            || alias.is_some_and(|value| value.required_fields.contains(&field.name));
        if let Some(response_alias) = self
            .model
            .policy
            .response_aliases
            .get(&format!("{owner}.{}", field.name))
        {
            let value = response_alias.name.clone();
            return Ok(if required {
                value
            } else {
                format!("Option<{value}>")
            });
        }
        let mut value = self.base_model_type(owner, &field.ty)?;
        let nullable = match field.ty.kind.as_str() {
            "scalar" => field.ty.base.as_deref() == Some("string") && !required,
            "object" => {
                let name = field.ty.name.as_deref().unwrap_or_default();
                self.is_optional_wrapper(name) || !required
            }
            _ => false,
        };
        if field.ty.kind == "object" {
            if let Some(target) = field.ty.name.as_ref() {
                if self
                    .model
                    .boxed_edges
                    .contains(&(owner.to_owned(), target.clone()))
                    && !self.is_optional_wrapper(target)
                {
                    value = format!("Box<{value}>");
                }
            }
        }
        Ok(if nullable && !value.starts_with("Option<") {
            format!("Option<{value}>")
        } else {
            value
        })
    }

    fn base_model_type(&self, owner: &str, ty: &NormalizedType) -> Result<String, String> {
        match ty.kind.as_str() {
            "scalar" => rust_scalar(ty.base.as_deref().unwrap_or_default())
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("unsupported scalar in {owner}")),
            "enum" | "union" => {
                Ok(short_type_name(ty.name.as_deref().unwrap_or_default()).to_owned())
            }
            "object" => {
                let name = ty.name.as_deref().unwrap_or_default();
                if self.is_optional_wrapper(name) {
                    self.base_model_type(name, &self.model.objects[name].fields[0].ty)
                } else {
                    Ok(short_type_name(name).to_owned())
                }
            }
            "vector" => Ok(format!(
                "Vec<{}>",
                self.base_model_type(
                    owner,
                    ty.element
                        .as_deref()
                        .ok_or_else(|| format!("vector in {owner} has no element"))?
                )?
            )),
            "map" => Ok(format!(
                "BTreeMap<String, {}>",
                self.base_model_type(
                    owner,
                    ty.element
                        .as_deref()
                        .ok_or_else(|| format!("map in {owner} has no value"))?
                )?
            )),
            other => Err(format!("unsupported normalized type {other} in {owner}")),
        }
    }

    fn codecs(&self) -> Result<String, String> {
        let mut output = String::new();
        for enumeration in self
            .model
            .enums
            .values()
            .filter(|enumeration| enumeration.kind == "enum")
        {
            output.push_str(&enum_conversions(enumeration));
        }
        for union in self
            .wrappers
            .values()
            .chain(self.direct_unions.iter())
            .collect::<BTreeSet<_>>()
        {
            if self.union_is_input(union) {
                output.push_str(&self.encode_union(union)?);
            }
            output.push_str(&self.decode_union(union)?);
        }
        for object in self.model.objects.values() {
            if !self.is_public_object(&object.name) || self.wrappers.contains_key(&object.name) {
                continue;
            }
            if self.input_objects.contains(&object.name) {
                output.push_str(&self.encode_table(object)?);
            }
            output.push_str(&self.decode_table(object, None)?);
        }
        for (path, alias) in &self.model.policy.response_aliases {
            let (owner, field) = path.rsplit_once('.').expect("validated alias path");
            let target = self.model.objects[owner]
                .fields
                .iter()
                .find(|item| item.name == field)
                .and_then(|item| item.ty.name.as_ref())
                .expect("validated alias target");
            output.push_str(&self.decode_table(&self.model.objects[target], Some(alias))?);
        }
        output.push_str(&self.envelopes_and_operations()?);
        Ok(output)
    }

    fn union_is_input(&self, union: &str) -> bool {
        self.model.objects.values().any(|object| {
            self.input_objects.contains(&object.name)
                && object.fields.iter().any(|field| {
                    matches!(field.ty.kind.as_str(), "union" | "enum")
                        && field.ty.name.as_deref() == Some(union)
                })
        })
    }

    fn encode_union(&self, union_name: &str) -> Result<String, String> {
        let union = &self.model.enums[union_name];
        let wrapper = self
            .wrappers
            .iter()
            .find_map(|(object, name)| (name == union_name).then_some(object.as_str()));
        let public = wrapper.unwrap_or(union_name);
        let public_short = short_type_name(public);
        let wire_short = short_type_name(union_name);
        let function = format!("encode_union_{}", flatc_snake(wire_short));
        let mut output = format!(
            "fn {function}<'a>(builder: &mut FlatBufferBuilder<'a>, value: &model::{public_short}) -> Result<(wire::{wire_short}, Option<flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>>), String> {{\n    match value {{\n"
        );
        if wrapper.is_none() {
            output.push_str(&format!(
                "        model::{public_short}::None => Ok((wire::{wire_short}::NONE, None)),\n"
            ));
        }
        for variant in union
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            let payload_name = variant.payload.as_deref().expect("payload");
            let payload = &self.model.objects[payload_name];
            let public_variant =
                public_union_variant_name(public, union, variant, &self.model.policy);
            let policy_path = format!("{}.{}", union.name, variant.name);
            let unit = self.model.policy.unit_union_variants.contains(&policy_path);
            let public_payload = self.is_public_object(&payload.name);
            let pattern = if unit || payload.fields.is_empty() {
                format!("model::{public_short}::{public_variant}")
            } else if public_payload
                || (payload.fields.len() == 1
                    && !self
                        .model
                        .policy
                        .named_union_variants
                        .contains(&policy_path))
            {
                format!("model::{public_short}::{public_variant}(field_0)")
            } else {
                format!(
                    "model::{public_short}::{public_variant} {{ {} }}",
                    payload
                        .fields
                        .iter()
                        .filter(|field| !is_union_tag(payload, field))
                        .map(|field| rust_field(&field.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            output.push_str(&format!("        {pattern} => {{\n"));
            let access = if public_payload {
                VariantAccess::PublicObject
            } else if payload.fields.len() == 1
                && !unit
                && !self
                    .model
                    .policy
                    .named_union_variants
                    .contains(&policy_path)
            {
                VariantAccess::Single
            } else if unit {
                VariantAccess::Unit
            } else {
                VariantAccess::Named
            };
            let payload_short = short_type_name(payload_name);
            if access == VariantAccess::PublicObject {
                output.push_str(&format!(
                    "            let payload = encode_table_{}(builder, field_0)?;\n",
                    flatc_snake(payload_short)
                ));
            } else {
                let (statements, arguments) = self.encode_fields(payload, access)?;
                output.push_str(&indent(&statements, 12));
                output.push_str(&format!(
                    "            let payload = wire::{payload_short}::create(builder, &wire::{payload_short}Args {{ {arguments} }});\n"
                ));
            }
            output.push_str(&format!(
                "            Ok((wire::{wire_short}::{}, Some(payload.as_union_value())))\n        }}\n",
                variant.name
            ));
        }
        output.push_str("    }\n}\n\n");
        if let Some(wrapper_name) = wrapper {
            let wrapper_short = short_type_name(wrapper_name);
            let object = &self.model.objects[wrapper_name];
            let (tag, payload) = union_wrapper_fields(object).expect("wrapper fields");
            output.push_str(&format!(
                "fn encode_table_{}<'a>(builder: &mut FlatBufferBuilder<'a>, value: &model::{wrapper_short}) -> Result<flatbuffers::WIPOffset<wire::{wrapper_short}<'a>>, String> {{\n    let ({}_type, {}) = {function}(builder, value)?;\n    Ok(wire::{wrapper_short}::create(builder, &wire::{wrapper_short}Args {{ {}_type, {} }}))\n}}\n\n",
                flatc_snake(wrapper_short), payload.name, payload.name, payload.name, payload.name
            ));
            let _ = tag;
        }
        Ok(output)
    }

    fn encode_table(&self, object: &NormalizedObject) -> Result<String, String> {
        let short = short_type_name(&object.name);
        let function = format!("encode_table_{}", flatc_snake(short));
        let (statements, arguments) = self.encode_fields(object, VariantAccess::Object)?;
        Ok(format!(
            "fn {function}<'a>(builder: &mut FlatBufferBuilder<'a>, value: &model::{short}) -> Result<flatbuffers::WIPOffset<wire::{short}<'a>>, String> {{\n{}    Ok(wire::{short}::create(builder, &wire::{short}Args {{ {arguments} }}))\n}}\n\n",
            indent(&statements, 4)
        ))
    }

    fn encode_fields(
        &self,
        object: &NormalizedObject,
        access: VariantAccess,
    ) -> Result<(String, String), String> {
        let mut statements = String::new();
        let mut arguments = Vec::new();
        for field in &object.fields {
            if is_union_tag(object, field) {
                continue;
            }
            let source = match access {
                VariantAccess::Object => format!("&value.{}", rust_field(&field.name)),
                VariantAccess::Single => "field_0".to_owned(),
                VariantAccess::Named => rust_field(&field.name),
                VariantAccess::Unit => String::new(),
                VariantAccess::PublicObject => {
                    unreachable!("public union payloads are encoded directly")
                }
            };
            if field.ty.kind == "union" {
                let union = field.ty.name.as_deref().unwrap_or_default();
                let function = format!("encode_union_{}", flatc_snake(short_type_name(union)));
                statements.push_str(&format!(
                    "let ({}_type, {}) = {function}(builder, {source})?;\n",
                    field.name, field.name
                ));
                arguments.push(format!("{}_type", field.name));
                arguments.push(field.name.clone());
                continue;
            }
            let (statement, argument) = self.encode_field(object, field, &source, access)?;
            statements.push_str(&statement);
            arguments.push(format!("{}: {argument}", rust_field(&field.name)));
        }
        Ok((statements, arguments.join(", ")))
    }

    fn encode_field(
        &self,
        owner: &NormalizedObject,
        field: &NormalizedField,
        source: &str,
        access: VariantAccess,
    ) -> Result<(String, String), String> {
        let name = rust_field(&field.name);
        let required = self.field_required(&owner.name, field, None);
        if access == VariantAccess::Unit {
            return self.encode_unit_field(field);
        }
        match field.ty.kind.as_str() {
            "scalar" => match field.ty.base.as_deref().unwrap_or_default() {
                "string" => {
                    let statement = if required {
                        format!("let {name} = Some(builder.create_string({source}));\n")
                    } else {
                        format!("let {name} = ({source}).as_deref().map(|item| builder.create_string(item));\n")
                    };
                    Ok((statement, name))
                }
                "float" | "double" => {
                    let finite = if field.ty.base.as_deref() == Some("float") {
                        "finite_f32"
                    } else {
                        "finite_f64"
                    };
                    Ok((
                        format!(
                            "let {name} = {finite}(*{source}, \"{}.{}\")?;\n",
                            short_type_name(&owner.name),
                            field.name
                        ),
                        name,
                    ))
                }
                _ => Ok((format!("let {name} = *{source};\n"), name)),
            },
            "enum" => {
                let enumeration = short_type_name(field.ty.name.as_deref().unwrap_or_default());
                Ok((
                    format!(
                        "let {name} = encode_{}(*{source});\n",
                        flatc_snake(enumeration)
                    ),
                    name,
                ))
            }
            "object" => {
                let target = field.ty.name.as_deref().unwrap_or_default();
                let target_short = short_type_name(target);
                if self.is_optional_wrapper(target) {
                    let inner = &self.model.objects[target].fields[0].ty;
                    let conversion = if inner.kind == "enum" {
                        format!(
                            "encode_{}(item)",
                            flatc_snake(short_type_name(inner.name.as_deref().unwrap_or_default()))
                        )
                    } else {
                        "item".to_owned()
                    };
                    Ok((
                        format!("let {name}_storage = (*{source}).map(|item| wire::{target_short}::new({conversion}));\nlet {name} = {name}_storage.as_ref();\n"),
                        name,
                    ))
                } else {
                    let function = format!("encode_table_{}", flatc_snake(target_short));
                    let statement = if required {
                        format!("let {name} = Some({function}(builder, {source})?);\n")
                    } else {
                        format!("let {name} = ({source}).as_ref().map(|item| {function}(builder, item)).transpose()?;\n")
                    };
                    Ok((statement, name))
                }
            }
            "vector" => self.encode_vector(field, source),
            "map" => self.encode_map(field, source),
            other => Err(format!("unsupported encode field type {other}")),
        }
    }

    fn encode_unit_field(&self, field: &NormalizedField) -> Result<(String, String), String> {
        let name = rust_field(&field.name);
        if field.ty.kind == "enum" {
            let enumeration = &self.model.enums[field.ty.name.as_deref().unwrap_or_default()];
            let chosen = enumeration
                .variants
                .iter()
                .find(|variant| variant.name != "Unspecified" && variant.name != "NONE")
                .unwrap_or(&enumeration.variants[0]);
            return Ok((
                format!(
                    "let {name} = wire::{}({});\n",
                    short_type_name(&enumeration.name),
                    chosen.value
                ),
                name,
            ));
        }
        Err(format!(
            "unit union payload field {} is not an enum",
            field.name
        ))
    }

    fn encode_vector(
        &self,
        field: &NormalizedField,
        source: &str,
    ) -> Result<(String, String), String> {
        let name = rust_field(&field.name);
        let element = field
            .ty
            .element
            .as_deref()
            .ok_or_else(|| "vector has no element".to_owned())?;
        let mut statement =
            format!("let mut {name}_items = Vec::with_capacity(({source}).len());\n");
        match element.kind.as_str() {
            "scalar" if element.base.as_deref() == Some("string") => {
                statement.push_str(&format!(
                    "for item in {source} {{ {name}_items.push(builder.create_string(item)); }}\n"
                ));
            }
            "scalar" => {
                statement.push_str(&format!("{name}_items.extend({source}.iter().copied());\n"));
            }
            "enum" => {
                statement.push_str(&format!(
                    "for item in {source} {{ {name}_items.push(encode_{}(*item)); }}\n",
                    flatc_snake(short_type_name(element.name.as_deref().unwrap_or_default()))
                ));
            }
            "object" => {
                let target = short_type_name(element.name.as_deref().unwrap_or_default());
                statement.push_str(&format!(
                    "for item in {source} {{ {name}_items.push(encode_table_{}(builder, item)?); }}\n",
                    flatc_snake(target)
                ));
            }
            other => return Err(format!("unsupported vector element {other}")),
        }
        statement.push_str(&format!(
            "let {name} = if ({source}).is_empty() {{ None }} else {{ Some(builder.create_vector(&{name}_items)) }};\n"
        ));
        Ok((statement, name))
    }

    fn encode_map(
        &self,
        field: &NormalizedField,
        source: &str,
    ) -> Result<(String, String), String> {
        let name = rust_field(&field.name);
        let entry = field
            .ty
            .map_entry
            .as_deref()
            .ok_or_else(|| "map has no entry".to_owned())?;
        let entry_short = short_type_name(entry);
        let entry_object = &self.model.objects[entry];
        let key_field = entry_object
            .fields
            .iter()
            .find(|item| item.key)
            .ok_or_else(|| format!("map entry {entry} has no key"))?;
        let value_field = entry_object
            .fields
            .iter()
            .find(|item| !item.key)
            .ok_or_else(|| format!("map entry {entry} has no value"))?;
        let element = field
            .ty
            .element
            .as_deref()
            .ok_or_else(|| "map has no element".to_owned())?;
        let value_encode = match element.kind.as_str() {
            "scalar" if element.base.as_deref() == Some("string") => {
                "Some(builder.create_string(item))".to_owned()
            }
            "object" => format!(
                "Some(encode_table_{}(builder, item)?)",
                flatc_snake(short_type_name(element.name.as_deref().unwrap_or_default()))
            ),
            "enum" => format!(
                "encode_{}(*item)",
                flatc_snake(short_type_name(element.name.as_deref().unwrap_or_default()))
            ),
            other => return Err(format!("unsupported map value {other}")),
        };
        let statement = format!(
            "let mut {name}_items = Vec::with_capacity(({source}).len());\nfor (key, item) in {source} {{\n    let map_key = Some(builder.create_string(key));\n    let map_value = {value_encode};\n    {name}_items.push(wire::{entry_short}::create(builder, &wire::{entry_short}Args {{ {}: map_key, {}: map_value }}));\n}}\nlet {name} = Some(builder.create_vector(&{name}_items));\n",
            rust_field(&key_field.name), rust_field(&value_field.name)
        );
        Ok((statement, name))
    }

    fn decode_union(&self, union_name: &str) -> Result<String, String> {
        let union = &self.model.enums[union_name];
        let wrapper = self
            .wrappers
            .iter()
            .find_map(|(object, name)| (name == union_name).then_some(object.as_str()));
        let public = wrapper.unwrap_or(union_name);
        let public_short = short_type_name(public);
        let wire_short = short_type_name(union_name);
        let function = format!("decode_union_{}", flatc_snake(wire_short));
        let owner_wire = wrapper.map(short_type_name).unwrap_or("PARENT");
        let (tag_expr, accessor_prefix, signature) = if let Some(wrapper_name) = wrapper {
            let wrapper_short = short_type_name(wrapper_name);
            let (_, payload) =
                union_wrapper_fields(&self.model.objects[wrapper_name]).expect("wrapper");
            (
                format!("value.{}_type()", payload.name),
                format!("value.{}_as_", payload.name),
                format!("value: wire::{wrapper_short}<'_>"),
            )
        } else {
            return Ok(String::new());
        };
        let mut output = format!(
            "fn {function}({signature}) -> Result<model::{public_short}, DecodeError> {{\n    match {tag_expr} {{\n"
        );
        for variant in union
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            let payload_name = variant.payload.as_deref().expect("payload");
            let payload = &self.model.objects[payload_name];
            let public_variant =
                public_union_variant_name(public, union, variant, &self.model.policy);
            let path = format!("{}.{}", union.name, variant.name);
            let accessor = format!("{}{}()", accessor_prefix, flatc_snake(&variant.name));
            output.push_str(&format!("        wire::{wire_short}::{} => {{\n            let payload = {accessor}.ok_or_else(|| DecodeError::Protocol(\"missing union payload\".to_owned()))?;\n", variant.name));
            if self.model.policy.unit_union_variants.contains(&path) || payload.fields.is_empty() {
                output.push_str(&format!(
                    "            Ok(model::{public_short}::{public_variant})\n"
                ));
            } else if self.is_public_object(&payload.name) {
                output.push_str(&format!(
                    "            Ok(model::{public_short}::{public_variant}(decode_table_{}(payload)?))\n",
                    flatc_snake(short_type_name(&payload.name))
                ));
            } else if payload.fields.len() == 1
                && !self.model.policy.named_union_variants.contains(&path)
            {
                let expression =
                    self.decode_field_expr(payload, &payload.fields[0], "payload", None)?;
                output.push_str(&format!(
                    "            Ok(model::{public_short}::{public_variant}({expression}))\n"
                ));
            } else {
                output.push_str(&format!(
                    "            Ok(model::{public_short}::{public_variant} {{\n"
                ));
                for field in payload
                    .fields
                    .iter()
                    .filter(|field| !is_union_tag(payload, field))
                {
                    let expression = self.decode_field_expr(payload, field, "payload", None)?;
                    output.push_str(&format!(
                        "                {}: {expression},\n",
                        rust_field(&field.name)
                    ));
                }
                output.push_str("            })\n");
            }
            output.push_str("        }\n");
        }
        output.push_str(&format!("        _ => Err(DecodeError::Protocol(\"unknown {} discriminator\".to_owned())),\n    }}\n}}\n\n", flatc_snake(owner_wire)));
        Ok(output)
    }

    fn decode_table(
        &self,
        object: &NormalizedObject,
        alias: Option<&ResponseAlias>,
    ) -> Result<String, String> {
        let wire_short = short_type_name(&object.name);
        let model_short = alias.map_or(wire_short, |value| value.name.as_str());
        let function = if alias.is_some() {
            format!("decode_alias_{}", flatc_snake(model_short))
        } else if self.wrappers.contains_key(&object.name) {
            format!(
                "decode_union_{}",
                flatc_snake(short_type_name(&self.wrappers[&object.name]))
            )
        } else {
            format!("decode_table_{}", flatc_snake(wire_short))
        };
        if self.wrappers.contains_key(&object.name) {
            return Ok(String::new());
        }
        let mut output = format!("fn {function}(value: wire::{wire_short}<'_>) -> Result<model::{model_short}, DecodeError> {{\n    Ok(model::{model_short}");
        let fields = object
            .fields
            .iter()
            .filter(|field| !is_union_tag(object, field))
            .collect::<Vec<_>>();
        if fields.is_empty() {
            output.push_str(")\n}\n\n");
            return Ok(output);
        }
        output.push_str(" {\n");
        for field in fields {
            let expression = self.decode_field_expr(object, field, "value", alias)?;
            output.push_str(&format!(
                "        {}: {expression},\n",
                rust_field(&field.name)
            ));
        }
        output.push_str("    })\n}\n\n");
        Ok(output)
    }

    fn decode_field_expr(
        &self,
        owner: &NormalizedObject,
        field: &NormalizedField,
        source: &str,
        alias: Option<&ResponseAlias>,
    ) -> Result<String, String> {
        let accessor = format!("{source}.{}()", rust_field(&field.name));
        let required = self.field_required(&owner.name, field, alias);
        if let Some(response_alias) = self
            .model
            .policy
            .response_aliases
            .get(&format!("{}.{}", owner.name, field.name))
        {
            let function = format!("decode_alias_{}", flatc_snake(&response_alias.name));
            return Ok(if required {
                format!("{function}({accessor}.ok_or_else(|| DecodeError::Protocol(\"missing response field {}.{}\".to_owned()))?)?", short_type_name(&owner.name), field.name)
            } else {
                format!("{accessor}.map({function}).transpose()?")
            });
        }
        match field.ty.kind.as_str() {
            "scalar" => match field.ty.base.as_deref().unwrap_or_default() {
                "string" if field.required => Ok(format!("{accessor}.to_owned()")),
                "string" if required => Ok(format!(
                    "required_string({accessor}, \"{}.{}\")?",
                    short_type_name(&owner.name),
                    field.name
                )),
                "string" => Ok(format!("{accessor}.map(ToOwned::to_owned)")),
                _ => Ok(accessor),
            },
            "enum" => Ok(format!(
                "decode_{}({accessor})?",
                flatc_snake(short_type_name(
                    field.ty.name.as_deref().unwrap_or_default()
                ))
            )),
            "object" => {
                let target = field.ty.name.as_deref().unwrap_or_default();
                if self.is_optional_wrapper(target) {
                    let inner = &self.model.objects[target].fields[0].ty;
                    let conversion = if inner.kind == "enum" {
                        format!(
                            "decode_{}(item.value())",
                            flatc_snake(short_type_name(inner.name.as_deref().unwrap_or_default()))
                        )
                    } else {
                        "Ok(item.value())".to_owned()
                    };
                    Ok(format!("{accessor}.map(|item| {conversion}).transpose()?"))
                } else {
                    let function = if self.wrappers.contains_key(target) {
                        format!(
                            "decode_union_{}",
                            flatc_snake(short_type_name(&self.wrappers[target]))
                        )
                    } else {
                        format!("decode_table_{}", flatc_snake(short_type_name(target)))
                    };
                    let decoded = if required {
                        format!("{function}({accessor}.ok_or_else(|| DecodeError::Protocol(\"missing response field {}.{}\".to_owned()))?)?", short_type_name(&owner.name), field.name)
                    } else {
                        format!("{accessor}.map({function}).transpose()?")
                    };
                    if self
                        .model
                        .boxed_edges
                        .contains(&(owner.name.clone(), target.to_owned()))
                    {
                        Ok(if required {
                            format!("Box::new({decoded})")
                        } else {
                            format!("{decoded}.map(Box::new)")
                        })
                    } else {
                        Ok(decoded)
                    }
                }
            }
            "vector" => self.decode_vector_expr(field, &accessor),
            "map" => self.decode_map_expr(field, &accessor),
            "union" => self.decode_direct_union_expr(field, source),
            other => Err(format!("unsupported decode field type {other}")),
        }
    }

    fn decode_vector_expr(
        &self,
        field: &NormalizedField,
        accessor: &str,
    ) -> Result<String, String> {
        let element = field
            .ty
            .element
            .as_deref()
            .ok_or_else(|| "vector has no element".to_owned())?;
        let mapped = match element.kind.as_str() {
            "scalar" if element.base.as_deref() == Some("string") => "item.to_owned()".to_owned(),
            "scalar" => "item".to_owned(),
            "enum" => format!(
                "decode_{}(item)?",
                flatc_snake(short_type_name(element.name.as_deref().unwrap_or_default()))
            ),
            "object" => {
                let target = element.name.as_deref().unwrap_or_default();
                let function = if self.wrappers.contains_key(target) {
                    format!(
                        "decode_union_{}",
                        flatc_snake(short_type_name(&self.wrappers[target]))
                    )
                } else {
                    format!("decode_table_{}", flatc_snake(short_type_name(target)))
                };
                format!("{function}(item)?")
            }
            other => return Err(format!("unsupported vector decode element {other}")),
        };
        Ok(format!("match {accessor} {{ Some(items) => {{ let mut result = Vec::with_capacity(items.len()); for item in items.iter() {{ result.push({mapped}); }} result }}, None => Vec::new() }}"))
    }

    fn decode_direct_union_expr(
        &self,
        field: &NormalizedField,
        source: &str,
    ) -> Result<String, String> {
        let union_name = field
            .ty
            .name
            .as_deref()
            .ok_or_else(|| "direct union has no name".to_owned())?;
        let union = &self.model.enums[union_name];
        let public_short = short_type_name(union_name);
        let wire_short = short_type_name(union_name);
        let mut output = format!("{{ match {source}.{}_type() {{ ", field.name);
        output.push_str(&format!(
            "wire::{wire_short}::NONE => Ok(model::{public_short}::None), "
        ));
        for variant in union
            .variants
            .iter()
            .filter(|variant| variant.payload.is_some())
        {
            let payload_name = variant.payload.as_deref().expect("payload");
            let payload = &self.model.objects[payload_name];
            let public_variant =
                public_union_variant_name(union_name, union, variant, &self.model.policy);
            let path = format!("{}.{}", union.name, variant.name);
            output.push_str(&format!(
                "wire::{wire_short}::{} => {{ let payload = {source}.{}_as_{}().ok_or_else(|| DecodeError::Protocol(\"missing union payload\".to_owned()))?; ",
                variant.name,
                field.name,
                flatc_snake(&variant.name)
            ));
            if self.model.policy.unit_union_variants.contains(&path) || payload.fields.is_empty() {
                output.push_str(&format!("Ok(model::{public_short}::{public_variant})"));
            } else if self.is_public_object(&payload.name) {
                output.push_str(&format!(
                    "Ok(model::{public_short}::{public_variant}(decode_table_{}(payload)?))",
                    flatc_snake(short_type_name(&payload.name))
                ));
            } else if payload.fields.len() == 1
                && !self.model.policy.named_union_variants.contains(&path)
            {
                let expression =
                    self.decode_field_expr(payload, &payload.fields[0], "payload", None)?;
                output.push_str(&format!(
                    "Ok(model::{public_short}::{public_variant}({expression}))"
                ));
            } else {
                output.push_str(&format!("Ok(model::{public_short}::{public_variant} {{ "));
                for payload_field in payload
                    .fields
                    .iter()
                    .filter(|item| !is_union_tag(payload, item))
                {
                    let expression =
                        self.decode_field_expr(payload, payload_field, "payload", None)?;
                    output.push_str(&format!(
                        "{}: {expression}, ",
                        rust_field(&payload_field.name)
                    ));
                }
                output.push_str("})");
            }
            output.push_str(" }, ");
        }
        output.push_str(&format!(
            "_ => Err(DecodeError::Protocol(\"unknown {} discriminator\".to_owned())) }} }}?",
            flatc_snake(public_short)
        ));
        Ok(output)
    }

    fn decode_map_expr(&self, field: &NormalizedField, accessor: &str) -> Result<String, String> {
        let entry = field
            .ty
            .map_entry
            .as_deref()
            .ok_or_else(|| "map has no entry".to_owned())?;
        let entry_object = &self.model.objects[entry];
        let key = entry_object
            .fields
            .iter()
            .find(|item| item.key)
            .ok_or_else(|| "map has no key".to_owned())?;
        let value = entry_object
            .fields
            .iter()
            .find(|item| !item.key)
            .ok_or_else(|| "map has no value".to_owned())?;
        let element = field
            .ty
            .element
            .as_deref()
            .ok_or_else(|| "map has no element".to_owned())?;
        let value_expr = match element.kind.as_str() {
            "scalar" if element.base.as_deref() == Some("string") => {
                format!("required_string(entry.{}(), \"map value\")?", value.name)
            }
            "object" => {
                let target = element.name.as_deref().unwrap_or_default();
                let function = if self.wrappers.contains_key(target) {
                    format!(
                        "decode_union_{}",
                        flatc_snake(short_type_name(&self.wrappers[target]))
                    )
                } else {
                    format!("decode_table_{}", flatc_snake(short_type_name(target)))
                };
                format!("{function}(entry.{}().ok_or_else(|| DecodeError::Protocol(\"missing map value\".to_owned()))?)?", value.name)
            }
            "enum" => format!(
                "decode_{}(entry.{}())?",
                flatc_snake(short_type_name(element.name.as_deref().unwrap_or_default())),
                value.name
            ),
            other => return Err(format!("unsupported map decode value {other}")),
        };
        let key_expr = if key.required {
            format!("entry.{}().to_owned()", key.name)
        } else {
            format!("required_string(entry.{}(), \"map key\")?", key.name)
        };
        Ok(format!("match {accessor} {{ Some(items) => {{ let mut result = BTreeMap::new(); for entry in items.iter() {{ result.insert({key_expr}, {value_expr}); }} result }}, None => BTreeMap::new() }}"))
    }

    fn field_required(
        &self,
        owner: &str,
        field: &NormalizedField,
        alias: Option<&ResponseAlias>,
    ) -> bool {
        field.required
            || self
                .model
                .policy
                .required_fields
                .contains(&format!("{owner}.{}", field.name))
            || alias.is_some_and(|value| value.required_fields.contains(&field.name))
    }

    fn envelopes_and_operations(&self) -> Result<String, String> {
        let request_root = self
            .model
            .roots
            .iter()
            .find(|root| root.side == "request")
            .ok_or_else(|| "missing request root".to_owned())?;
        let response_root = self
            .model
            .roots
            .iter()
            .find(|root| root.side == "response")
            .ok_or_else(|| "missing response root".to_owned())?;
        let request_root_short = short_type_name(&request_root.table);
        let response_root_short = short_type_name(&response_root.table);
        let request_union = short_type_name(&request_root.union);
        let response_union = short_type_name(&response_root.union);
        let request_identifier = request_root
            .identifier
            .as_deref()
            .ok_or_else(|| "request root has no file identifier".to_owned())?;
        let response_identifier = response_root
            .identifier
            .as_deref()
            .ok_or_else(|| "response root has no file identifier".to_owned())?;
        let mut output = format!(
            "fn finish_request(builder: &mut FlatBufferBuilder<'_>, payload_type: wire::{request_union}, payload: flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>) -> Vec<u8> {{\n    let envelope = wire::{request_root_short}::create(builder, &wire::{request_root_short}Args {{ protocol_version: 1, payload_type, payload: Some(payload) }});\n    builder.finish(envelope, Some(\"{request_identifier}\"));\n    builder.finished_data().to_vec()\n}}\n\n"
        );
        for operation in &self.model.operations {
            let request = short_type_name(&operation.request);
            output.push_str(&format!(
                "fn encode_operation_{}(value: model::{request}) -> Result<Vec<u8>, String> {{\n    let mut builder = FlatBufferBuilder::new();\n    let payload = encode_table_{}(&mut builder, &value)?;\n    Ok(finish_request(&mut builder, wire::{request_union}::{}, payload.as_union_value()))\n}}\n\n",
                operation.name,
                flatc_snake(request),
                operation.request_variant
            ));
        }
        let error_short = short_type_name(&self.model.error_payload);
        output.push_str(&format!(
            "fn response_envelope(bytes: &[u8], expected_tag: i64, operation: &str) -> Result<wire::{response_root_short}<'_>, DecodeError> {{\n    if bytes.len() < 8 || bytes[4..8] != *b\"{response_identifier}\" {{\n        return Err(DecodeError::Protocol(\"missing {response_identifier} file identifier\".to_owned()));\n    }}\n    let value = root::<wire::{response_root_short}<'_>>(bytes).map_err(|error| DecodeError::Protocol(error.to_string()))?;\n    if value.protocol_version() != 1 {{\n        return Err(DecodeError::Protocol(\"unsupported protocol version\".to_owned()));\n    }}\n    if i64::from(value.payload_type().0) == {} {{\n        let error = value.payload_as_{}().ok_or_else(|| DecodeError::Protocol(\"error response payload is missing\".to_owned()))?;\n        return Err(DecodeError::Portable(decode_table_{}(error)?));\n    }}\n    if i64::from(value.payload_type().0) != expected_tag {{\n        return Err(DecodeError::Protocol(format!(\"response variant does not match operation {{operation}}\")));\n    }}\n    Ok(value)\n}}\n\n",
            self.model.error_tag,
            flatc_snake(&self.model.error_variant),
            flatc_snake(error_short)
        ));
        for operation in &self.model.operations {
            let response = short_type_name(&operation.response);
            output.push_str(&format!(
                "impl DecodeResponse for model::{response} {{\n    fn decode(operation: &str, bytes: &[u8]) -> Result<Self, DecodeError> {{\n        let envelope = response_envelope(bytes, {}, operation)?;\n        let value = envelope.payload_as_{}().ok_or_else(|| DecodeError::Protocol(\"response payload is missing\".to_owned()))?;\n        decode_table_{}(value)\n    }}\n}}\n\n",
                operation.response_tag,
                flatc_snake(&operation.response_variant),
                flatc_snake(response)
            ));
        }
        output = output
            .replace(
                "fn response_envelope(bytes: &[u8]",
                "fn response_envelope<'a>(bytes: &'a [u8]",
            )
            .replace(
                &format!("Result<wire::{response_root_short}<'_>, DecodeError>"),
                &format!("Result<wire::{response_root_short}<'a>, DecodeError>"),
            )
            .replace(
                &format!("root::<wire::{response_root_short}<'_>>"),
                &format!("root::<wire::{response_root_short}<'a>>"),
            );
        let _ = response_union;
        Ok(output)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VariantAccess {
    Object,
    Single,
    Named,
    Unit,
    PublicObject,
}

fn type_object_name(ty: &NormalizedType) -> Option<String> {
    if ty.kind == "object" {
        ty.name.clone()
    } else {
        ty.element.as_deref().and_then(type_object_name)
    }
}

fn visit_object(model: &Model, name: &str, visited: &mut BTreeSet<String>) {
    if !visited.insert(name.to_owned()) {
        return;
    }
    let Some(object) = model.objects.get(name) else {
        return;
    };
    for field in &object.fields {
        visit_type(model, &field.ty, visited);
    }
}

fn visit_type(model: &Model, ty: &NormalizedType, visited: &mut BTreeSet<String>) {
    if ty.kind == "object" {
        if let Some(name) = &ty.name {
            visit_object(model, name, visited);
        }
    } else if ty.kind == "union" {
        if let Some(enumeration) = ty.name.as_ref().and_then(|name| model.enums.get(name)) {
            for payload in enumeration
                .variants
                .iter()
                .filter_map(|variant| variant.payload.as_ref())
            {
                visit_object(model, payload, visited);
            }
        }
    }
    if let Some(element) = &ty.element {
        visit_type(model, element, visited);
    }
}

fn ordinary_enum_declaration(enumeration: &NormalizedEnum) -> String {
    let name = short_type_name(&enumeration.name);
    let default = enumeration
        .variants
        .iter()
        .position(|variant| variant.name == "Unspecified")
        .unwrap_or(0);
    let mut output = format!(
        "    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]\n    pub enum {name} {{\n"
    );
    for (index, variant) in enumeration.variants.iter().enumerate() {
        if index == default {
            output.push_str("        #[default]\n");
        }
        output.push_str(&format!("        {},\n", rust_variant_name(&variant.name)));
    }
    output.push_str("    }\n\n");
    output
}

fn enum_conversions(enumeration: &NormalizedEnum) -> String {
    let name = short_type_name(&enumeration.name);
    let function = flatc_snake(name);
    let mut output = format!(
        "fn encode_{function}(value: model::{name}) -> wire::{name} {{\n    match value {{\n"
    );
    for variant in &enumeration.variants {
        output.push_str(&format!(
            "        model::{name}::{} => wire::{name}({}),\n",
            rust_variant_name(&variant.name),
            variant.value
        ));
    }
    output.push_str("    }\n}\n\n");
    output.push_str(&format!("fn decode_{function}(value: wire::{name}) -> Result<model::{name}, DecodeError> {{\n    match i64::from(value.0) {{\n"));
    for variant in &enumeration.variants {
        output.push_str(&format!(
            "        {} => Ok(model::{name}::{}),\n",
            variant.value,
            rust_variant_name(&variant.name)
        ));
    }
    output.push_str(&format!("        _ => Err(DecodeError::Protocol(\"unknown {function}\".to_owned())),\n    }}\n}}\n\n"));
    output
}

fn client_methods(model: &Model, asynchronous: bool) -> String {
    let mut output = String::new();
    for operation in &model.operations {
        let request = short_type_name(&operation.request);
        let response = short_type_name(&operation.response);
        output.push_str(&format!(
            "    pub {}fn {}(&self, request: model::{request}) -> Result<model::{response}, ClientError<T::Error>> {{ self.call(\"{}\", encode_operation_{}(request)){} }}\n",
            if asynchronous { "async " } else { "" },
            operation.name,
            operation.name,
            operation.name,
            if asynchronous { ".await" } else { "" }
        ));
    }
    output
}

fn is_union_tag(object: &NormalizedObject, field: &NormalizedField) -> bool {
    field.ty.kind == "enum"
        && object.fields.iter().any(|payload| {
            payload.ty.kind == "union"
                && payload.ty.name == field.ty.name
                && field.id + 1 == payload.id
        })
}

fn rust_scalar(base: &str) -> Option<&'static str> {
    match base {
        "bool" => Some("bool"),
        "byte" => Some("i8"),
        "ubyte" => Some("u8"),
        "short" => Some("i16"),
        "ushort" => Some("u16"),
        "int" => Some("i32"),
        "uint" => Some("u32"),
        "long" => Some("i64"),
        "ulong" => Some("u64"),
        "float" => Some("f32"),
        "double" => Some("f64"),
        "string" => Some("String"),
        _ => None,
    }
}

fn rust_field(name: &str) -> String {
    match name {
        "type" | "match" | "ref" | "self" | "crate" | "super" | "mod" | "enum" | "struct"
        | "union" | "fn" | "pub" | "use" | "where" | "loop" | "move" | "async" | "await"
        | "dyn" => format!("r#{name}"),
        _ => name.to_owned(),
    }
}

fn flatc_snake(value: &str) -> String {
    let mut output = String::new();
    let chars = value.chars().collect::<Vec<_>>();
    for (index, current) in chars.iter().copied().enumerate() {
        if current == '.' {
            if !output.ends_with('_') {
                output.push('_');
            }
            continue;
        }
        if current == '_' {
            if !output.ends_with('_') {
                output.push('_');
            }
            continue;
        }
        let previous = index.checked_sub(1).map(|item| chars[item]);
        let next = chars.get(index + 1).copied();
        let boundary = current.is_ascii_uppercase()
            && previous.is_some_and(|item| item.is_ascii_lowercase() || item.is_ascii_digit())
            || current.is_ascii_uppercase()
                && previous.is_some_and(|item| item.is_ascii_uppercase())
                && next.is_some_and(|item| item.is_ascii_lowercase())
                && index != 1
            || current.is_ascii_digit()
                && previous.is_some_and(|item| item.is_ascii_alphabetic())
                && !previous.is_some_and(|item| item == 'V');
        if boundary && !output.is_empty() && !output.ends_with('_') {
            output.push('_');
        }
        output.push(current.to_ascii_lowercase());
    }
    output
}

fn indent(value: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::flatc_snake;

    #[test]
    fn flatc_style_snake_case_handles_numeric_names() {
        assert_eq!(flatc_snake("Int64Value"), "int_64_value");
        assert_eq!(flatc_snake("UInt64Value"), "uint_64_value");
        assert_eq!(
            flatc_snake("SearchKernel_V1_InsertRequest"),
            "search_kernel_v1_insert_request"
        );
        assert_eq!(flatc_snake("Bm25Ranking"), "bm_25_ranking");
    }
}
