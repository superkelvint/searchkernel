use crate::document::{
    ArrayValue, AttributeDocument, AttributeUpdate, Document, DocumentUpdate, FieldUpdate,
    FieldValue, PositionValue, PredicateValue, ScalarValue,
};
use crate::error::{check, cstring, path_cstring, Error, Result};
use crate::linguistics::{Linguistics, LinguisticsOptions};
use crate::native::{
    add_attribute_document_value, add_attribute_update_value, add_document_field,
    add_text_annotation, add_text_array_annotation, add_update_field, attribute_document_create,
    attribute_update_create, engine_group_tree, engine_group_tree_with_request,
    generic_search_request_add_grouping_plan, grouping_aggregation_average,
    grouping_aggregation_count, grouping_aggregation_for_expression,
    grouping_aggregation_quantiles, grouping_aggregation_summary,
    grouping_expression_aggregation_reference, grouping_expression_attribute,
    grouping_filter_is_true, grouping_filter_multi, grouping_filter_not, grouping_filter_range,
    grouping_filter_regex, grouping_filter_regex_expression, grouping_level_add_aggregation,
    grouping_level_add_order_by, grouping_level_set_filter, grouping_level_set_limits,
    grouping_plan_add_level, grouping_plan_add_root_aggregation, grouping_plan_create,
    grouping_plan_set_all, native_bytes, native_f64s, read_generic_search_result, require_ptr,
    NativeDocument, NativeError, NativeGenericSearchRequest, NativeQuery, NativeUpdate,
};
use crate::query::{self, lower_query, validate_query_fields, Query};
use crate::search::{
    validate_search_ranking_inputs, GroupingAggregation, GroupingExpression, GroupingFilter,
    GroupingLiteral, GroupingOrder, GroupingPlan, MissingPlacement, QueryInput, SearchRequest,
    SearchResult, SortOrder, Source, SummaryRequest,
};
use crate::tensor::TensorValue;
use searchkernel_vespa_bindings as sys;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::ptr::{self, NonNull};

const EXPECTED_ABI: u32 = sys::EXPECTED_ABI;

/// Bounded conversion above the unchanged raw feature DATA ABI. Vespa owns the
/// binary codec; callers never reinterpret its bytes as doubles themselves.
pub fn decode_diagnostic_tensor(bytes: &[u8], max_cells: u32) -> Result<TensorValue> {
    if bytes.is_empty() || bytes.len() > 262_144 || max_cells == 0 || max_cells > 4096 {
        return Err(Error::message(
            "diagnostic tensor exceeds input/cell bounds",
        ));
    }
    let mut type_spec = vec![0_u8; 1024];
    let mut cells = vec![0.0; max_cells as usize];
    let (mut type_length, mut cell_count) = (0_u32, 0_u32);
    let mut error = NativeError::new();
    // Both outputs are caller-owned, sized above, and copied before returning.
    let status = unsafe {
        sys::sk_diagnostic_tensor_decode(
            bytes.as_ptr(),
            bytes.len() as u32,
            type_spec.as_mut_ptr().cast(),
            type_spec.len() as u32,
            &mut type_length,
            cells.as_mut_ptr(),
            max_cells,
            &mut cell_count,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    if type_length as usize > type_spec.len() || cell_count as usize > cells.len() {
        return Err(Error::message(
            "native diagnostic tensor output exceeds capacity",
        ));
    }
    type_spec.truncate(type_length as usize);
    cells.truncate(cell_count as usize);
    Ok(TensorValue::dense(
        String::from_utf8(type_spec)
            .map_err(|_| Error::message("native tensor type is not UTF-8"))?,
        cells,
    ))
}

fn json_f64(value: &serde_json::Value, field: &str, context: &str) -> Result<f64, Error> {
    value
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| Error::message(format!("{context} is missing numeric field '{field}'")))
}

fn decode_struct_array(value: &str) -> Result<Vec<BTreeMap<String, ScalarValue>>, Error> {
    let value: serde_json::Value = serde_json::from_str(value).map_err(|error| {
        Error::message(format!(
            "native document struct array is invalid JSON: {error}"
        ))
    })?;
    let elements = value
        .as_array()
        .ok_or_else(|| Error::message("native document struct array is not a JSON array"))?;
    let mut decoded = Vec::with_capacity(elements.len());
    for element in elements {
        let fields = element.as_object().ok_or_else(|| {
            Error::message("native document struct array element is not a JSON object")
        })?;
        let mut decoded_fields = BTreeMap::new();
        for (name, encoded) in fields {
            let encoded = encoded.as_object().ok_or_else(|| {
                Error::message(format!("native struct field '{name}' is not an object"))
            })?;
            let kind = encoded
                .get("type")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    Error::message(format!("native struct field '{name}' has no type"))
                })?;
            let value = encoded.get("value").ok_or_else(|| {
                Error::message(format!("native struct field '{name}' has no value"))
            })?;
            let scalar = match kind {
                "string" => ScalarValue::String(
                    value
                        .as_str()
                        .ok_or_else(|| {
                            Error::message(format!("native struct field '{name}' is not a string"))
                        })?
                        .to_owned(),
                ),
                "integer" => ScalarValue::Integer(value.as_i64().ok_or_else(|| {
                    Error::message(format!("native struct field '{name}' is not an integer"))
                })?),
                "float" => ScalarValue::Float(value.as_f64().ok_or_else(|| {
                    Error::message(format!("native struct field '{name}' is not a float"))
                })?),
                "boolean" => ScalarValue::Boolean(value.as_bool().ok_or_else(|| {
                    Error::message(format!("native struct field '{name}' is not a boolean"))
                })?),
                _ => {
                    return Err(Error::message(format!(
                        "native struct field '{name}' has unknown type '{kind}'"
                    )))
                }
            };
            decoded_fields.insert(name.clone(), scalar);
        }
        decoded.push(decoded_fields);
    }
    Ok(decoded)
}

fn document_value_payload_name(kind: u32) -> Result<Option<&'static str>, Error> {
    Ok(match kind {
        sys::SK_VALUE_STRING => Some("document field value"),
        sys::SK_VALUE_INTEGER | sys::SK_VALUE_FLOAT | sys::SK_VALUE_BOOLEAN => None,
        sys::SK_VALUE_STRING_ARRAY => Some("document string array value"),
        sys::SK_VALUE_STRUCT_ARRAY => Some("document struct array value"),
        sys::SK_VALUE_POSITION => Some("document position value"),
        sys::SK_VALUE_PREDICATE => Some("document predicate value"),
        _ => {
            return Err(Error::message(
                "native document returned an unknown value kind",
            ))
        }
    })
}

fn decode_non_tensor_document_value(
    kind: u32,
    payload: &[u8],
    integer: i64,
    floating: f64,
    boolean: bool,
) -> Result<FieldValue, Error> {
    match kind {
        sys::SK_VALUE_STRING => {
            let value = std::str::from_utf8(payload)
                .map_err(|_| Error::message("native document string value is invalid UTF-8"))?;
            Ok(FieldValue::Scalar(ScalarValue::String(value.to_owned())))
        }
        sys::SK_VALUE_INTEGER => Ok(FieldValue::Scalar(ScalarValue::Integer(integer))),
        sys::SK_VALUE_FLOAT => Ok(FieldValue::Scalar(ScalarValue::Float(floating))),
        sys::SK_VALUE_BOOLEAN => Ok(FieldValue::Scalar(ScalarValue::Boolean(boolean))),
        sys::SK_VALUE_STRING_ARRAY => {
            let value = std::str::from_utf8(payload).map_err(|_| {
                Error::message("native document string array value is invalid UTF-8")
            })?;
            let values: Vec<String> = serde_json::from_str(value).map_err(|error| {
                Error::message(format!(
                    "native document string array is invalid JSON: {error}"
                ))
            })?;
            Ok(FieldValue::Array(ArrayValue::Strings(values)))
        }
        sys::SK_VALUE_STRUCT_ARRAY => {
            let value = std::str::from_utf8(payload).map_err(|_| {
                Error::message("native document struct array value is invalid UTF-8")
            })?;
            Ok(FieldValue::Array(ArrayValue::Structs(decode_struct_array(
                value,
            )?)))
        }
        sys::SK_VALUE_POSITION => {
            let value: serde_json::Value = serde_json::from_slice(payload).map_err(|error| {
                Error::message(format!("native document position is invalid JSON: {error}"))
            })?;
            let latitude = json_f64(&value, "latitude", "document position")?;
            let longitude = json_f64(&value, "longitude", "document position")?;
            Ok(FieldValue::Position(PositionValue::new(
                latitude, longitude,
            )))
        }
        sys::SK_VALUE_PREDICATE => {
            let value = std::str::from_utf8(payload)
                .map_err(|_| Error::message("native document predicate value is invalid UTF-8"))?;
            let parsed = serde_json::from_str::<serde_json::Value>(value).map_err(|error| {
                Error::message(format!(
                    "native document predicate is invalid JSON: {error}"
                ))
            })?;
            // Vespa preserves the insertion order of the predicate root.
            // PredicateSlimeBuilder writes `key` before `type`, while an
            // expression supplied as JSON retains its explicit type-first
            // representation. Preserve that distinction at the portable
            // boundary.
            let key_first = matches!(
                (value.find("\"key\""), value.find("\"type\"")),
                (Some(key_pos), Some(type_pos)) if key_pos < type_pos
            );
            if key_first {
                let key = parsed
                    .get("key")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| Error::message("native predicate feature key is missing"))?;
                let features = parsed
                    .get("feature_set")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| Error::message("native predicate feature set is missing"))?
                    .iter()
                    .map(|feature| {
                        feature
                            .as_str()
                            .map(|feature| (key.to_owned(), feature.to_owned()))
                            .ok_or_else(|| {
                                Error::message("native predicate feature is not a string")
                            })
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?;
                Ok(FieldValue::Predicate(PredicateValue::new(features)))
            } else {
                Ok(FieldValue::Predicate(PredicateValue::from_expression(
                    value,
                )))
            }
        }
        _ => Err(Error::message(
            "native document returned an unknown value kind",
        )),
    }
}

fn grouping_expression(
    expression: &GroupingExpression,
) -> Result<crate::native::NativeGroupingExpression, Error> {
    match expression {
        GroupingExpression::Attribute(field) => grouping_expression_attribute(field),
        GroupingExpression::AggregationReference(index) => {
            grouping_expression_aggregation_reference(*index)
        }
        GroupingExpression::Literal(GroupingLiteral::String(value)) => {
            crate::native::grouping_expression_string(value)
        }
        GroupingExpression::Literal(GroupingLiteral::Integer(value)) => {
            crate::native::grouping_expression_integer(*value)
        }
        GroupingExpression::Literal(GroupingLiteral::Float(value)) => {
            crate::native::grouping_expression_float(*value)
        }
        GroupingExpression::Literal(GroupingLiteral::Boolean(value)) => {
            crate::native::grouping_expression_boolean(*value)
        }
        GroupingExpression::Function { name, arguments } => {
            let arguments = arguments
                .iter()
                .map(grouping_expression)
                .collect::<Result<Vec<_>, _>>()?;
            crate::native::grouping_expression_function(name, &arguments)
        }
        GroupingExpression::Negative(value) => {
            let value = grouping_expression(value)?;
            crate::native::grouping_expression_negative(&value)
        }
        GroupingExpression::Binary {
            operator,
            left,
            right,
        } => {
            let left = grouping_expression(left)?;
            let right = grouping_expression(right)?;
            crate::native::grouping_expression_binary(&left, &right, *operator)
        }
    }
}

fn grouping_filter(filter: &GroupingFilter) -> Result<crate::native::NativeGroupingFilter, Error> {
    match filter {
        GroupingFilter::Regex { field, pattern } => grouping_filter_regex(pattern, field),
        GroupingFilter::RegexExpression {
            expression,
            pattern,
        } => {
            let expression = grouping_expression(expression)?;
            grouping_filter_regex_expression(pattern, &expression)
        }
        GroupingFilter::And(filters) | GroupingFilter::Or(filters) => {
            let filters = filters
                .iter()
                .map(grouping_filter)
                .collect::<Result<Vec<_>, _>>()?;
            grouping_filter_multi(&filters, matches!(filter, GroupingFilter::And(_)))
        }
        GroupingFilter::Not(filter) => {
            let filter = grouping_filter(filter)?;
            grouping_filter_not(&filter)
        }
        GroupingFilter::Range {
            expression,
            lower,
            upper,
            include_lower,
            include_upper,
        } => {
            let expression = grouping_expression(expression)?;
            grouping_filter_range(&expression, *lower, *upper, *include_lower, *include_upper)
        }
        GroupingFilter::IsTrue(expression) => {
            let expression = grouping_expression(expression)?;
            grouping_filter_is_true(&expression)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestFieldType {
    Text,
    String,
    Integer,
    Float,
    Boolean,
    Other,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct ManifestField {
    field_type: ManifestFieldType,
    attribute: bool,
}

#[derive(Debug)]
struct SchemaManifest {
    document_type: String,
    fields: BTreeMap<String, ManifestField>,
}

fn json_string_field(object: &str, field_name: &str) -> Option<String> {
    let marker = format!("\"{field_name}\"");
    let start = object.find(&marker)? + marker.len();
    let remainder = object[start..].trim_start();
    let remainder = remainder.strip_prefix(':')?.trim_start();
    let remainder = remainder.strip_prefix('"')?;
    let mut value = String::new();
    let mut escaped = false;
    for character in remainder.chars() {
        if escaped {
            value.push(match character {
                '"' => '"',
                '\\' => '\\',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(value);
        } else {
            value.push(character);
        }
    }
    None
}

fn json_boolean_field(object: &str, field_name: &str) -> Option<bool> {
    let marker = format!("\"{field_name}\"");
    let start = object.find(&marker)? + marker.len();
    let remainder = object[start..].trim_start();
    let remainder = remainder.strip_prefix(':')?.trim_start();
    if remainder.starts_with("true") {
        Some(true)
    } else if remainder.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn manifest_field_objects(manifest: &str) -> Result<Vec<&str>, Error> {
    let fields_start = manifest
        .find("\"fields\"")
        .and_then(|offset| manifest[offset..].find('[').map(|index| offset + index + 1))
        .ok_or_else(|| Error::message("generated schema manifest has no fields"))?;
    let bytes = manifest.as_bytes();
    let mut objects = Vec::new();
    let mut object_start = None;
    let mut depth = 0_u32;
    let mut escaped = false;
    let mut in_string = false;
    for (offset, byte) in bytes.iter().enumerate().skip(fields_start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match *byte {
            b'"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    object_start = Some(offset);
                }
                depth += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    Error::message("generated schema manifest has unbalanced fields")
                })?;
                if depth == 0 {
                    let start = object_start.take().ok_or_else(|| {
                        Error::message("generated schema manifest has unbalanced fields")
                    })?;
                    objects.push(&manifest[start..=offset]);
                }
            }
            b']' if depth == 0 => return Ok(objects),
            _ => {}
        }
    }
    Err(Error::message(
        "generated schema manifest has an unterminated fields array",
    ))
}

fn configured_schema(config_dir: &Path) -> Result<SchemaManifest, Error> {
    let manifest_path = config_dir.join("searchkernel-schema.json");
    let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
        Error::message(format!(
            "could not read generated schema manifest '{}': {error}",
            manifest_path.display()
        ))
    })?;
    let document_type = json_string_field(&manifest, "document_type")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::message("generated schema manifest has no document type"))?;
    let fields = manifest_field_objects(&manifest)?
        .iter()
        .map(|field| {
            let name = json_string_field(field, "name")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| Error::message("generated schema manifest has an unnamed field"))?;
            let field_type = match json_string_field(field, "type").as_deref() {
                Some("text") => ManifestFieldType::Text,
                Some("string") => ManifestFieldType::String,
                Some("integer") => ManifestFieldType::Integer,
                Some("float") => ManifestFieldType::Float,
                Some("boolean") => ManifestFieldType::Boolean,
                Some(_) | None => ManifestFieldType::Other,
            };
            let attribute = json_boolean_field(field, "attribute").unwrap_or(false);
            Ok((
                name.to_owned(),
                ManifestField {
                    field_type,
                    attribute,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    Ok(SchemaManifest {
        document_type,
        fields,
    })
}
pub struct Engine {
    pub(crate) handle: NonNull<sys::sk_engine>,
    document_type: String,
    #[allow(dead_code)]
    fields: BTreeMap<String, ManifestField>,
}

fn add_grouping_level_configuration(
    native_level: NonNull<sys::sk_grouping_level>,
    level: &crate::GroupingLevel,
) -> Result<(), Error> {
    if let Some(max_groups) = level.max_groups {
        let native_max_groups = if max_groups == u64::MAX {
            -1
        } else {
            i64::try_from(max_groups)
                .map_err(|_| Error::message("grouping max groups is too large"))?
        };
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_grouping_level_set_max_groups(
                    native_level.as_ptr(),
                    native_max_groups,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
    }
    grouping_level_set_limits(native_level, None, level.precision)?;
    for order in &level.order_by {
        let expression = grouping_expression(&order.expression)?;
        grouping_level_add_order_by(native_level, &expression, order.ascending)?;
    }
    if let Some(filter) = &level.filter {
        let filter = grouping_filter(filter)?;
        grouping_level_set_filter(native_level, &filter)?;
    }
    Ok(())
}

fn add_grouping_aggregation(
    native_level: NonNull<sys::sk_grouping_level>,
    aggregation: &GroupingAggregation,
) -> Result<(), Error> {
    let expression_for =
        |expression: &GroupingExpression| -> Result<_, Error> { grouping_expression(expression) };
    let native = match aggregation {
        GroupingAggregation::Count => grouping_aggregation_count()?,
        GroupingAggregation::Average(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_average(&expression)?
        }
        GroupingAggregation::Sum(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_SUM)?
        }
        GroupingAggregation::Min(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_MIN)?
        }
        GroupingAggregation::Max(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_MAX)?
        }
        GroupingAggregation::StandardDeviation(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_STDDEV)?
        }
        GroupingAggregation::Xor(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_XOR)?
        }
        GroupingAggregation::Quantiles {
            expression,
            quantiles,
        } => {
            let expression = expression_for(expression)?;
            grouping_aggregation_quantiles(&expression, quantiles)?
        }
        GroupingAggregation::Summary {
            summary_class,
            max_hits,
        } => grouping_aggregation_summary(summary_class, *max_hits)?,
    };
    grouping_level_add_aggregation(native_level, &native)
}

fn add_root_grouping_aggregation(
    native_plan: &crate::native::NativeGroupingPlan,
    aggregation: &GroupingAggregation,
) -> Result<(), Error> {
    let expression_for =
        |expression: &GroupingExpression| -> Result<_, Error> { grouping_expression(expression) };
    let native = match aggregation {
        GroupingAggregation::Count => grouping_aggregation_count()?,
        GroupingAggregation::Average(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_average(&expression)?
        }
        GroupingAggregation::Sum(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_SUM)?
        }
        GroupingAggregation::Min(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_MIN)?
        }
        GroupingAggregation::Max(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_MAX)?
        }
        GroupingAggregation::StandardDeviation(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_STDDEV)?
        }
        GroupingAggregation::Xor(expression) => {
            let expression = expression_for(expression)?;
            grouping_aggregation_for_expression(&expression, sys::SK_AGGREGATION_XOR)?
        }
        GroupingAggregation::Quantiles {
            expression,
            quantiles,
        } => {
            let expression = expression_for(expression)?;
            grouping_aggregation_quantiles(&expression, quantiles)?
        }
        GroupingAggregation::Summary {
            summary_class,
            max_hits,
        } => grouping_aggregation_summary(summary_class, *max_hits)?,
    };
    grouping_plan_add_root_aggregation(native_plan, &native)
}

// SAFETY: the native adapter owns the pointed-to engine and protects its
// lifetime with an exclusive lifecycle gate. Supported operations are safe to
// invoke concurrently according to the wrapper contract; the only mutable
// adapter-owned bookkeeping is protected inside the native implementation.
unsafe impl Send for Engine {}

// SAFETY: all methods borrow Engine immutably and the native adapter permits
// concurrent supported operations while close/destruction requires exclusive
// ownership. Rust's borrow rules prevent Drop from racing an outstanding
// shared borrow; Arc<Engine> can therefore safely share this owner.
unsafe impl Sync for Engine {}

impl Engine {
    /// Open a native engine using its data and generated configuration directories.
    pub fn open(data_dir: impl AsRef<Path>, config_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let actual_abi = unsafe { sys::sk_native_abi_version() };
        if actual_abi != EXPECTED_ABI {
            return Err(Error::message(format!(
                "unsupported native ABI version {actual_abi}; expected {EXPECTED_ABI}"
            )));
        }

        let schema = configured_schema(config_dir.as_ref())?;
        let data_dir = path_cstring(data_dir.as_ref())?;
        let config_dir = path_cstring(config_dir.as_ref())?;
        let mut handle = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_open(
                data_dir.as_ptr(),
                config_dir.as_ptr(),
                &mut handle,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(Self {
            handle: require_ptr(handle, "engine")?,
            document_type: schema.document_type,
            fields: schema.fields,
        })
    }

    /// Gracefully close the engine and report any native durability failure.
    ///
    /// Dropping an engine remains a best-effort cleanup operation because Rust
    /// destructors cannot return errors. This consuming method makes graceful
    /// close observable while ensuring the native handle is still destroyed.
    pub fn close(self) -> Result<(), Error> {
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_close(self.handle.as_ptr(), error.as_mut_ptr(), error.capacity())
        };
        check(status, &error)
    }

    /// Put a complete document. Each field is lowered to the native Vespa
    /// document path; subsequent fields are applied only while constructing
    /// the native document operation and are not exposed as separate API
    /// calls.
    pub fn put(&self, document: Document) -> Result<(), Error> {
        if document.fields().is_empty() {
            return Err(Error::message("document must contain at least one field"));
        }
        let id = cstring(document.id())?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_create_empty(
                id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native = NativeDocument(require_ptr(native, "document")?);
        for (field, value) in document.fields() {
            add_document_field(native.0.as_ptr(), field, value)?;
            let analysis = document
                .text_analysis()
                .get(field)
                .cloned()
                .or_else(|| match value {
                    FieldValue::Scalar(ScalarValue::String(value)) => {
                        Some(Linguistics.analyze_document(value, LinguisticsOptions::default()))
                    }
                    _ => None,
                });
            if let Some(analysis) = analysis {
                add_text_annotation(native.0.as_ptr(), field, &analysis, false)?;
            }
            if let FieldValue::Array(ArrayValue::Strings(values)) = value {
                let analyses = values
                    .iter()
                    .map(|value| Linguistics.analyze_document(value, LinguisticsOptions::default()))
                    .collect::<Vec<_>>();
                add_text_array_annotation(native.0.as_ptr(), field, &analyses, false)?;
            }
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Put a document through Vespa's attribute-only mutation path.
    pub fn put_attributes(&self, document: &AttributeDocument) -> Result<(), Error> {
        if document.fields().is_empty() {
            return Err(Error::message("document must contain at least one field"));
        }
        let native = attribute_document_create(document.document_id())?;
        for (field, value) in document.fields() {
            add_attribute_document_value(native.0.as_ptr(), field, value)?;
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_put_attributes(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Apply field-level mutations to an existing document.
    pub fn update(&self, update: DocumentUpdate) -> Result<(), Error> {
        if update.fields().is_empty() {
            return Err(Error::message(
                "document update must contain at least one field",
            ));
        }
        let id = cstring(update.id())?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_update_create_empty(
                id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native = NativeUpdate(require_ptr(native, "document update")?);
        if update.create_if_nonexistent_flag() {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_document_update_set_create_if_nonexistent(
                    native.0.as_ptr(),
                    1,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for (field, mutation) in update.fields() {
            add_update_field(native.0.as_ptr(), field, mutation)?;
            let analysis = update
                .text_analysis()
                .get(field)
                .cloned()
                .or_else(|| match mutation {
                    FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String(value))) => {
                        Some(Linguistics.analyze_document(value, LinguisticsOptions::default()))
                    }
                    _ => None,
                });
            if let Some(analysis) = analysis {
                add_text_annotation(native.0.as_ptr(), field, &analysis, true)?;
            }
            if let FieldUpdate::Assign(FieldValue::Array(ArrayValue::Strings(values))) = mutation {
                let analyses = update
                    .array_text_analysis()
                    .get(field)
                    .cloned()
                    .unwrap_or_else(|| {
                        values
                            .iter()
                            .map(|value| {
                                Linguistics.analyze_document(value, LinguisticsOptions::default())
                            })
                            .collect()
                    });
                add_text_array_annotation(native.0.as_ptr(), field, &analyses, true)?;
            }
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Apply assignments through Vespa's attribute-only mutation path.
    pub fn update_attributes(&self, update: &AttributeUpdate) -> Result<(), Error> {
        if update.assignments().is_empty() {
            return Err(Error::message(
                "update must contain at least one assignment",
            ));
        }
        let native = attribute_update_create(update.document_id())?;
        for (field, value) in update.assignments() {
            add_attribute_update_value(native.0.as_ptr(), field, value)?;
        }
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_update_attributes(
                self.handle.as_ptr(),
                native.0.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)
    }

    /// Execute an explicit M-layer request and preserve its Vespa-shaped
    /// result without performing a second query in the adapter.
    pub fn search(&self, request: SearchRequest) -> Result<SearchResult, Error> {
        self.execute_search(&request)
    }

    /// Execute one M-layer match request with all structured grouping plans
    /// attached to that request. Vespa serializes every grouping response in
    /// the same match reply, so callers cannot observe different generations
    /// between hits and sibling grouping plans.
    pub fn search_with_grouping_plans(
        &self,
        request: SearchRequest,
        plans: &[GroupingPlan],
    ) -> Result<SearchResult, Error> {
        self.execute_search_with_grouping_plans(&request, plans)
    }

    /// Retrieve the complete document through Vespa's document retriever.
    pub fn get(&self, document_id: &str) -> Result<Option<Document>, Error> {
        let document_id = cstring(document_id)?;
        let mut native = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_get(
                self.handle.as_ptr(),
                document_id.as_ptr(),
                &mut native,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let Some(native) = NonNull::new(native) else {
            return Ok(None);
        };
        let native = NativeDocument(native);
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_document_field_count(
                native.0.as_ptr(),
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let mut document = Document::new(document_id.to_string_lossy().into_owned());
        for index in 0..count {
            let mut name = ptr::null();
            let mut name_length = 0;
            let mut kind = 0;
            let mut string = ptr::null();
            let mut string_length = 0;
            let mut integer = 0;
            let mut floating = 0.0;
            let mut boolean = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_document_field(
                    native.0.as_ptr(),
                    index,
                    &mut name,
                    &mut name_length,
                    &mut kind,
                    &mut string,
                    &mut string_length,
                    &mut integer,
                    &mut floating,
                    &mut boolean,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            let name = native_bytes(&native, name, name_length, "document field name")?;
            let name = std::str::from_utf8(name)
                .map_err(|_| Error::message("native document field name is invalid UTF-8"))?;
            let value = if kind == sys::SK_VALUE_TENSOR {
                let mut type_spec = ptr::null();
                let mut type_spec_length = 0;
                let mut cells = ptr::null();
                let mut cell_count = 0;
                let mut error = NativeError::new();
                let status = unsafe {
                    sys::sk_document_field_tensor_dense(
                        native.0.as_ptr(),
                        index,
                        &mut type_spec,
                        &mut type_spec_length,
                        &mut cells,
                        &mut cell_count,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                };
                check(status, &error)?;
                let type_spec = native_bytes(
                    &native,
                    type_spec,
                    type_spec_length,
                    "document tensor type spec",
                )?;
                let type_spec = std::str::from_utf8(type_spec).map_err(|_| {
                    Error::message("native document tensor type spec is invalid UTF-8")
                })?;
                let cells = native_f64s(&native, cells, cell_count, "document tensor cells")?;
                if cells.iter().any(|value| !value.is_finite()) {
                    return Err(Error::message(
                        "native document tensor contains a non-finite cell",
                    ));
                }
                FieldValue::Tensor(TensorValue::dense(type_spec, cells.to_vec()))
            } else {
                let payload = match document_value_payload_name(kind)? {
                    Some(name) => native_bytes(&native, string, string_length, name)?,
                    None => &[],
                };
                decode_non_tensor_document_value(kind, payload, integer, floating, boolean != 0)?
            };
            document = document.field(name, value);
        }
        Ok(Some(document))
    }

    /// Delete a document and report Vespa's `wasFound` result.
    pub fn delete(&self, document_id: &str) -> Result<bool, Error> {
        let document_id = cstring(document_id)?;
        let mut was_found = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_remove(
                self.handle.as_ptr(),
                document_id.as_ptr(),
                &mut was_found,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        Ok(was_found != 0)
    }

    fn create_native_search_request(
        &self,
        request: &SearchRequest,
    ) -> Result<NativeGenericSearchRequest, Error> {
        let rank_profile = cstring(&request.rank_profile)?;
        let summary_kind = match request.summary {
            SummaryRequest::None => sys::SK_SUMMARY_NONE,
            SummaryRequest::Default => sys::SK_SUMMARY_DEFAULT,
            SummaryRequest::Fields(_) => sys::SK_SUMMARY_FIELDS,
        };
        let mut native_request = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_request_create(
                rank_profile.as_ptr(),
                request.limit,
                request.offset,
                summary_kind,
                &mut native_request,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_request =
            NativeGenericSearchRequest(require_ptr(native_request, "generic search request")?);
        if let Some(controls) = &request.diagnostics {
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_set_diagnostics(
                    native_request.0.as_ptr(),
                    controls.trace_level,
                    controls.matching_depth,
                    controls.first_phase_depth,
                    controls.second_phase_depth,
                    controls.raw_bytes,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        if let Some(summary_class) = &request.summary_class {
            let summary_class = cstring(summary_class)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_set_summary_class(
                    native_request.0.as_ptr(),
                    summary_class.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        let timeout_millis = u64::try_from(request.timeout.as_millis())
            .map_err(|_| Error::message("search timeout is too large to encode"))?;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_request_set_timeout(
                native_request.0.as_ptr(),
                timeout_millis,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if let Some(sort_spec) = &request.raw_sort_spec {
            let sort_spec = cstring(sort_spec)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_set_sort_spec(
                    native_request.0.as_ptr(),
                    sort_spec.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for sort in &request.sort {
            let field = cstring(&sort.field)?;
            let order = match sort.order {
                SortOrder::Ascending => sys::SK_SORT_ASCENDING,
                SortOrder::Descending => sys::SK_SORT_DESCENDING,
            };
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_sort(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    order,
                    match sort.missing {
                        MissingPlacement::Default => sys::SK_SORT_MISSING_DEFAULT,
                        MissingPlacement::First => sys::SK_SORT_MISSING_FIRST,
                        MissingPlacement::Last => sys::SK_SORT_MISSING_LAST,
                    },
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        if let SummaryRequest::Fields(fields) = &request.summary {
            for field in fields {
                let field = cstring(field)?;
                let mut error = NativeError::new();
                let status = unsafe {
                    sys::sk_generic_search_request_add_summary_field(
                        native_request.0.as_ptr(),
                        field.as_ptr(),
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                };
                check(status, &error)?;
            }
        }
        for field in &request.highlight_fields {
            let field = cstring(field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_highlight_field(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for (name, value) in &request.query_features {
            let name = cstring(name)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_query_feature(
                    native_request.0.as_ptr(),
                    name.as_ptr(),
                    *value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for (name, input) in &request.query_inputs {
            let QueryInput::Literal(crate::TensorValue::Dense { type_spec, values }) = input else {
                return Err(Error::message(
                    "sparse, mixed, and expression query inputs are not supported by the current native path",
                ));
            };
            crate::tensor::validate_dense(type_spec, values).map_err(|error| {
                Error::invalid_argument_detail(error.detail_code, error.message)
            })?;
            let name = cstring(name)?;
            let type_spec = cstring(type_spec)?;
            let value_count = u32::try_from(values.len())
                .map_err(|_| Error::message("tensor value count exceeds native capacity"))?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_query_tensor_dense(
                    native_request.0.as_ptr(),
                    name.as_ptr(),
                    type_spec.as_ptr(),
                    values.as_ptr(),
                    value_count,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for (name, value) in &request.rank_properties {
            let name = cstring(name)?;
            let value = cstring(value)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_rank_property(
                    native_request.0.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        for facet in &request.facets {
            let field = cstring(&facet.field)?;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_request_add_grouping(
                    native_request.0.as_ptr(),
                    field.as_ptr(),
                    facet.max_groups,
                    match facet.order {
                        GroupingOrder::None => sys::SK_GROUP_ORDER_NONE,
                        GroupingOrder::CountAscending => sys::SK_GROUP_ORDER_COUNT_ASC,
                        GroupingOrder::CountDescending => sys::SK_GROUP_ORDER_COUNT_DESC,
                        GroupingOrder::ValueAscending => sys::SK_GROUP_ORDER_VALUE_ASC,
                        GroupingOrder::ValueDescending => sys::SK_GROUP_ORDER_VALUE_DESC,
                    },
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
        }
        Ok(native_request)
    }

    fn execute_search(&self, request: &SearchRequest) -> Result<SearchResult, Error> {
        self.execute_search_with_grouping_plans(request, &[])
    }

    fn execute_search_with_grouping_plans(
        &self,
        request: &SearchRequest,
        plans: &[GroupingPlan],
    ) -> Result<SearchResult, Error> {
        validate_search_ranking_inputs(request)?;
        query::validate_query(&request.query)?;
        self.validate_search_fields(request)?;
        if let Source::Named(source) = &request.source {
            if source != &self.document_type {
                return Err(Error::message(format!(
                    "unsupported query source: {source}"
                )));
            }
        }
        let native_query = self.create_native_query(&request.query, request.language)?;
        let native_request = self.create_native_search_request(request)?;
        let native_plans = plans
            .iter()
            .map(|plan| self.create_native_grouping_plan(plan))
            .collect::<Result<Vec<_>, _>>()?;
        for plan in &native_plans {
            generic_search_request_add_grouping_plan(native_request.0, plan)?;
        }
        let mut native_result = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_engine_search_generic(
                self.handle.as_ptr(),
                native_query.0.as_ptr(),
                native_request.0.as_ptr(),
                &mut native_result,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        read_generic_search_result(require_ptr(native_result, "generic search result")?)
    }

    fn create_native_grouping_plan(
        &self,
        plan: &GroupingPlan,
    ) -> Result<crate::native::NativeGroupingPlan, Error> {
        self.validate_grouping_plan(plan)?;
        let native_plan = grouping_plan_create()?;
        grouping_plan_set_all(&native_plan, plan.group_all)?;
        for aggregation in &plan.root_aggregations {
            add_root_grouping_aggregation(&native_plan, aggregation)?;
        }
        for level in &plan.levels {
            let classifier = grouping_expression(&level.classifier)?;
            let native_level = grouping_plan_add_level(&native_plan, &classifier)?;
            add_grouping_level_configuration(native_level, level)?;
            for aggregation in &level.aggregations {
                add_grouping_aggregation(native_level, aggregation)?;
            }
        }
        Ok(native_plan)
    }

    pub fn group(
        &self,
        query: &Query,
        plan: &GroupingPlan,
    ) -> Result<crate::GroupingResult, Error> {
        query::validate_query(query)?;
        self.validate_grouping_plan(plan)?;
        let native_query = self.create_native_query(query, LinguisticsOptions::default())?;
        let native_plan = grouping_plan_create()?;
        grouping_plan_set_all(&native_plan, plan.group_all)?;
        for aggregation in &plan.root_aggregations {
            add_root_grouping_aggregation(&native_plan, aggregation)?;
        }
        for level in &plan.levels {
            let classifier = grouping_expression(&level.classifier)?;
            let native_level = grouping_plan_add_level(&native_plan, &classifier)?;
            add_grouping_level_configuration(native_level, level)?;
            for aggregation in &level.aggregations {
                add_grouping_aggregation(native_level, aggregation)?;
            }
        }
        engine_group_tree(self.handle, native_query.0, &native_plan)
    }

    /// Execute general grouping with the same rank profile, inputs, rank
    /// properties, and timeout as the corresponding search request. The
    /// legacy [`Engine::group`] surface remains available for callers that
    /// intentionally need its query-only compatibility behavior.
    pub fn group_with_request(
        &self,
        request: &SearchRequest,
        plan: &GroupingPlan,
    ) -> Result<crate::GroupingResult, Error> {
        validate_search_ranking_inputs(request)?;
        query::validate_query(&request.query)?;
        self.validate_search_fields(request)?;
        self.validate_grouping_plan(plan)?;
        if let Source::Named(source) = &request.source {
            if source != &self.document_type {
                return Err(Error::message(format!(
                    "unsupported query source: {source}"
                )));
            }
        }
        let native_query = self.create_native_query(&request.query, request.language)?;
        let native_request = self.create_native_search_request(request)?;
        let native_plan = grouping_plan_create()?;
        grouping_plan_set_all(&native_plan, plan.group_all)?;
        for aggregation in &plan.root_aggregations {
            add_root_grouping_aggregation(&native_plan, aggregation)?;
        }
        for level in &plan.levels {
            let classifier = grouping_expression(&level.classifier)?;
            let native_level = grouping_plan_add_level(&native_plan, &classifier)?;
            add_grouping_level_configuration(native_level, level)?;
            for aggregation in &level.aggregations {
                add_grouping_aggregation(native_level, aggregation)?;
            }
        }
        engine_group_tree_with_request(self.handle, native_query.0, native_request.0, &native_plan)
    }

    fn manifest_field(&self, name: &str) -> Result<ManifestField, Error> {
        self.fields
            .get(name)
            .copied()
            .ok_or_else(|| Error::message(format!("unknown field: {name}")))
    }

    fn attribute_field(&self, name: &str) -> Result<ManifestField, Error> {
        let field = self.manifest_field(name)?;
        if !field.attribute {
            return Err(Error::message(format!("field is not an attribute: {name}")));
        }
        Ok(field)
    }

    fn validate_search_fields(&self, request: &SearchRequest) -> Result<(), Error> {
        for sort in &request.sort {
            let field = self.attribute_field(&sort.field)?;
            if !matches!(
                field.field_type,
                ManifestFieldType::String
                    | ManifestFieldType::Integer
                    | ManifestFieldType::Float
                    | ManifestFieldType::Boolean
            ) {
                return Err(Error::message(format!(
                    "unsupported sort field type: {}",
                    sort.field
                )));
            }
        }
        self.validate_unbounded_attribute_range(&request.query)
    }

    fn validate_unbounded_attribute_range(&self, query: &Query) -> Result<(), Error> {
        match query {
            Query::AttributeRange {
                field,
                lower,
                upper,
            } if matches!(lower, crate::AttributeBound::Unbounded)
                && matches!(upper, crate::AttributeBound::Unbounded) =>
            {
                // Vespa represents an unbounded range on both sides as an
                // attribute-exists predicate. Existence is valid for every
                // attribute type, including keyword/string attributes.
                self.attribute_field(field)?;
                Ok(())
            }
            Query::And(children) | Query::Or(children) | Query::WeakAnd { children, .. } => {
                for child in children {
                    self.validate_unbounded_attribute_range(child)?;
                }
                Ok(())
            }
            Query::Not(child) | Query::Labeled { query: child, .. } => {
                self.validate_unbounded_attribute_range(child)
            }
            Query::Rank { matching, ranking } => {
                self.validate_unbounded_attribute_range(matching)?;
                for child in ranking {
                    self.validate_unbounded_attribute_range(child)?;
                }
                Ok(())
            }
            Query::Near { children, .. }
            | Query::ONear { children, .. }
            | Query::SameElement { children, .. } => {
                for child in children {
                    self.validate_unbounded_attribute_range(child)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_grouping_expression(
        &self,
        expression: &GroupingExpression,
        numeric: bool,
    ) -> Result<(), Error> {
        match expression {
            GroupingExpression::Attribute(field) => {
                let field_info = self.attribute_field(field)?;
                let supported = if numeric {
                    matches!(
                        field_info.field_type,
                        ManifestFieldType::Integer | ManifestFieldType::Float
                    )
                } else {
                    matches!(
                        field_info.field_type,
                        ManifestFieldType::String
                            | ManifestFieldType::Integer
                            | ManifestFieldType::Float
                    )
                };
                if !supported {
                    return Err(Error::message(format!(
                        "unsupported grouping field type: {field}"
                    )));
                }
            }
            GroupingExpression::Function { arguments, .. } => {
                for argument in arguments {
                    self.validate_grouping_expression(argument, numeric)?;
                }
            }
            GroupingExpression::Negative(value) => {
                self.validate_grouping_expression(value, numeric)?;
            }
            GroupingExpression::Binary { left, right, .. } => {
                self.validate_grouping_expression(left, numeric)?;
                self.validate_grouping_expression(right, numeric)?;
            }
            GroupingExpression::AggregationReference(_) | GroupingExpression::Literal(_) => {}
        }
        Ok(())
    }

    fn validate_grouping_aggregation(
        &self,
        aggregation: &GroupingAggregation,
    ) -> Result<(), Error> {
        match aggregation {
            GroupingAggregation::Count | GroupingAggregation::Summary { .. } => Ok(()),
            GroupingAggregation::Average(expression)
            | GroupingAggregation::Sum(expression)
            | GroupingAggregation::Min(expression)
            | GroupingAggregation::Max(expression)
            | GroupingAggregation::StandardDeviation(expression)
            | GroupingAggregation::Xor(expression)
            | GroupingAggregation::Quantiles { expression, .. } => {
                self.validate_grouping_expression(expression, true)
            }
        }
    }

    fn validate_grouping_plan(&self, plan: &GroupingPlan) -> Result<(), Error> {
        for aggregation in &plan.root_aggregations {
            self.validate_grouping_aggregation(aggregation)?;
        }
        for level in &plan.levels {
            self.validate_grouping_expression(&level.classifier, false)?;
            for aggregation in &level.aggregations {
                self.validate_grouping_aggregation(aggregation)?;
            }
            for order in &level.order_by {
                self.validate_grouping_expression(&order.expression, false)?;
            }
            if let Some(filter) = &level.filter {
                self.validate_grouping_filter(filter)?;
            }
        }
        Ok(())
    }

    fn validate_grouping_filter(&self, filter: &GroupingFilter) -> Result<(), Error> {
        match filter {
            GroupingFilter::Regex { field, .. } => {
                self.attribute_field(field)?;
            }
            GroupingFilter::RegexExpression { expression, .. }
            | GroupingFilter::IsTrue(expression) => {
                self.validate_grouping_expression(expression, false)?;
            }
            GroupingFilter::And(filters) | GroupingFilter::Or(filters) => {
                for filter in filters {
                    self.validate_grouping_filter(filter)?;
                }
            }
            GroupingFilter::Not(filter) => self.validate_grouping_filter(filter)?,
            GroupingFilter::Range { expression, .. } => {
                self.validate_grouping_expression(expression, true)?;
            }
        }
        Ok(())
    }

    fn create_native_query(
        &self,
        query: &Query,
        language: LinguisticsOptions,
    ) -> Result<NativeQuery, Error> {
        let mut native_query = ptr::null_mut();
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_query_create(
                self.handle.as_ptr(),
                &mut native_query,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let native_query = NativeQuery(require_ptr(native_query, "query")?);
        validate_query_fields(&native_query, query)?;
        lower_query(&native_query, query, language)?;
        Ok(native_query)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { sys::sk_engine_destroy(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod document_decode_tests {
    use super::*;

    #[test]
    fn malformed_composite_payloads_fail_closed() {
        let invalid_string_array = decode_non_tensor_document_value(
            sys::SK_VALUE_STRING_ARRAY,
            br#"{"not":"an array"}"#,
            0,
            0.0,
            false,
        )
        .expect_err("string-array payload must be an array");
        assert!(invalid_string_array
            .to_string()
            .contains("string array is invalid JSON"));

        let invalid_struct = decode_non_tensor_document_value(
            sys::SK_VALUE_STRUCT_ARRAY,
            br#"[{"age":{"type":"mystery","value":37}}]"#,
            0,
            0.0,
            false,
        )
        .expect_err("unknown struct scalar kind must fail");
        assert!(invalid_struct
            .to_string()
            .contains("unknown type 'mystery'"));

        let invalid_position = decode_non_tensor_document_value(
            sys::SK_VALUE_POSITION,
            br#"{"latitude":1.25}"#,
            0,
            0.0,
            false,
        )
        .expect_err("position without longitude must fail");
        assert!(invalid_position
            .to_string()
            .contains("missing numeric field 'longitude'"));

        let invalid_predicate = decode_non_tensor_document_value(
            sys::SK_VALUE_PREDICATE,
            br#"{"key":"hobby","type":4,"feature_set":[1]}"#,
            0,
            0.0,
            false,
        )
        .expect_err("non-string predicate feature must fail");
        assert!(invalid_predicate
            .to_string()
            .contains("predicate feature is not a string"));

        let unknown_kind = decode_non_tensor_document_value(u32::MAX, &[], 0, 0.0, false)
            .expect_err("unknown native kind must fail");
        assert!(unknown_kind.to_string().contains("unknown value kind"));
    }

    #[test]
    fn keyless_predicate_expression_is_not_misclassified_as_feature_form() {
        let expression = r#"{"type":6}"#;
        let decoded = decode_non_tensor_document_value(
            sys::SK_VALUE_PREDICATE,
            expression.as_bytes(),
            0,
            0.0,
            false,
        )
        .expect("Vespa true predicate expression should remain expression-form");
        assert_eq!(
            decoded,
            FieldValue::Predicate(PredicateValue::from_expression(expression))
        );
    }

    #[test]
    fn payload_classification_rejects_unknown_kind_before_dereference() {
        let error = document_value_payload_name(u32::MAX)
            .expect_err("unknown kind must fail before reading a native payload");
        assert!(error.to_string().contains("unknown value kind"));
    }
}
