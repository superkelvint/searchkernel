#[cfg(test)]
use super::document::validate_position_precision;
use super::*;

pub(super) fn packed_strings(values: &[String]) -> Result<(Vec<u8>, Vec<u32>), Error> {
    let mut bytes = Vec::new();
    let mut lengths = Vec::with_capacity(values.len());
    for value in values {
        lengths.push(
            u32::try_from(value.len())
                .map_err(|_| Error::message("string array element exceeds native capacity"))?,
        );
        bytes.extend_from_slice(value.as_bytes());
    }
    Ok((bytes, lengths))
}

type PackedPredicate = (Vec<u8>, Vec<u32>, Vec<u8>, Vec<u32>);

fn validate_predicate_expression_numbers(value: &serde_json::Value) -> Result<(), Error> {
    match value {
        serde_json::Value::Number(number) if number.as_i64().is_none() => Err(Error::unsupported(
            "predicate expression numbers must be signed 64-bit integers",
        )),
        serde_json::Value::Array(values) => {
            for value in values {
                validate_predicate_expression_numbers(value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                validate_predicate_expression_numbers(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(super) fn native_predicate_expression(expression: &str) -> Result<String, Error> {
    let parsed = serde_json::from_str::<serde_json::Value>(expression).map_err(|error| {
        Error::message(format!("predicate expression is invalid JSON: {error}"))
    })?;
    let object = parsed
        .as_object()
        .ok_or_else(|| Error::message("predicate expression must be a JSON object"))?;
    validate_predicate_expression_numbers(&parsed)?;
    let Some(kind) = object.get("type") else {
        return Ok(expression.to_owned());
    };
    let mut remainder = object.clone();
    remainder.remove("type");
    let remainder =
        serde_json::to_string(&serde_json::Value::Object(remainder)).map_err(|error| {
            Error::message(format!(
                "predicate expression could not be encoded: {error}"
            ))
        })?;
    let kind = serde_json::to_string(kind).map_err(|error| {
        Error::message(format!(
            "predicate expression type could not be encoded: {error}"
        ))
    })?;
    let mut result = format!("{{\"type\":{kind}");
    if remainder.len() > 2 {
        result.push(',');
        result.push_str(&remainder[1..remainder.len() - 1]);
    }
    result.push('}');
    Ok(result)
}

pub(super) fn packed_predicate(value: &PredicateValue) -> Result<PackedPredicate, Error> {
    if value.features.len() != 1 {
        return Err(Error::unsupported(
            "predicate feature form requires exactly one feature key; use predicate expression form for composed predicates",
        ));
    }
    let keys: Vec<String> = value.features.keys().cloned().collect();
    let values: Vec<String> = value.features.values().cloned().collect();
    let (key_bytes, key_lengths) = packed_strings(&keys)?;
    let (value_bytes, value_lengths) = packed_strings(&values)?;
    Ok((key_bytes, key_lengths, value_bytes, value_lengths))
}

pub(crate) fn length_u32(length: usize, what: &str) -> Result<u32, Error> {
    u32::try_from(length)
        .map_err(|_| Error::message(format!("{what} count exceeds native ABI capacity")))
}

pub(crate) struct NativeOccurrences {
    pub(crate) term_bytes: Vec<u8>,
    pub(crate) term_lengths: Vec<u32>,
    pub(crate) byte_starts: Vec<u32>,
    pub(crate) byte_lengths: Vec<u32>,
}

pub(crate) struct NativeArrayOccurrences {
    pub(crate) term_bytes: Vec<u8>,
    pub(crate) term_lengths: Vec<u32>,
    pub(crate) byte_starts: Vec<u32>,
    pub(crate) byte_lengths: Vec<u32>,
    pub(crate) element_offsets: Vec<u32>,
}

impl NativeArrayOccurrences {
    pub(super) fn new() -> Self {
        Self {
            term_bytes: Vec::new(),
            term_lengths: Vec::new(),
            byte_starts: Vec::new(),
            byte_lengths: Vec::new(),
            element_offsets: vec![0],
        }
    }

    pub(super) fn push(&mut self, analysis: &DocumentAnalysis) -> Result<(), Error> {
        for term in &analysis.terms {
            self.term_lengths
                .push(length_u32(term.term.len(), "array text term")?);
            self.term_bytes.extend_from_slice(term.term.as_bytes());
            self.byte_starts
                .push(length_u32(term.byte_start, "array text span")?);
            self.byte_lengths
                .push(length_u32(term.byte_len, "array text span")?);
        }
        self.element_offsets
            .push(length_u32(self.term_lengths.len(), "array occurrence")?);
        Ok(())
    }
}

pub(crate) fn text_occurrences(analysis: &DocumentAnalysis) -> Result<NativeOccurrences, Error> {
    let mut result = NativeOccurrences {
        term_bytes: Vec::new(),
        term_lengths: Vec::with_capacity(analysis.terms.len()),
        byte_starts: Vec::with_capacity(analysis.terms.len()),
        byte_lengths: Vec::with_capacity(analysis.terms.len()),
    };
    for term in &analysis.terms {
        result
            .term_lengths
            .push(length_u32(term.term.len(), "text term")?);
        result.term_bytes.extend_from_slice(term.term.as_bytes());
        result
            .byte_starts
            .push(length_u32(term.byte_start, "text span")?);
        result
            .byte_lengths
            .push(length_u32(term.byte_len, "text span")?);
    }
    Ok(result)
}

pub(crate) struct NativeQueryTerms {
    pub(crate) term_bytes: Vec<u8>,
    pub(crate) term_lengths: Vec<u32>,
}

pub(crate) fn text_query_terms(analysis: &QueryAnalysis) -> Result<NativeQueryTerms, Error> {
    let mut result = NativeQueryTerms {
        term_bytes: Vec::new(),
        term_lengths: Vec::with_capacity(analysis.terms.len()),
    };
    for term in &analysis.terms {
        let stem = term.stems.first().ok_or_else(|| {
            Error::message("M4 query analysis returned no stem for a query position")
        })?;
        if term.stems.len() != 1 {
            return Err(Error::message(
                "M4 query analysis returned multiple stems for a query position",
            ));
        }
        result
            .term_lengths
            .push(length_u32(stem.len(), "text query term")?);
        result.term_bytes.extend_from_slice(stem.as_bytes());
    }
    Ok(result)
}

pub(crate) fn validate_term_query(analysis: &QueryAnalysis) -> Result<(), Error> {
    if analysis.terms.len() == 1 {
        Ok(())
    } else {
        Err(Error::message(
            "term query must analyze to exactly one term",
        ))
    }
}

pub(super) fn validate_native_slice_pointer<T>(
    pointer: *const T,
    length: u32,
    name: &str,
) -> Result<(), Error> {
    if length != 0 && pointer.is_null() {
        return Err(Error::invalid_result(format!(
            "native operation returned a null {name}"
        )));
    }
    Ok(())
}

pub(crate) fn native_bytes<'a>(
    _owner: &'a NativeDocument,
    pointer: *const std::os::raw::c_char,
    length: u32,
    name: &str,
) -> Result<&'a [u8], Error> {
    validate_native_slice_pointer(pointer, length, name)?;
    if length == 0 {
        return Ok(&[]);
    }
    // SAFETY: the native result contract keeps the returned bytes alive for
    // the lifetime of its owning opaque result, which the caller holds while
    // consuming this slice.
    Ok(unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length as usize) })
}

pub(crate) fn native_f64s<'a>(
    _owner: &'a NativeDocument,
    pointer: *const f64,
    length: u32,
    name: &str,
) -> Result<&'a [f64], Error> {
    validate_native_slice_pointer(pointer, length, name)?;
    if length == 0 {
        return Ok(&[]);
    }
    // SAFETY: the native result contract keeps the returned cells alive for
    // the lifetime of its owning opaque document, which the caller holds.
    Ok(unsafe { std::slice::from_raw_parts(pointer, length as usize) })
}

#[cfg(test)]
mod native_slice_tests {
    use super::{
        native_predicate_expression, packed_predicate, packed_strings,
        validate_native_slice_pointer, validate_position_precision,
    };
    use crate::document::PredicateValue;
    use std::collections::BTreeMap;
    use std::ptr;

    #[test]
    fn predicate_expression_normalization_preserves_expression_provenance() {
        assert_eq!(
            native_predicate_expression(r#"{"key":"hobby","type":4,"feature_set":["music"]}"#)
                .expect("predicate expression should normalize"),
            r#"{"type":4,"feature_set":["music"],"key":"hobby"}"#
        );
        assert_eq!(
            native_predicate_expression(r#"{"type":6}"#)
                .expect("keyless predicate expression should normalize"),
            r#"{"type":6}"#
        );
        assert!(native_predicate_expression("[]").is_err());
        assert!(
            native_predicate_expression(
                r#"{"type":4,"key":"hobby","feature_set":["music"],"weight":1.23456789}"#
            )
            .is_err(),
            "floating predicate JSON must fail before Vespa's lossy JSON result encoder"
        );
    }

    #[test]
    fn predicate_feature_form_requires_exactly_one_key() {
        let single =
            PredicateValue::new(BTreeMap::from([("hobby".to_owned(), "music".to_owned())]));
        packed_predicate(&single).expect("single predicate feature key is representable");

        for features in [
            BTreeMap::new(),
            BTreeMap::from([
                ("hobby".to_owned(), "music".to_owned()),
                ("region".to_owned(), "us".to_owned()),
            ]),
        ] {
            let error = packed_predicate(&PredicateValue::new(features))
                .expect_err("lossy predicate feature map must fail before FFI");
            assert!(error.to_string().contains("exactly one feature key"));
        }
    }

    #[test]
    fn packed_document_buffers_report_exact_utf8_byte_totals() {
        let (bytes, lengths) = packed_strings(&["a".into(), String::new(), "é".into()])
            .expect("string array should pack");
        assert_eq!(bytes, [b'a', 0xc3, 0xa9]);
        assert_eq!(lengths, [1, 0, 2]);
        assert_eq!(
            lengths.iter().map(|length| *length as usize).sum::<usize>(),
            bytes.len()
        );

        let predicate = PredicateValue::new(BTreeMap::from([("k-é".to_owned(), "值".to_owned())]));
        let (key_bytes, key_lengths, value_bytes, value_lengths) =
            packed_predicate(&predicate).expect("predicate feature should pack");
        assert_eq!(
            key_lengths
                .iter()
                .map(|length| *length as usize)
                .sum::<usize>(),
            key_bytes.len()
        );
        assert_eq!(
            value_lengths
                .iter()
                .map(|length| *length as usize)
                .sum::<usize>(),
            value_bytes.len()
        );
        assert_eq!(key_bytes, "k-é".as_bytes());
        assert_eq!(value_bytes, "值".as_bytes());
    }

    #[test]
    fn position_precision_must_be_exactly_representable_by_vespa() {
        validate_position_precision(37.774929, -122.419416)
            .expect("microdegree coordinates are representable");

        let latitude = validate_position_precision(37.7749294, -122.419416)
            .expect_err("sub-microdegree latitude must fail before FFI");
        assert!(latitude.to_string().contains("latitude"));

        let longitude = validate_position_precision(37.774929, -122.4194164)
            .expect_err("sub-microdegree longitude must fail before FFI");
        assert!(longitude.to_string().contains("longitude"));
    }

    #[test]
    fn nonzero_native_length_requires_non_null_pointer() {
        let error =
            validate_native_slice_pointer::<u8>(ptr::null(), 1, "document composite payload")
                .expect_err("nonzero length with null pointer must fail");
        assert!(error
            .to_string()
            .contains("null document composite payload"));
    }

    #[test]
    fn zero_native_length_allows_null_pointer_without_dereference() {
        validate_native_slice_pointer::<u8>(ptr::null(), 0, "empty payload")
            .expect("zero-length payload does not dereference its pointer");
    }
}
