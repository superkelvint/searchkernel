use crate::document::{FieldUpdate, FieldValue, ScalarValue};
use crate::error::{check, cstring, Error, Result};
use crate::linguistics::{DocumentAnalysis, QueryAnalysis};
use crate::query::Value;
use crate::search::{AggregateValue, Group, GroupingResult, Hit, SearchResult, TotalHitsRelation};
use crate::tensor::TensorValue;
use searchkernel_vespa_bindings as sys;
use std::collections::BTreeMap;
use std::ptr::{self, NonNull};

pub(crate) struct NativeError(pub(crate) [u8; sys::SK_ERROR_MESSAGE_CAPACITY as usize]);

impl NativeError {
    pub(crate) fn new() -> Self {
        Self([0; 512])
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.0.len() as u32
    }
}

pub(crate) struct NativeDocument(pub(crate) NonNull<sys::sk_document>);
impl Drop for NativeDocument {
    fn drop(&mut self) {
        unsafe { sys::sk_document_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeUpdate(pub(crate) NonNull<sys::sk_document_update>);
impl Drop for NativeUpdate {
    fn drop(&mut self) {
        unsafe { sys::sk_document_update_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn add_document_field(
    document: *mut sys::sk_document,
    field: &str,
    value: &FieldValue,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match value {
        FieldValue::Scalar(ScalarValue::String(value)) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_add_string(
                    document,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldValue::Scalar(ScalarValue::Integer(value)) => unsafe {
            sys::sk_document_add_integer(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Float(value)) => unsafe {
            sys::sk_document_add_float(
                document,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Scalar(ScalarValue::Boolean(value)) => unsafe {
            sys::sk_document_add_boolean(
                document,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldValue::Array(_) => {
            return Err(Error::message(
                "array document values require a multi-valued schema",
            ));
        }
        FieldValue::Tensor(TensorValue::Dense { type_spec, values }) => {
            let type_spec = cstring(type_spec)?;
            let value_count = u32::try_from(values.len())
                .map_err(|_| Error::message("tensor value count exceeds native capacity"))?;
            unsafe {
                sys::sk_document_add_tensor_dense(
                    document,
                    field.as_ptr(),
                    type_spec.as_ptr(),
                    values.as_ptr(),
                    value_count,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldValue::Tensor(_) => {
            return Err(Error::message(
                "sparse and mixed tensor document values are not supported by the current native path",
            ));
        }
    };
    check(status, &error)
}

pub(crate) fn add_update_field(
    update: *mut sys::sk_document_update,
    field: &str,
    mutation: &FieldUpdate,
) -> Result<(), Error> {
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = match mutation {
        FieldUpdate::Clear => unsafe {
            sys::sk_document_update_clear(
                update,
                field.as_ptr(),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::String(value))) => {
            let value = cstring(value)?;
            unsafe {
                sys::sk_document_update_assign_string(
                    update,
                    field.as_ptr(),
                    value.as_ptr(),
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            }
        }
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Integer(value))) => unsafe {
            sys::sk_document_update_assign_integer(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Float(value))) => unsafe {
            sys::sk_document_update_assign_float(
                update,
                field.as_ptr(),
                *value,
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Scalar(ScalarValue::Boolean(value))) => unsafe {
            sys::sk_document_update_assign_boolean(
                update,
                field.as_ptr(),
                u8::from(*value),
                error.as_mut_ptr(),
                error.capacity(),
            )
        },
        FieldUpdate::Assign(FieldValue::Array(_)) => {
            return Err(Error::message(
                "array document updates require a multi-valued schema",
            ));
        }
        FieldUpdate::Assign(FieldValue::Tensor(_)) => {
            return Err(Error::message(
                "tensor document updates are not supported by the current native document ABI",
            ));
        }
    };
    check(status, &error)
}

pub(crate) fn add_text_annotation<T>(
    handle: *mut T,
    field: &str,
    analysis: &DocumentAnalysis,
    update: bool,
) -> Result<(), Error> {
    let occurrences = text_occurrences(analysis)?;
    let field = cstring(field)?;
    let mut error = NativeError::new();
    let status = unsafe {
        if update {
            sys::sk_document_update_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        } else {
            sys::sk_document_annotate_text(
                handle.cast(),
                field.as_ptr(),
                occurrences.term_bytes.as_ptr(),
                length_u32(occurrences.term_bytes.len(), "text term bytes")?,
                occurrences.term_lengths.as_ptr(),
                occurrences.byte_starts.as_ptr(),
                occurrences.byte_lengths.as_ptr(),
                length_u32(occurrences.term_lengths.len(), "text occurrence")?,
                error.as_mut_ptr(),
                error.capacity(),
            )
        }
    };
    check(status, &error)
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

pub(crate) fn require_ptr<T>(pointer: *mut T, name: &str) -> Result<NonNull<T>, Error> {
    NonNull::new(pointer)
        .ok_or_else(|| Error::message(format!("native operation returned a null {name}")))
}

pub(crate) fn native_bytes<'a>(
    _owner: &'a NativeDocument,
    pointer: *const std::os::raw::c_char,
    length: u32,
    name: &str,
) -> Result<&'a [u8], Error> {
    if length == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() {
        return Err(Error::message(format!(
            "native operation returned a null {name}"
        )));
    }
    // SAFETY: the native result contract keeps the returned bytes alive for
    // the lifetime of its owning opaque result, which the caller holds while
    // consuming this slice.
    Ok(unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length as usize) })
}

pub(crate) struct NativeQuery(pub(crate) NonNull<sys::sk_query>);
impl Drop for NativeQuery {
    fn drop(&mut self) {
        unsafe { sys::sk_query_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeGenericSearchRequest(pub(crate) NonNull<sys::sk_generic_search_request>);
impl Drop for NativeGenericSearchRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_search_request_destroy(self.0.as_ptr()) };
    }
}

pub(crate) struct NativeGenericGroupingRequest(
    pub(crate) NonNull<sys::sk_generic_grouping_request>,
);
impl Drop for NativeGenericGroupingRequest {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_grouping_request_destroy(self.0.as_ptr()) };
    }
}

pub(crate) fn read_generic_search_result(
    pointer: NonNull<sys::sk_generic_search_result>,
) -> Result<SearchResult, Error> {
    let result = NativeGenericSearchResult(pointer);
    let hit_count = unsafe { sys::sk_generic_search_result_hit_count(result.0.as_ptr()) } as usize;
    let total_hit_count =
        unsafe { sys::sk_generic_search_result_total_hit_count(result.0.as_ptr()) };
    let coverage = crate::search::SearchCoverage {
        active: unsafe { sys::sk_generic_search_result_coverage_active(result.0.as_ptr()) },
        covered: unsafe { sys::sk_generic_search_result_coverage_covered(result.0.as_ptr()) },
        target_active: unsafe {
            sys::sk_generic_search_result_coverage_target_active(result.0.as_ptr())
        },
        degradation_flags: unsafe {
            sys::sk_generic_search_result_degradation_flags(result.0.as_ptr())
        },
    };
    let issue_count =
        unsafe { sys::sk_generic_search_result_issue_count(result.0.as_ptr()) } as usize;
    let mut issues = Vec::with_capacity(issue_count);
    for issue_index in 0..issue_count {
        let mut message = ptr::null();
        let mut message_length = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_issue_message(
                result.0.as_ptr(),
                issue_index as u32,
                &mut message,
                &mut message_length,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if message.is_null() && message_length != 0 {
            return Err(Error::message(
                "native search result issue returned null message",
            ));
        }
        let bytes = if message_length == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(message.cast::<u8>(), message_length as usize) }
        };
        issues.push(
            String::from_utf8(bytes.to_vec())
                .map_err(|_| Error::message("native search result issue is invalid UTF-8"))?,
        );
    }
    let match_feature_count =
        unsafe { sys::sk_generic_search_result_match_feature_count(result.0.as_ptr()) } as usize;
    let mut match_feature_names = Vec::with_capacity(match_feature_count);
    for feature_index in 0..match_feature_count {
        let mut name = ptr::null();
        let mut name_length = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_match_feature_name(
                result.0.as_ptr(),
                feature_index as u32,
                &mut name,
                &mut name_length,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if name.is_null() {
            return Err(Error::message("native match feature returned null name"));
        }
        let bytes = unsafe { std::slice::from_raw_parts(name.cast::<u8>(), name_length as usize) };
        let name = String::from_utf8(bytes.to_vec())
            .map_err(|_| Error::message("native match feature returned invalid UTF-8 name"))?;
        if match_feature_names.iter().any(|previous| previous == &name) {
            return Err(Error::message(
                "native match features contain duplicate names",
            ));
        }
        match_feature_names.push(name);
    }
    let mut hits = Vec::with_capacity(hit_count);
    for index in 0..hit_count {
        let mut id = ptr::null();
        let mut id_length = 0;
        let mut score = 0.0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_hit(
                result.0.as_ptr(),
                index as u32,
                &mut id,
                &mut id_length,
                &mut score,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        if id.is_null() {
            return Err(Error::message(
                "native local result returned null document ID",
            ));
        }
        let id = unsafe { std::slice::from_raw_parts(id.cast::<u8>(), id_length as usize) };
        let document_id = String::from_utf8(id.to_vec())
            .map_err(|_| Error::message("native result returned invalid UTF-8"))?;
        let summaries =
            unsafe { sys::sk_generic_search_result_summary_count(result.0.as_ptr(), index as u32) }
                as usize;
        let mut summary = BTreeMap::new();
        for summary_index in 0..summaries {
            let mut name = ptr::null();
            let mut name_length = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_summary_name(
                    result.0.as_ptr(),
                    index as u32,
                    summary_index as u32,
                    &mut name,
                    &mut name_length,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            if name.is_null() {
                return Err(Error::message("native summary returned null field name"));
            }
            let name_bytes =
                unsafe { std::slice::from_raw_parts(name.cast::<u8>(), name_length as usize) };
            let name = String::from_utf8(name_bytes.to_vec())
                .map_err(|_| Error::message("native summary returned invalid UTF-8 field name"))?;
            let mut kind = 0;
            let mut string = ptr::null();
            let mut string_length = 0;
            let mut integer = 0;
            let mut float = 0.0;
            let mut boolean = 0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_summary_value(
                    result.0.as_ptr(),
                    index as u32,
                    summary_index as u32,
                    &mut kind,
                    &mut string,
                    &mut string_length,
                    &mut integer,
                    &mut float,
                    &mut boolean,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            let value = match kind {
                sys::SK_VALUE_STRING => {
                    if string.is_null() {
                        return Err(Error::message("native summary returned null string"));
                    }
                    let bytes = unsafe {
                        std::slice::from_raw_parts(string.cast::<u8>(), string_length as usize)
                    };
                    Value::String(
                        String::from_utf8(bytes.to_vec())
                            .map_err(|_| Error::message("native summary returned invalid UTF-8"))?,
                    )
                }
                sys::SK_VALUE_INTEGER => Value::Integer(integer),
                sys::SK_VALUE_FLOAT => Value::Float(float),
                sys::SK_VALUE_BOOLEAN => Value::Boolean(boolean != 0),
                _ => return Err(Error::message("native summary returned unknown value kind")),
            };
            summary.insert(name, value);
        }
        let mut match_features = BTreeMap::new();
        for (feature_index, feature_name) in match_feature_names.iter().enumerate() {
            let mut value = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_match_feature_value(
                    result.0.as_ptr(),
                    index as u32,
                    feature_index as u32,
                    &mut value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            match_features.insert(feature_name.clone(), value);
        }
        hits.push(Hit {
            document_id,
            score,
            match_features,
            fields: summary
                .into_iter()
                .map(|(name, value)| (name, FieldValue::Scalar(value)))
                .collect(),
        });
    }
    let grouping_count = unsafe { sys::sk_generic_search_result_grouping_count(result.0.as_ptr()) };
    let mut facets = Vec::with_capacity(grouping_count as usize);
    for grouping_index in 0..grouping_count {
        facets.push(read_search_facet(result.0.as_ptr(), grouping_index)?);
    }
    Ok(SearchResult {
        total_hits: total_hit_count,
        total_hits_relation: TotalHitsRelation::Exact,
        hits,
        facets,
        coverage,
        issues,
    })
}

struct NativeGenericSearchResult(NonNull<sys::sk_generic_search_result>);
impl Drop for NativeGenericSearchResult {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_search_result_destroy(self.0.as_ptr()) };
    }
}

fn read_search_facet(
    result: *const sys::sk_generic_search_result,
    grouping_index: u32,
) -> Result<GroupingResult, Error> {
    let group_count =
        unsafe { sys::sk_generic_search_result_group_count(result, grouping_index) } as usize;
    let mut groups = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let mut key_kind = 0;
        let mut key_string = ptr::null();
        let mut key_string_length = 0;
        let mut key_integer = 0;
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_search_result_group(
                result,
                grouping_index,
                group_index as u32,
                &mut key_kind,
                &mut key_string,
                &mut key_string_length,
                &mut key_integer,
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let key = match key_kind {
            sys::SK_VALUE_STRING => {
                if key_string.is_null() {
                    return Err(Error::message("native search returned null facet key"));
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(key_string.cast::<u8>(), key_string_length as usize)
                };
                Value::String(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| Error::message("native search returned invalid facet key"))?,
                )
            }
            sys::SK_VALUE_INTEGER => Value::Integer(key_integer),
            sys::SK_VALUE_BOOLEAN => Value::Boolean(key_integer != 0),
            _ => {
                return Err(Error::message(
                    "native search returned unsupported facet key kind",
                ))
            }
        };
        groups.push(Group {
            key,
            count,
            aggregations: Vec::new(),
        });
    }
    Ok(GroupingResult { groups })
}

pub(crate) fn read_generic_grouping_result(
    pointer: NonNull<sys::sk_generic_grouping_result>,
) -> Result<GroupingResult, Error> {
    let result = NativeGenericGroupingResult(pointer);
    let group_count =
        unsafe { sys::sk_generic_grouping_result_group_count(result.0.as_ptr()) } as usize;
    let mut groups = Vec::with_capacity(group_count);
    for group_index in 0..group_count {
        let mut key_kind = 0;
        let mut key_string = ptr::null();
        let mut key_string_length = 0;
        let mut key_integer = 0;
        let mut count = 0;
        let mut error = NativeError::new();
        let status = unsafe {
            sys::sk_generic_grouping_result_group(
                result.0.as_ptr(),
                group_index as u32,
                &mut key_kind,
                &mut key_string,
                &mut key_string_length,
                &mut key_integer,
                &mut count,
                error.as_mut_ptr(),
                error.capacity(),
            )
        };
        check(status, &error)?;
        let key = match key_kind {
            sys::SK_VALUE_STRING => {
                if key_string.is_null() {
                    return Err(Error::message("native grouping returned null string key"));
                }
                let bytes = unsafe {
                    std::slice::from_raw_parts(key_string.cast::<u8>(), key_string_length as usize)
                };
                Value::String(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| Error::message("native grouping returned invalid UTF-8"))?,
                )
            }
            sys::SK_VALUE_INTEGER => Value::Integer(key_integer),
            _ => {
                return Err(Error::message(
                    "native grouping returned unsupported key kind",
                ));
            }
        };
        let aggregation_count = unsafe {
            sys::sk_generic_grouping_result_aggregation_count(result.0.as_ptr(), group_index as u32)
        } as usize;
        let mut aggregations = Vec::with_capacity(aggregation_count);
        for aggregation_index in 0..aggregation_count {
            let mut kind = 0;
            let mut integer = 0;
            let mut float = 0.0;
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_grouping_result_aggregation(
                    result.0.as_ptr(),
                    group_index as u32,
                    aggregation_index as u32,
                    &mut kind,
                    &mut integer,
                    &mut float,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            aggregations.push(match kind {
                sys::SK_VALUE_INTEGER => AggregateValue::Integer(integer),
                sys::SK_VALUE_FLOAT => AggregateValue::Float(float),
                _ => {
                    return Err(Error::message(
                        "native grouping returned unknown aggregate kind",
                    ));
                }
            });
        }
        groups.push(Group {
            key,
            count,
            aggregations,
        });
    }
    Ok(GroupingResult { groups })
}

struct NativeGenericGroupingResult(NonNull<sys::sk_generic_grouping_result>);
impl Drop for NativeGenericGroupingResult {
    fn drop(&mut self) {
        unsafe { sys::sk_generic_grouping_result_destroy(self.0.as_ptr()) };
    }
}
