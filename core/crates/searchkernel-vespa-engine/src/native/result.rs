use super::grouping::grouping_tree_node;
use super::*;

pub(crate) fn read_generic_search_result(
    pointer: NonNull<sys::sk_generic_search_result>,
) -> Result<SearchResult, Error> {
    let result = NativeGenericSearchResult(pointer);
    let mut trace = ptr::null();
    let mut length = 0;
    let mut truncated = 0;
    let mut matching_seconds = 0.0;
    let mut summary_seconds = 0.0;
    let mut query_seconds = 0.0;
    let mut error = NativeError::new();
    let status = unsafe {
        sys::sk_generic_search_result_diagnostics(
            result.0.as_ptr(),
            &mut trace,
            &mut length,
            &mut truncated,
            &mut matching_seconds,
            &mut summary_seconds,
            &mut query_seconds,
            error.as_mut_ptr(),
            error.capacity(),
        )
    };
    check(status, &error)?;
    if truncated & !3 != 0
        || length > 262_144
        || [matching_seconds, summary_seconds, query_seconds]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(Error::message("native diagnostic metadata is invalid"));
    }
    let diagnostics = if query_seconds == 0.0 && length == 0 && truncated == 0 {
        None
    } else {
        // The opaque result owns this bounded buffer. Copy before its Drop.
        if trace.is_null() && length != 0 {
            return Err(Error::message(
                "native diagnostic trace returned a null buffer",
            ));
        }
        let bytes = if length == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(trace, length as usize) }
        };
        Some(crate::search::SearchDiagnostics {
            trace: match std::str::from_utf8(bytes) {
                Ok(value) => value.to_owned(),
                Err(error) if truncated & 1 != 0 => {
                    std::str::from_utf8(&bytes[..error.valid_up_to()])
                        .map_err(|_| Error::message("invalid truncated trace"))?
                        .to_owned()
                }
                Err(_) => return Err(Error::message("native trace is not UTF-8")),
            },
            truncated: truncated & 1 != 0,
            features_truncated: truncated & 2 != 0,
            matching_seconds,
            summary_seconds,
            query_seconds,
        })
    };
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
        let highlight_count = unsafe {
            sys::sk_generic_search_result_highlight_count(result.0.as_ptr(), index as u32)
        } as usize;
        let mut highlights = Vec::with_capacity(highlight_count);
        for highlight_index in 0..highlight_count {
            let mut field = ptr::null();
            let mut field_length = 0;
            let mut error = NativeError::new();
            check(
                unsafe {
                    sys::sk_generic_search_result_highlight_field(
                        result.0.as_ptr(),
                        index as u32,
                        highlight_index as u32,
                        &mut field,
                        &mut field_length,
                        error.as_mut_ptr(),
                        error.capacity(),
                    )
                },
                &error,
            )?;
            if field.is_null() && field_length != 0 {
                return Err(Error::message("native highlight returned null field"));
            }
            let field_bytes = if field_length == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(field.cast::<u8>(), field_length as usize) }
            };
            let field = String::from_utf8(field_bytes.to_vec())
                .map_err(|_| Error::message("native highlight field is invalid UTF-8"))?;
            let fragment_count = unsafe {
                sys::sk_generic_search_result_highlight_fragment_count(
                    result.0.as_ptr(),
                    index as u32,
                    highlight_index as u32,
                )
            } as usize;
            let mut fragments = Vec::with_capacity(fragment_count);
            for fragment_index in 0..fragment_count {
                let mut text = ptr::null();
                let mut text_length = 0;
                let mut error = NativeError::new();
                check(
                    unsafe {
                        sys::sk_generic_search_result_highlight_fragment_text(
                            result.0.as_ptr(),
                            index as u32,
                            highlight_index as u32,
                            fragment_index as u32,
                            &mut text,
                            &mut text_length,
                            error.as_mut_ptr(),
                            error.capacity(),
                        )
                    },
                    &error,
                )?;
                if text.is_null() && text_length != 0 {
                    return Err(Error::message(
                        "native highlight returned null fragment text",
                    ));
                }
                let text_bytes = if text_length == 0 {
                    &[][..]
                } else {
                    unsafe { std::slice::from_raw_parts(text.cast::<u8>(), text_length as usize) }
                };
                let text = String::from_utf8(text_bytes.to_vec())
                    .map_err(|_| Error::message("native highlight text is invalid UTF-8"))?;
                let match_count = unsafe {
                    sys::sk_generic_search_result_highlight_match_count(
                        result.0.as_ptr(),
                        index as u32,
                        highlight_index as u32,
                        fragment_index as u32,
                    )
                } as usize;
                let mut matches = Vec::with_capacity(match_count);
                for match_index in 0..match_count {
                    let mut start_byte = 0;
                    let mut end_byte = 0;
                    let mut error = NativeError::new();
                    check(
                        unsafe {
                            sys::sk_generic_search_result_highlight_match(
                                result.0.as_ptr(),
                                index as u32,
                                highlight_index as u32,
                                fragment_index as u32,
                                match_index as u32,
                                &mut start_byte,
                                &mut end_byte,
                                error.as_mut_ptr(),
                                error.capacity(),
                            )
                        },
                        &error,
                    )?;
                    if start_byte > end_byte || end_byte as usize > text.len() {
                        return Err(Error::message("native highlight span is out of bounds"));
                    }
                    matches.push(HighlightSpan {
                        start_byte,
                        end_byte,
                    });
                }
                fragments.push(HighlightFragment { text, matches });
            }
            highlights.push(HighlightResult { field, fragments });
        }
        let mut match_features = BTreeMap::new();
        for (feature_index, feature_name) in match_feature_names.iter().enumerate() {
            let mut native_value: *const sys::sk_feature_value = ptr::null();
            let mut error = NativeError::new();
            let status = unsafe {
                sys::sk_generic_search_result_match_feature_value_ref(
                    result.0.as_ptr(),
                    index as u32,
                    feature_index as u32,
                    &mut native_value,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            };
            check(status, &error)?;
            if native_value.is_null() {
                return Err(Error::message("native match feature returned null value"));
            }
            let value = match unsafe { sys::sk_feature_value_kind(native_value) } {
                sys::SK_FEATURE_VALUE_DOUBLE => {
                    let mut value = 0.0;
                    check(
                        unsafe {
                            sys::sk_feature_value_double(
                                native_value,
                                &mut value,
                                error.as_mut_ptr(),
                                error.capacity(),
                            )
                        },
                        &error,
                    )?;
                    MatchFeatureValue::Double(value)
                }
                sys::SK_FEATURE_VALUE_DATA => {
                    let mut data = ptr::null();
                    let mut size = 0_usize;
                    check(
                        unsafe {
                            sys::sk_feature_value_data(
                                native_value,
                                &mut data,
                                &mut size,
                                error.as_mut_ptr(),
                                error.capacity(),
                            )
                        },
                        &error,
                    )?;
                    if size != 0 && data.is_null() {
                        return Err(Error::message("native match feature returned null data"));
                    }
                    let bytes = if size == 0 {
                        &[][..]
                    } else {
                        unsafe { std::slice::from_raw_parts(data, size) }
                    };
                    MatchFeatureValue::Data(bytes.to_vec())
                }
                _ => return Err(Error::message("native match feature returned unknown kind")),
            };
            match_features.insert(feature_name.clone(), value);
        }
        hits.push(Hit {
            document_id,
            score,
            match_features,
            highlights,
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
    let grouping_tree_count =
        unsafe { sys::sk_generic_search_result_grouping_tree_count(result.0.as_ptr()) };
    let mut groupings = Vec::with_capacity(grouping_tree_count as usize);
    for grouping_index in 0..grouping_tree_count {
        let mut root = ptr::null();
        let mut error = NativeError::new();
        check(
            unsafe {
                sys::sk_generic_search_result_grouping_tree_root(
                    result.0.as_ptr(),
                    grouping_index,
                    &mut root,
                    error.as_mut_ptr(),
                    error.capacity(),
                )
            },
            &error,
        )?;
        groupings.push(grouping_tree_node(root, false)?);
    }
    Ok(SearchResult {
        total_hits: total_hit_count,
        // SearchReply exposes no result-side hit-count accuracy bit.  Coverage
        // and degradation describe how much of the corpus was searched, but
        // do not prove that totalHitCount is exact.  Keep the uncertainty
        // explicit rather than fabricating Exact or misclassifying it as a
        // LowerBound.
        total_hits_relation: TotalHitsRelation::Unknown,
        hits,
        facets,
        groupings,
        coverage,
        issues,
        diagnostics,
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
) -> Result<FacetResult, Error> {
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
        groups.push(FacetGroup { key, count });
    }
    Ok(FacetResult { groups })
}
