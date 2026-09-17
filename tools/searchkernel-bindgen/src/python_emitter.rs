use crate::normalized::*;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn generate(model: &Model, template: &str) -> Result<String, String> {
    let projection = Projection::new(model);
    let declarations = projection.declarations()?;
    let aliases = if projection.public_objects.contains(&model.error_payload) {
        format!(
            "PortableError = {}\n",
            short_type_name(&model.error_payload)
        )
    } else {
        String::new()
    };
    let standalone = standalone_encoders(model);
    let methods = model
        .operations
        .iter()
        .map(|operation| {
            format!(
                "    def {}(self, request):\n        return self._call(\"{}\", request)\n",
                operation.name, operation.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let descriptor = descriptor(model, &projection.public_objects)?;
    let exports = projection.exports(model);
    let generated = template
        .replace("# __MODEL_DECLARATIONS__", &declarations)
        .replace("# __MODEL_ALIASES__", &aliases)
        .replace("__SCHEMA_DESCRIPTOR__", &descriptor)
        .replace("# __STANDALONE_ENCODERS__", &standalone)
        .replace("# __CLIENT_METHODS__", &methods)
        .replace("__ALL__", &format!("{:?}", exports));
    let enum_inventory = model
        .enums
        .values()
        .map(|enumeration| {
            (
                short_type_name(&enumeration.name).to_owned(),
                enumeration
                    .variants
                    .iter()
                    .map(|variant| (py_snake(&variant.name), variant.value))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let enum_inventory = serde_json::to_string(&enum_inventory)
        .map_err(|error| format!("serialize generated enum inventory: {error}"))?;
    Ok(format!(
        "{generated}\n# BFBS-derived enum mappings; do not edit\n_GENERATED_ENUM_VARIANTS = {enum_inventory}\n"
    ))
}

struct Projection<'a> {
    model: &'a Model,
    wrappers: BTreeMap<String, String>,
    direct_unions: BTreeSet<String>,
    public_objects: BTreeSet<String>,
}

impl<'a> Projection<'a> {
    fn new(model: &'a Model) -> Self {
        let wrappers = model
            .objects
            .values()
            .filter_map(|object| {
                union_wrapper_fields(object).map(|(_, payload)| {
                    (
                        object.name.clone(),
                        payload.ty.name.clone().unwrap_or_default(),
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let root_unions = model
            .roots
            .iter()
            .map(|root| root.union.clone())
            .collect::<BTreeSet<_>>();
        let mut payloads = model
            .enums
            .values()
            .filter(|item| item.kind == "union" && !root_unions.contains(&item.name))
            .flat_map(|item| {
                item.variants
                    .iter()
                    .filter_map(|variant| variant.payload.clone())
            })
            .collect::<BTreeSet<_>>();
        let external = model
            .objects
            .values()
            .flat_map(|object| object.fields.iter())
            .filter(|field| field.ty.kind != "union")
            .filter_map(|field| object_reference(&field.ty))
            .collect::<BTreeSet<_>>();
        payloads.retain(|name| !external.contains(name));
        let entries = model
            .objects
            .values()
            .flat_map(|object| {
                object
                    .fields
                    .iter()
                    .filter_map(|field| field.ty.map_entry.clone())
            })
            .collect::<BTreeSet<_>>();
        let roots = model
            .roots
            .iter()
            .map(|root| root.table.clone())
            .collect::<BTreeSet<_>>();
        let optional = model
            .objects
            .values()
            .filter(|object| object.kind == "struct" && object.fields.len() == 1)
            .map(|object| object.name.clone())
            .collect::<BTreeSet<_>>();
        let public_objects = model
            .objects
            .keys()
            .filter(|name| {
                !payloads.contains(*name)
                    && !entries.contains(*name)
                    && !roots.contains(*name)
                    && !optional.contains(*name)
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut direct_unions = BTreeSet::new();
        for object in model
            .objects
            .values()
            .filter(|object| !wrappers.contains_key(&object.name))
        {
            for field in &object.fields {
                if field.ty.kind == "union"
                    && !root_unions.contains(field.ty.name.as_ref().unwrap_or(&String::new()))
                {
                    if let Some(name) = &field.ty.name {
                        direct_unions.insert(name.clone());
                    }
                }
            }
        }
        Self {
            model,
            wrappers,
            direct_unions,
            public_objects,
        }
    }

    fn declarations(&self) -> Result<String, String> {
        let mut output = String::new();
        for (object, union) in &self.wrappers {
            output.push_str(&self.union_declaration(object, union, false)?);
        }
        for union in &self.direct_unions {
            output.push_str(&self.union_declaration(union, union, true)?);
        }
        for name in &self.public_objects {
            if self.wrappers.contains_key(name) {
                continue;
            }
            output.push_str(&self.object_declaration(&self.model.objects[name], None)?);
        }
        for (path, alias) in &self.model.policy.response_aliases {
            let (owner, field) = path.rsplit_once('.').expect("validated alias");
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
        public: &str,
        union_name: &str,
        direct: bool,
    ) -> Result<String, String> {
        let union = &self.model.enums[union_name];
        let name = short_type_name(public);
        let default = self
            .model
            .policy
            .union_defaults
            .get(public)
            .map(|value| py_snake(value))
            .unwrap_or_else(|| "none".to_owned());
        let mut fields = BTreeMap::<String, String>::new();
        if direct {
            fields.insert("value".to_owned(), "Any".to_owned());
        }
        for variant in union.variants.iter().filter(|item| item.payload.is_some()) {
            let payload = &self.model.objects[variant.payload.as_deref().unwrap_or_default()];
            let path = format!("{}.{}", union.name, variant.name);
            if self.model.policy.unit_union_variants.contains(&path) || payload.fields.is_empty() {
                continue;
            }
            if direct
                || (self.public_objects.contains(&payload.name)
                    && !self.wrappers.contains_key(&payload.name))
                || (name == "Value" && payload.fields.len() == 1)
            {
                fields.insert("value".to_owned(), "Any".to_owned());
            } else {
                for field in payload
                    .fields
                    .iter()
                    .filter(|field| !is_union_tag(payload, field))
                {
                    fields.insert(field.name.clone(), self.python_type(&field.ty));
                }
            }
        }
        let mut output =
            format!("@dataclass(kw_only=True)\nclass {name}:\n    kind: str = {default:?}\n");
        for (field, ty) in fields {
            let default = if ty.starts_with("list[") {
                "dc_field(default_factory=list)"
            } else if ty.starts_with("dict[") {
                "dc_field(default_factory=dict)"
            } else {
                "None"
            };
            output.push_str(&format!("    {field}: Optional[{ty}] = {default}\n"));
        }
        if name == "Value" {
            for (method, kind) in [
                ("null", "null"),
                ("boolean", "bool"),
                ("int64", "int64"),
                ("uint64", "uint64"),
                ("float64", "float64"),
                ("string", "string"),
                ("bytes", "bytes"),
                ("list", "list"),
                ("object", "object"),
            ] {
                if method == "null" {
                    output.push_str(
                        "    @classmethod\n    def null(cls): return cls(kind=\"null\", value=None)\n",
                    );
                } else {
                    output.push_str(&format!("    @classmethod\n    def {method}(cls, value): return cls(kind={kind:?}, value=value)\n"));
                }
            }
        }
        output.push('\n');
        Ok(output)
    }

    fn object_declaration(
        &self,
        object: &NormalizedObject,
        alias: Option<&ResponseAlias>,
    ) -> Result<String, String> {
        let name = alias.map_or(short_type_name(&object.name), |value| value.name.as_str());
        let mut output = format!("@dataclass(kw_only=True)\nclass {name}:\n");
        let mut fields = object
            .fields
            .iter()
            .filter(|field| !is_union_tag(object, field))
            .collect::<Vec<_>>();
        fields.sort_by_key(|field| field.id);
        if fields.is_empty() {
            output.push_str("    pass\n\n");
            return Ok(output);
        }
        for field in fields {
            let ty = if let Some(response_alias) = self
                .model
                .policy
                .response_aliases
                .get(&format!("{}.{}", object.name, field.name))
            {
                response_alias.name.clone()
            } else {
                self.python_type(&field.ty)
            };
            let required = field.required
                || self
                    .model
                    .policy
                    .required_fields
                    .contains(&format!("{}.{}", object.name, field.name))
                || alias.is_some_and(|value| value.required_fields.contains(&field.name));
            let (annotation, default) = match field.ty.kind.as_str() {
                "vector" => (ty, "dc_field(default_factory=list)".to_owned()),
                "map" => (ty, "dc_field(default_factory=dict)".to_owned()),
                "scalar" if field.ty.base.as_deref() == Some("string") && required => {
                    (ty, "\"\"".to_owned())
                }
                "scalar" if field.ty.base.as_deref() == Some("bool") => (ty, "False".to_owned()),
                "scalar" if field.ty.base.as_deref() != Some("string") => (ty, "0".to_owned()),
                "enum" => {
                    let default = self.model.enums[field.ty.name.as_deref().unwrap_or_default()]
                        .variants
                        .iter()
                        .find(|item| item.value == field.default_integer)
                        .map(|item| py_snake(&item.name))
                        .unwrap_or_else(|| "unspecified".to_owned());
                    ("str".to_owned(), format!("{default:?}"))
                }
                _ if required => (format!("Optional[{ty}]"), "None".to_owned()),
                _ => (format!("Optional[{ty}]"), "None".to_owned()),
            };
            output.push_str(&format!("    {}: {annotation} = {default}\n", field.name));
        }
        output.push('\n');
        Ok(output)
    }

    fn python_type(&self, ty: &NormalizedType) -> String {
        match ty.kind.as_str() {
            "scalar" => match ty.base.as_deref() {
                Some("string") => "str",
                Some("bool") => "bool",
                Some("float" | "double") => "float",
                Some(_) => "int",
                None => "Any",
            }
            .to_owned(),
            "enum" => "str".to_owned(),
            "union" => short_type_name(ty.name.as_deref().unwrap_or_default()).to_owned(),
            "object" => {
                let name = ty.name.as_deref().unwrap_or_default();
                if self
                    .model
                    .objects
                    .get(name)
                    .is_some_and(|object| object.kind == "struct" && object.fields.len() == 1)
                {
                    self.python_type(&self.model.objects[name].fields[0].ty)
                } else {
                    short_type_name(name).to_owned()
                }
            }
            "vector" => format!("list[{}]", self.python_type(ty.element.as_deref().unwrap())),
            "map" => format!(
                "dict[str, {}]",
                self.python_type(ty.element.as_deref().unwrap())
            ),
            _ => "Any".to_owned(),
        }
    }

    fn exports(&self, model: &Model) -> Vec<String> {
        let mut result = self
            .public_objects
            .iter()
            .map(|name| short_type_name(name).to_owned())
            .collect::<BTreeSet<_>>();
        result.extend(
            self.direct_unions
                .iter()
                .map(|name| short_type_name(name).to_owned()),
        );
        result.extend(
            model
                .policy
                .response_aliases
                .values()
                .map(|alias| alias.name.clone()),
        );
        result.extend(
            [
                "PortableError",
                "Client",
                "ClientError",
                "ProtocolError",
                "encode_index_definition",
                "request_from_json",
                "request_from_json_text",
            ]
            .into_iter()
            .map(ToOwned::to_owned),
        );
        result.into_iter().collect()
    }
}

fn descriptor(model: &Model, public_objects: &BTreeSet<String>) -> Result<String, String> {
    let mut value = schema_model(model);
    let object = value
        .as_object_mut()
        .ok_or_else(|| "schema model is not an object".to_owned())?;
    object.insert("error_payload".to_owned(), json!(model.error_payload));
    object.insert("error_tag".to_owned(), json!(model.error_tag));
    object.insert("public_objects".to_owned(), json!(public_objects));
    object.insert(
        "policy".to_owned(),
        json!({
            "union_variants": model.policy.union_variants,
            "unit_union_variants": model.policy.unit_union_variants,
            "union_defaults": model.policy.union_defaults,
        }),
    );
    object.insert(
        "aliases".to_owned(),
        Value::Object(
            model
                .policy
                .response_aliases
                .values()
                .map(|alias| {
                    (
                        alias.name.clone(),
                        json!({"required_fields":alias.required_fields}),
                    )
                })
                .collect(),
        ),
    );
    object.insert(
        "field_aliases".to_owned(),
        Value::Object(
            model
                .policy
                .response_aliases
                .iter()
                .map(|(path, alias)| (path.clone(), json!(alias.name)))
                .collect(),
        ),
    );
    let standalone_type = model
        .policy
        .standalone_encoders
        .keys()
        .next()
        .cloned()
        .unwrap_or_default();
    object.insert("standalone_type".to_owned(), json!(standalone_type));
    serde_json::to_string(&value).map_err(|error| error.to_string())
}

fn standalone_encoders(model: &Model) -> String {
    model.policy.standalone_encoders.iter().map(|(name, function)| format!(
        "def {function}(value: {}) -> bytes:\n    b = _Builder(b\"\\0\\0\\0\\0\")\n    return b.finish(_encode_table(b, {name:?}, value))\n",
        short_type_name(name)
    )).collect::<Vec<_>>().join("\n")
}

fn object_reference(ty: &NormalizedType) -> Option<String> {
    if ty.kind == "object" {
        ty.name.clone()
    } else {
        ty.element.as_deref().and_then(object_reference)
    }
}

fn is_union_tag(object: &NormalizedObject, field: &NormalizedField) -> bool {
    field.ty.kind == "enum"
        && object.fields.iter().any(|payload| {
            payload.ty.kind == "union"
                && payload.ty.name == field.ty.name
                && field.id + 1 == payload.id
        })
}

fn py_snake(value: &str) -> String {
    value
        .chars()
        .enumerate()
        .fold(String::new(), |mut result, (index, character)| {
            if character.is_ascii_uppercase() && index != 0 {
                result.push('_');
            }
            result.push(character.to_ascii_lowercase());
            result
        })
}
