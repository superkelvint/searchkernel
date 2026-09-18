use flatbuffers_reflection::reflection::{BaseType, Schema, Type};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const CLIENT_POLICY: &str = include_str!("../client-policy.json");

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedType {
    pub kind: String,
    pub base: Option<String>,
    pub name: Option<String>,
    pub element: Option<Box<NormalizedType>>,
    pub map_key: Option<Box<NormalizedType>>,
    pub map_entry: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedField {
    pub name: String,
    pub id: u16,
    pub offset: u16,
    pub ty: NormalizedType,
    pub default_integer: i64,
    pub default_real: f64,
    pub required: bool,
    pub optional: bool,
    pub key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedObject {
    pub name: String,
    pub kind: String,
    pub fields: Vec<NormalizedField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedVariant {
    pub name: String,
    pub value: i64,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedEnum {
    pub name: String,
    pub kind: String,
    pub underlying: String,
    pub variants: Vec<NormalizedVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Root {
    pub side: String,
    pub table: String,
    pub union: String,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    pub name: String,
    pub request: String,
    pub response: String,
    pub request_variant: String,
    pub request_tag: i64,
    pub response_variant: String,
    pub response_tag: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientPolicy {
    pub required_fields: BTreeSet<String>,
    pub response_aliases: BTreeMap<String, ResponseAlias>,
    pub union_variants: BTreeMap<String, String>,
    pub named_union_variants: BTreeSet<String>,
    pub unit_union_variants: BTreeSet<String>,
    pub union_defaults: BTreeMap<String, String>,
    pub standalone_encoders: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseAlias {
    pub name: String,
    pub required_fields: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct Model {
    pub objects: BTreeMap<String, NormalizedObject>,
    pub enums: BTreeMap<String, NormalizedEnum>,
    pub roots: Vec<Root>,
    pub operations: Vec<Operation>,
    pub recursive_edges: BTreeSet<(String, String)>,
    pub boxed_edges: BTreeSet<(String, String)>,
    pub policy: ClientPolicy,
    pub error_payload: String,
    #[allow(dead_code)]
    pub error_variant: String,
    #[allow(dead_code)]
    pub error_tag: i64,
    #[allow(dead_code)]
    pub protocol_version: u32,
}

pub fn normalize(request: &Schema<'_>, response: &Schema<'_>) -> Result<Model, String> {
    let mut objects = BTreeMap::new();
    let mut enums = BTreeMap::new();
    for schema in [request, response] {
        let (schema_objects, schema_enums) = normalize_schema(schema)?;
        merge(&mut objects, schema_objects, "object")?;
        merge(&mut enums, schema_enums, "enum")?;
    }
    validate_references(&objects, &enums)?;
    validate_union_fields(&objects)?;
    let request_root = normalize_root(request, "request")?;
    let response_root = normalize_root(response, "response")?;
    let (operations, error_payload, error_variant, error_tag) =
        pair_operations(&objects, &enums, &request_root, &response_root)?;
    let recursive_edges = find_recursive_edges(&objects, &enums, true);
    let boxed_edges = find_recursive_edges(&objects, &enums, false);
    let policy = load_policy(&objects, &enums)?;
    let protocol_version = protocol_version(&objects, &request_root, &response_root)?;
    Ok(Model {
        objects,
        enums,
        roots: vec![request_root, response_root],
        operations,
        recursive_edges,
        boxed_edges,
        policy,
        error_payload,
        error_variant,
        error_tag,
        protocol_version,
    })
}

fn protocol_version(
    objects: &BTreeMap<String, NormalizedObject>,
    request: &Root,
    response: &Root,
) -> Result<u32, String> {
    let version = |root: &Root| -> Result<u32, String> {
        let table = objects
            .get(&root.table)
            .ok_or_else(|| format!("missing {} envelope table", root.side))?;
        let field = table
            .fields
            .iter()
            .find(|field| field.name == "protocol_version")
            .ok_or_else(|| format!("{} envelope has no protocol_version", root.side))?;
        if field.ty.kind != "scalar" || field.ty.base.as_deref() != Some("uint") {
            return Err(format!(
                "{} envelope protocol_version must be uint",
                root.side
            ));
        }
        u32::try_from(field.default_integer)
            .map_err(|_| format!("{} envelope protocol_version is invalid", root.side))
    };
    let request_version = version(request)?;
    let response_version = version(response)?;
    if request_version != response_version {
        return Err("request and response protocol_version defaults differ".to_owned());
    }
    if request_version != 0 {
        return Err(
            "V2 payload envelopes must use fail-closed protocol_version default 0".to_owned(),
        );
    }
    Ok(request_version)
}

fn normalize_schema(
    schema: &Schema<'_>,
) -> Result<(Vec<NormalizedObject>, Vec<NormalizedEnum>), String> {
    let mut objects = Vec::new();
    for object in schema.objects().iter() {
        let mut fields = Vec::new();
        for field in object.fields().iter() {
            fields.push(NormalizedField {
                name: field.name().to_owned(),
                id: field.id(),
                offset: field.offset(),
                ty: normalize_type(schema, &field.type_())?,
                default_integer: field.default_integer(),
                default_real: field.default_real(),
                required: field.required(),
                optional: field.optional(),
                key: field.key(),
            });
        }
        objects.push(NormalizedObject {
            name: object.name().to_owned(),
            kind: if object.is_struct() {
                "struct"
            } else {
                "table"
            }
            .to_owned(),
            fields,
        });
    }
    let mut enums = Vec::new();
    for enumeration in schema.enums().iter() {
        let name = enumeration.name().to_owned();
        let mut names = BTreeSet::new();
        let mut numbers = BTreeSet::new();
        let mut variants = Vec::new();
        for value in enumeration.values().iter() {
            if !names.insert(value.name().to_owned()) || !numbers.insert(value.value()) {
                return Err(format!("enum {name} has duplicate variants"));
            }
            let payload = if enumeration.is_union() && value.name() != "NONE" {
                let ty = value.union_type().ok_or_else(|| {
                    format!("union {name} variant {} has no payload", value.name())
                })?;
                if ty.base_type() != BaseType::Obj {
                    return Err(format!(
                        "union {name} variant {} is not an object",
                        value.name()
                    ));
                }
                Some(object_name(schema, ty.index(), &name)?.to_owned())
            } else {
                None
            };
            if enumeration.is_union() && value.name() == "NONE" && value.value() != 0 {
                return Err(format!("union {name} NONE must have tag zero"));
            }
            variants.push(NormalizedVariant {
                name: value.name().to_owned(),
                value: value.value(),
                payload,
            });
        }
        if variants.is_empty() {
            return Err(format!("BFBS enum {name} has no variants"));
        }
        enums.push(NormalizedEnum {
            name,
            kind: if enumeration.is_union() {
                "union"
            } else {
                "enum"
            }
            .to_owned(),
            underlying: normalize_underlying(&enumeration.underlying_type())?,
            variants,
        });
    }
    Ok((objects, enums))
}

fn normalize_underlying(ty: &Type<'_>) -> Result<String, String> {
    if ty.base_type() == BaseType::UType {
        return Ok("ubyte".to_owned());
    }
    scalar_name(ty.base_type())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("unsupported enum type {:?}", ty.base_type()))
}

fn normalize_type(schema: &Schema<'_>, ty: &Type<'_>) -> Result<NormalizedType, String> {
    let base = ty.base_type();
    if scalar_name(base).is_some() && ty.index() >= 0 {
        return Ok(named_type(
            "enum",
            Some(scalar_name(base).unwrap()),
            enum_name(schema, ty.index(), "enum field")?,
        ));
    }
    match base {
        BaseType::UType => Ok(named_type(
            "enum",
            None,
            enum_name(schema, ty.index(), "enum field")?,
        )),
        BaseType::Union => Ok(named_type(
            "union",
            None,
            enum_name(schema, ty.index(), "union field")?,
        )),
        BaseType::Obj => Ok(named_type(
            "object",
            None,
            object_name(schema, ty.index(), "object field")?,
        )),
        BaseType::Vector => normalize_vector(schema, ty),
        other => scalar_name(other)
            .map(|base| NormalizedType {
                kind: "scalar".to_owned(),
                base: Some(base.to_owned()),
                name: None,
                element: None,
                map_key: None,
                map_entry: None,
            })
            .ok_or_else(|| format!("unsupported BFBS type {other:?}")),
    }
}

fn named_type(kind: &str, base: Option<&str>, name: &str) -> NormalizedType {
    NormalizedType {
        kind: kind.to_owned(),
        base: base.map(ToOwned::to_owned),
        name: Some(name.to_owned()),
        element: None,
        map_key: None,
        map_entry: None,
    }
}

fn normalize_vector(schema: &Schema<'_>, ty: &Type<'_>) -> Result<NormalizedType, String> {
    let element = normalize_vector_element(schema, ty)?;
    if ty.element() != BaseType::Obj {
        return Ok(vector_type(element));
    }
    let entry_name = object_name(schema, ty.index(), "vector object")?;
    let entry = schema
        .objects()
        .iter()
        .find(|object| object.name() == entry_name)
        .ok_or_else(|| format!("missing vector entry {entry_name}"))?;
    let keys = entry
        .fields()
        .iter()
        .filter(|field| field.key())
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return Ok(vector_type(element));
    }
    if keys.len() != 1 || entry.fields().len() != 2 {
        // A FlatBuffers key attribute also enables lookup-by-key on ordinary
        // tables. It does not make a multi-field object vector a semantic map.
        return Ok(vector_type(element));
    }
    let key = normalize_type(schema, &keys[0].type_())?;
    if key.kind != "scalar" || key.base.as_deref() != Some("string") {
        return Err(format!("keyed vector {entry_name} must use a string key"));
    }
    let value = entry
        .fields()
        .iter()
        .find(|field| !field.key())
        .ok_or_else(|| format!("keyed vector {entry_name} has no value"))?;
    Ok(NormalizedType {
        kind: "map".to_owned(),
        base: None,
        name: Some(entry_name.to_owned()),
        element: Some(Box::new(normalize_type(schema, &value.type_())?)),
        map_key: Some(Box::new(key)),
        map_entry: Some(entry_name.to_owned()),
    })
}

fn vector_type(element: NormalizedType) -> NormalizedType {
    NormalizedType {
        kind: "vector".to_owned(),
        base: None,
        name: None,
        element: Some(Box::new(element)),
        map_key: None,
        map_entry: None,
    }
}

fn normalize_vector_element(schema: &Schema<'_>, ty: &Type<'_>) -> Result<NormalizedType, String> {
    if scalar_name(ty.element()).is_some() && ty.index() >= 0 {
        return Ok(named_type(
            "enum",
            Some(scalar_name(ty.element()).unwrap()),
            enum_name(schema, ty.index(), "vector enum")?,
        ));
    }
    match ty.element() {
        BaseType::UType => Ok(named_type(
            "enum",
            None,
            enum_name(schema, ty.index(), "vector enum")?,
        )),
        BaseType::Union => Ok(named_type(
            "union",
            None,
            enum_name(schema, ty.index(), "vector union")?,
        )),
        BaseType::Obj => Ok(named_type(
            "object",
            None,
            object_name(schema, ty.index(), "vector object")?,
        )),
        BaseType::Vector => Err("nested BFBS vectors are unsupported".to_owned()),
        other => scalar_name(other)
            .map(|base| NormalizedType {
                kind: "scalar".to_owned(),
                base: Some(base.to_owned()),
                name: None,
                element: None,
                map_key: None,
                map_entry: None,
            })
            .ok_or_else(|| format!("unsupported vector element {other:?}")),
    }
}

fn scalar_name(base: BaseType) -> Option<&'static str> {
    match base {
        BaseType::Bool => Some("bool"),
        BaseType::Byte => Some("byte"),
        BaseType::UByte => Some("ubyte"),
        BaseType::Short => Some("short"),
        BaseType::UShort => Some("ushort"),
        BaseType::Int => Some("int"),
        BaseType::UInt => Some("uint"),
        BaseType::Long => Some("long"),
        BaseType::ULong => Some("ulong"),
        BaseType::Float => Some("float"),
        BaseType::Double => Some("double"),
        BaseType::String => Some("string"),
        _ => None,
    }
}

fn object_name<'a>(schema: &'a Schema<'_>, index: i32, context: &str) -> Result<&'a str, String> {
    if index < 0 || index as usize >= schema.objects().len() {
        return Err(format!("{context} references missing object index {index}"));
    }
    Ok(schema.objects().get(index as usize).name())
}

fn enum_name<'a>(schema: &'a Schema<'_>, index: i32, context: &str) -> Result<&'a str, String> {
    if index < 0 || index as usize >= schema.enums().len() {
        return Err(format!("{context} references missing enum index {index}"));
    }
    Ok(schema.enums().get(index as usize).name())
}

fn merge<T>(target: &mut BTreeMap<String, T>, values: Vec<T>, kind: &str) -> Result<(), String>
where
    T: Clone + PartialEq + Named,
{
    for value in values {
        let name = value.name().to_owned();
        if let Some(previous) = target.get(&name) {
            if previous != &value {
                return Err(format!("conflicting BFBS definitions for {kind} {name}"));
            }
        } else {
            target.insert(name, value);
        }
    }
    Ok(())
}

trait Named {
    fn name(&self) -> &str;
}
impl Named for NormalizedObject {
    fn name(&self) -> &str {
        &self.name
    }
}
impl Named for NormalizedEnum {
    fn name(&self) -> &str {
        &self.name
    }
}

fn validate_references(
    objects: &BTreeMap<String, NormalizedObject>,
    enums: &BTreeMap<String, NormalizedEnum>,
) -> Result<(), String> {
    fn check(
        ty: &NormalizedType,
        objects: &BTreeMap<String, NormalizedObject>,
        enums: &BTreeMap<String, NormalizedEnum>,
    ) -> Result<(), String> {
        if let Some(name) = &ty.name {
            let found = match ty.kind.as_str() {
                "object" => objects.contains_key(name),
                "enum" | "union" => enums.contains_key(name),
                _ => true,
            };
            if !found {
                return Err(format!("missing {} reference {name}", ty.kind));
            }
        }
        if let Some(element) = &ty.element {
            check(element, objects, enums)?;
        }
        if let Some(key) = &ty.map_key {
            check(key, objects, enums)?;
        }
        Ok(())
    }
    for object in objects.values() {
        for field in &object.fields {
            check(&field.ty, objects, enums)?;
        }
    }
    for enumeration in enums.values().filter(|item| item.kind == "union") {
        for payload in enumeration
            .variants
            .iter()
            .filter_map(|item| item.payload.as_ref())
        {
            if !objects.contains_key(payload) {
                return Err(format!(
                    "union {} references missing payload {payload}",
                    enumeration.name
                ));
            }
        }
    }
    Ok(())
}

fn validate_union_fields(objects: &BTreeMap<String, NormalizedObject>) -> Result<(), String> {
    for object in objects.values() {
        for payload in object
            .fields
            .iter()
            .filter(|field| field.ty.kind == "union")
        {
            let count = object
                .fields
                .iter()
                .filter(|field| {
                    field.ty.kind == "enum"
                        && field.ty.name == payload.ty.name
                        && field.id + 1 == payload.id
                })
                .count();
            if count != 1 {
                return Err(format!(
                    "malformed union field {}.{}",
                    object.name, payload.name
                ));
            }
        }
    }
    Ok(())
}

fn normalize_root(schema: &Schema<'_>, side: &str) -> Result<Root, String> {
    let table = schema
        .root_table()
        .ok_or_else(|| format!("{side} BFBS has no root"))?;
    let unions = table
        .fields()
        .iter()
        .filter(|field| field.type_().base_type() == BaseType::Union)
        .collect::<Vec<_>>();
    if unions.len() != 1 {
        return Err(format!("{side} root {} must have one union", table.name()));
    }
    Ok(Root {
        side: side.to_owned(),
        table: table.name().to_owned(),
        union: enum_name(schema, unions[0].type_().index(), "root union")?.to_owned(),
        identifier: schema.file_ident().map(ToOwned::to_owned),
    })
}

fn pair_operations(
    objects: &BTreeMap<String, NormalizedObject>,
    enums: &BTreeMap<String, NormalizedEnum>,
    request: &Root,
    response: &Root,
) -> Result<(Vec<Operation>, String, String, i64), String> {
    let requests = operation_variants(&enums[&request.union], objects, "request", "Request")?;
    let response_union = &enums[&response.union];
    let mut results = BTreeMap::new();
    let mut errors = Vec::new();
    for variant in response_union
        .variants
        .iter()
        .filter(|item| item.name != "NONE")
    {
        let payload = variant
            .payload
            .as_ref()
            .ok_or_else(|| format!("response variant {} has no payload", variant.name))?;
        if let Some(stem) = short_type_name(payload).strip_suffix("Result") {
            if results
                .insert(
                    stem.to_owned(),
                    (payload.clone(), variant.name.clone(), variant.value),
                )
                .is_some()
            {
                return Err(format!("duplicate response operation {stem}"));
            }
        } else {
            errors.push((payload.clone(), variant.name.clone(), variant.value));
        }
    }
    if errors.len() != 1 {
        return Err(format!(
            "response union must have one non-operation payload, found {}",
            errors.len()
        ));
    }
    let mut operations = Vec::new();
    for (stem, (request_payload, request_variant, request_tag)) in requests {
        let (response_payload, response_variant, response_tag) = results
            .remove(&stem)
            .ok_or_else(|| format!("unpaired request operation {request_payload}"))?;
        operations.push(Operation {
            name: snake_case(&stem),
            request: request_payload,
            response: response_payload,
            request_variant,
            request_tag,
            response_variant,
            response_tag,
        });
    }
    if let Some((stem, (payload, _, _))) = results.into_iter().next() {
        return Err(format!("unpaired result operation {stem} ({payload})"));
    }
    let (payload, variant, tag) = errors.remove(0);
    Ok((operations, payload, variant, tag))
}

fn operation_variants(
    union: &NormalizedEnum,
    objects: &BTreeMap<String, NormalizedObject>,
    side: &str,
    suffix: &str,
) -> Result<BTreeMap<String, (String, String, i64)>, String> {
    let mut result = BTreeMap::new();
    for variant in union.variants.iter().filter(|item| item.name != "NONE") {
        let payload = variant
            .payload
            .as_ref()
            .ok_or_else(|| format!("{side} variant {} has no payload", variant.name))?;
        if !objects.contains_key(payload) {
            return Err(format!("{side} variant references missing {payload}"));
        }
        let stem = short_type_name(payload)
            .strip_suffix(suffix)
            .ok_or_else(|| format!("{side} payload {payload} does not end in {suffix}"))?;
        if result
            .insert(
                stem.to_owned(),
                (payload.clone(), variant.name.clone(), variant.value),
            )
            .is_some()
        {
            return Err(format!("duplicate {side} operation {stem}"));
        }
    }
    if result.is_empty() {
        return Err(format!("{side} union has no operations"));
    }
    Ok(result)
}

fn find_recursive_edges(
    objects: &BTreeMap<String, NormalizedObject>,
    enums: &BTreeMap<String, NormalizedEnum>,
    include_indirect: bool,
) -> BTreeSet<(String, String)> {
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for object in objects.values() {
        for field in &object.fields {
            if field.ty.kind == "object" {
                if let Some(name) = &field.ty.name {
                    graph
                        .entry(object.name.clone())
                        .or_default()
                        .insert(name.clone());
                }
            } else if include_indirect && matches!(field.ty.kind.as_str(), "vector" | "map") {
                if let Some(name) = field.ty.element.as_deref().and_then(type_reference) {
                    graph
                        .entry(object.name.clone())
                        .or_default()
                        .insert(name.to_owned());
                }
            } else if field.ty.kind == "union" {
                if let Some(union) = field.ty.name.as_ref().and_then(|name| enums.get(name)) {
                    for payload in union
                        .variants
                        .iter()
                        .filter_map(|item| item.payload.as_ref())
                    {
                        graph
                            .entry(object.name.clone())
                            .or_default()
                            .insert(payload.clone());
                    }
                }
            }
        }
    }
    let mut result = BTreeSet::new();
    for (source, targets) in &graph {
        for target in targets {
            let mut pending = vec![target.as_str()];
            let mut seen = BTreeSet::new();
            while let Some(current) = pending.pop() {
                if current == source {
                    result.insert((source.clone(), target.clone()));
                    break;
                }
                if seen.insert(current) {
                    pending.extend(
                        graph
                            .get(current)
                            .into_iter()
                            .flat_map(|next| next.iter().map(String::as_str)),
                    );
                }
            }
        }
    }
    result
}

fn load_policy(
    objects: &BTreeMap<String, NormalizedObject>,
    enums: &BTreeMap<String, NormalizedEnum>,
) -> Result<ClientPolicy, String> {
    let policy: ClientPolicy = serde_json::from_str(CLIENT_POLICY)
        .map_err(|error| format!("invalid client-policy.json: {error}"))?;
    for path in &policy.required_fields {
        validate_field_path(path, objects)?;
    }
    let mut alias_names = BTreeSet::new();
    for (path, alias) in &policy.response_aliases {
        let field = validate_field_path(path, objects)?;
        let target = field
            .ty
            .name
            .as_ref()
            .filter(|_| field.ty.kind == "object")
            .ok_or_else(|| format!("response alias {path} is not an object"))?;
        if !alias_names.insert(alias.name.as_str()) {
            return Err(format!("duplicate response alias {}", alias.name));
        }
        for required in &alias.required_fields {
            if !objects[target]
                .fields
                .iter()
                .any(|field| field.name == *required)
            {
                return Err(format!(
                    "response alias {path} requires missing field {required}"
                ));
            }
        }
    }
    for (path, rename) in &policy.union_variants {
        validate_union_policy(path, rename, enums)?;
    }
    for path in &policy.named_union_variants {
        validate_union_policy(path, "named", enums)?;
    }
    for path in &policy.unit_union_variants {
        validate_union_policy(path, "unit", enums)?;
    }
    for (object, default) in &policy.union_defaults {
        let wrapper = objects
            .get(object)
            .ok_or_else(|| format!("union default references missing {object}"))?;
        let (_, payload) = union_wrapper_fields(wrapper)
            .ok_or_else(|| format!("union default {object} is not a wrapper"))?;
        let union = &enums[payload.ty.name.as_deref().unwrap_or_default()];
        if !union
            .variants
            .iter()
            .filter(|item| item.payload.is_some())
            .any(|variant| public_union_variant_name(object, union, variant, &policy) == *default)
        {
            return Err(format!(
                "union default {object} references missing variant {default}"
            ));
        }
    }
    let mut functions = BTreeSet::new();
    for (object, function) in &policy.standalone_encoders {
        if !objects.contains_key(object) || function.is_empty() || !functions.insert(function) {
            return Err(format!("invalid standalone encoder {object}: {function}"));
        }
    }
    Ok(policy)
}

fn validate_field_path<'a>(
    path: &str,
    objects: &'a BTreeMap<String, NormalizedObject>,
) -> Result<&'a NormalizedField, String> {
    let (object, field) = path
        .rsplit_once('.')
        .ok_or_else(|| format!("invalid field policy path {path}"))?;
    objects
        .get(object)
        .and_then(|value| value.fields.iter().find(|item| item.name == field))
        .ok_or_else(|| format!("field policy references missing {path}"))
}

fn validate_union_policy(
    path: &str,
    value: &str,
    enums: &BTreeMap<String, NormalizedEnum>,
) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("union policy {path} is empty"));
    }
    let found = enums
        .values()
        .filter(|item| item.kind == "union")
        .any(|item| {
            path.strip_prefix(&item.name)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(|variant| item.variants.iter().any(|value| value.name == variant))
        });
    if found {
        Ok(())
    } else {
        Err(format!("union policy references missing variant {path}"))
    }
}

pub fn short_type_name(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(_, short)| short)
}

pub fn snake_case(value: &str) -> String {
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

#[allow(dead_code)]
pub fn rust_variant_name(name: &str) -> String {
    name.rsplit_once('_')
        .map_or(name, |(_, suffix)| suffix)
        .to_owned()
}

pub fn union_wrapper_fields(
    object: &NormalizedObject,
) -> Option<(&NormalizedField, &NormalizedField)> {
    if object.kind != "table" || object.fields.len() != 2 {
        return None;
    }
    let payload = object
        .fields
        .iter()
        .find(|field| field.ty.kind == "union")?;
    let tag = object.fields.iter().find(|field| {
        field.ty.kind == "enum" && field.ty.name == payload.ty.name && field.id + 1 == payload.id
    })?;
    Some((tag, payload))
}

pub fn public_union_variant_name(
    public_type: &str,
    union: &NormalizedEnum,
    variant: &NormalizedVariant,
    policy: &ClientPolicy,
) -> String {
    if let Some(name) = policy
        .union_variants
        .get(&format!("{}.{}", union.name, variant.name))
    {
        return name.clone();
    }
    let public = short_type_name(public_type);
    let mut name = variant
        .payload
        .as_deref()
        .map(short_type_name)
        .unwrap_or(&variant.name)
        .to_owned();
    if let Some(value) = name.strip_prefix(public).filter(|item| !item.is_empty()) {
        name = value.to_owned();
    } else if let Some(value) = name.strip_suffix(public).filter(|item| !item.is_empty()) {
        name = value.to_owned();
    }
    for suffix in ["Expr", "Operation", "Target"] {
        if let Some(base) = public.strip_suffix(suffix) {
            if let Some(value) = name.strip_prefix(base).filter(|item| !item.is_empty()) {
                name = value.to_owned();
            } else if suffix == "Expr" {
                if let Some(value) = name.strip_suffix(base).filter(|item| !item.is_empty()) {
                    name = value.to_owned();
                }
            }
        }
    }
    name
}

fn type_reference(ty: &NormalizedType) -> Option<&str> {
    if ty.kind == "object" {
        ty.name.as_deref()
    } else {
        ty.element.as_deref().and_then(type_reference)
    }
}

pub fn schema_model(model: &Model) -> Value {
    let objects = model.objects.values().map(|object| json!({
        "name": object.name, "kind": object.kind,
        "ergonomic": if object.kind == "struct" && object.fields.len() == 1 { Some("optional_scalar") } else if union_wrapper_fields(object).is_some() { Some("union_variant") } else { None },
        "fields": object.fields.iter().map(|field| json!({
            "name": field.name, "id": field.id, "offset": field.offset,
            "type": type_json(&field.ty), "default_integer": field.default_integer,
            "default_real": field.default_real, "required": field.required,
            "optional": field.optional, "key": field.key,
            "protocol_required": model.policy.required_fields.contains(&format!("{}.{}", object.name, field.name)),
            "recursive": type_reference(&field.ty).is_some_and(|target| model.recursive_edges.contains(&(object.name.clone(), target.to_owned()))),
            "boxed": type_reference(&field.ty).is_some_and(|target| model.boxed_edges.contains(&(object.name.clone(), target.to_owned())))
        })).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    let enums = model.enums.values().map(|item| json!({
        "name": item.name, "kind": item.kind, "underlying": item.underlying,
        "variants": item.variants.iter().map(|variant| json!({"name":variant.name,"value":variant.value,"payload":variant.payload})).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    json!({
        "format_version": 2,
        "objects": objects,
        "enums": enums,
        "roots": model.roots.iter().map(|root| json!({"side":root.side,"table":root.table,"union":root.union,"identifier":root.identifier})).collect::<Vec<_>>(),
        "operations": model.operations.iter().map(|operation| json!({
            "operation":operation.name,"request":operation.request,"request_variant":operation.request_variant,"request_tag":operation.request_tag,
            "response":operation.response,"response_variant":operation.response_variant,"response_tag":operation.response_tag
        })).collect::<Vec<_>>(),
        "recursive_references": model.recursive_edges.iter().map(|(source,target)| json!({"source":source,"target":target})).collect::<Vec<_>>(),
        "boxed_references": model.boxed_edges.iter().map(|(source,target)| json!({"source":source,"target":target})).collect::<Vec<_>>()
    })
}

fn type_json(ty: &NormalizedType) -> Value {
    let mut result = serde_json::Map::new();
    result.insert("kind".to_owned(), json!(ty.kind));
    if let Some(base) = &ty.base {
        result.insert("base".to_owned(), json!(base));
    }
    if let Some(name) = &ty.name {
        result.insert("name".to_owned(), json!(name));
    }
    if let Some(element) = &ty.element {
        result.insert("element".to_owned(), type_json(element));
    }
    if let Some(key) = &ty.map_key {
        result.insert("key".to_owned(), type_json(key));
    }
    if let Some(entry) = &ty.map_entry {
        result.insert("entry".to_owned(), json!(entry));
    }
    Value::Object(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flatbuffers::root;

    fn baseline_model() -> Model {
        let request_bytes =
            include_bytes!("../../../core/crates/searchkernel-json/bfbs/searchkernel_request.bfbs");
        let response_bytes = include_bytes!(
            "../../../core/crates/searchkernel-json/bfbs/searchkernel_response.bfbs"
        );
        let request = root::<Schema<'_>>(request_bytes).expect("request BFBS");
        let response = root::<Schema<'_>>(response_bytes).expect("response BFBS");
        normalize(&request, &response).expect("normalized baseline")
    }

    #[test]
    fn preserves_enum_identity_union_tags_and_operation_tags() {
        let model = baseline_model();
        let primitive_kind = &model.enums["SearchKernel.V2.PrimitiveDataTypeKind"];
        assert_eq!(primitive_kind.underlying, "ubyte");
        assert!(primitive_kind
            .variants
            .iter()
            .any(|variant| variant.name == "Float64" && variant.value == 7));
        let search = model
            .operations
            .iter()
            .find(|operation| operation.name == "search")
            .expect("search operation");
        assert_eq!((search.request_tag, search.response_tag), (6, 6));
        assert_eq!(model.error_tag, 8);
        assert_eq!(model.error_payload, "SearchKernel.V2.Error");
    }

    #[test]
    fn derives_optional_enum_maps_and_recursive_boxing() {
        let model = baseline_model();
        let optional = &model.objects["SearchKernel.V2.MissingPolicy"];
        assert_eq!(optional.kind, "table");
        assert_eq!(optional.fields[0].ty.kind, "union");
        assert_eq!(
            optional.fields[0].ty.name.as_deref(),
            Some("SearchKernel.V2.MissingPolicyKind")
        );

        let fields = model.objects["SearchKernel.V2.Document"]
            .fields
            .iter()
            .find(|field| field.name == "fields")
            .expect("document fields");
        assert_eq!(fields.ty.kind, "map");
        assert_eq!(
            fields.ty.map_entry.as_deref(),
            Some("SearchKernel.V2.ValueEntry")
        );
        assert_eq!(
            fields
                .ty
                .element
                .as_deref()
                .and_then(|ty| ty.name.as_deref()),
            Some("SearchKernel.V2.Value")
        );
        assert!(model.recursive_edges.contains(&(
            "SearchKernel.V2.Value".to_owned(),
            "SearchKernel.V2.ValueList".to_owned()
        )));
        assert!(model.boxed_edges.contains(&(
            "SearchKernel.V2.FilterNot".to_owned(),
            "SearchKernel.V2.FilterExpr".to_owned()
        )));
    }

    #[test]
    fn rejects_conflicts_missing_references_and_stale_policy() {
        let mut model = baseline_model();
        let original = model.objects["SearchKernel.V2.Document"].clone();
        let mut conflicting = original.clone();
        conflicting.kind = "struct".to_owned();
        let mut merged = BTreeMap::from([(original.name.clone(), original)]);
        assert!(merge(&mut merged, vec![conflicting], "object")
            .unwrap_err()
            .contains("conflicting BFBS definitions"));

        let missing = NormalizedObject {
            name: "Synthetic.MissingOwner".to_owned(),
            kind: "table".to_owned(),
            fields: vec![NormalizedField {
                name: "value".to_owned(),
                id: 0,
                offset: 4,
                ty: named_type("object", None, "Synthetic.DoesNotExist"),
                default_integer: 0,
                default_real: 0.0,
                required: false,
                optional: true,
                key: false,
            }],
        };
        assert!(validate_references(
            &BTreeMap::from([(missing.name.clone(), missing)]),
            &BTreeMap::new()
        )
        .unwrap_err()
        .contains("missing object reference"));

        model.objects.remove("SearchKernel.V2.AnalyzedToken");
        let error = load_policy(&model.objects, &model.enums).unwrap_err();
        assert!(error.contains("AnalyzedToken"), "{error}");
    }
}
