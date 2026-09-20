use crate::document::{Document, DocumentUpdate, FieldUpdate, FieldValue, ScalarValue};
use crate::error::{check, cstring, path_cstring, Error, Result};
use crate::linguistics::{Linguistics, LinguisticsOptions};
use crate::native::{
    add_document_field, add_text_annotation, add_update_field, engine_group_tree,
    engine_group_tree_with_request, grouping_aggregation_average, grouping_aggregation_count,
    grouping_expression_attribute, grouping_level_add_aggregation, grouping_plan_add_level,
    grouping_plan_create, native_bytes, native_f64s, read_generic_search_result, require_ptr,
    NativeDocument, NativeError, NativeGenericSearchRequest, NativeQuery, NativeUpdate,
};
use crate::query::{self, lower_query, validate_query_fields, Query};
use crate::search::{
    validate_search_ranking_inputs, GroupingAggregation, GroupingExpression, GroupingOrder,
    GroupingPlan, MissingPlacement, QueryInput, SearchRequest, SearchResult, SortOrder, Source,
    SummaryRequest,
};
use crate::tensor::TensorValue;
use searchkernel_vespa_bindings as sys;
use std::fs;
use std::path::Path;
use std::ptr::{self, NonNull};

const EXPECTED_ABI: u32 = sys::EXPECTED_ABI;

fn configured_document_type(config_dir: &Path) -> Result<String, Error> {
    let manifest_path = config_dir.join("searchkernel-schema.json");
    let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
        Error::message(format!(
            "could not read generated schema manifest '{}': {error}",
            manifest_path.display()
        ))
    })?;
    let key = "\"document_type\"";
    let value = manifest
        .find(key)
        .and_then(|key_start| {
            manifest[key_start + key.len()..]
                .find(':')
                .map(|colon| key_start + key.len() + colon + 1)
        })
        .and_then(|value_start| {
            let value = manifest[value_start..].trim_start();
            value.strip_prefix('"').map(|value| value.to_owned())
        })
        .and_then(|value| value.find('"').map(|end| value[..end].to_owned()))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::message("generated schema manifest has no document type"))?;
    Ok(value)
}
pub struct Engine {
    pub(crate) handle: NonNull<sys::sk_engine>,
    document_type: String,
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

        let document_type = configured_document_type(config_dir.as_ref())?;
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
            document_type,
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

    /// Execute an explicit M-layer request and preserve its Vespa-shaped
    /// result without performing a second query in the adapter.
    pub fn search(&self, request: SearchRequest) -> Result<SearchResult, Error> {
        self.execute_search(&request)
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
            let value = match kind {
                sys::SK_VALUE_STRING => {
                    let value =
                        native_bytes(&native, string, string_length, "document field value")?;
                    let value = std::str::from_utf8(value).map_err(|_| {
                        Error::message("native document string value is invalid UTF-8")
                    })?;
                    FieldValue::Scalar(ScalarValue::String(value.to_owned()))
                }
                sys::SK_VALUE_INTEGER => FieldValue::Scalar(ScalarValue::Integer(integer)),
                sys::SK_VALUE_FLOAT => FieldValue::Scalar(ScalarValue::Float(floating)),
                sys::SK_VALUE_BOOLEAN => FieldValue::Scalar(ScalarValue::Boolean(boolean != 0)),
                sys::SK_VALUE_TENSOR => {
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
                }
                _ => {
                    return Err(Error::message(
                        "native document returned an unknown value kind",
                    ))
                }
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
        validate_search_ranking_inputs(request)?;
        query::validate_query(&request.query)?;
        if let Source::Named(source) = &request.source {
            if source != &self.document_type {
                return Err(Error::message(format!(
                    "unsupported query source: {source}"
                )));
            }
        }
        let native_query = self.create_native_query(&request.query, request.language)?;
        let native_request = self.create_native_search_request(request)?;
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

    pub fn group(
        &self,
        query: &Query,
        plan: &GroupingPlan,
    ) -> Result<crate::GroupingResult, Error> {
        query::validate_query(query)?;
        let native_query = self.create_native_query(query, LinguisticsOptions::default())?;
        let native_plan = grouping_plan_create()?;
        for level in &plan.levels {
            let classifier = match &level.classifier {
                GroupingExpression::Attribute(field) => grouping_expression_attribute(field)?,
            };
            let native_level = grouping_plan_add_level(&native_plan, &classifier)?;
            for aggregation in &level.aggregations {
                match aggregation {
                    GroupingAggregation::Count => {
                        let aggregation = grouping_aggregation_count()?;
                        grouping_level_add_aggregation(native_level, &aggregation)?;
                    }
                    GroupingAggregation::Average(expression) => {
                        let expression = match expression {
                            GroupingExpression::Attribute(field) => {
                                grouping_expression_attribute(field)?
                            }
                        };
                        let aggregation = grouping_aggregation_average(&expression)?;
                        grouping_level_add_aggregation(native_level, &aggregation)?;
                    }
                }
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
        for level in &plan.levels {
            let classifier = match &level.classifier {
                GroupingExpression::Attribute(field) => grouping_expression_attribute(field)?,
            };
            let native_level = grouping_plan_add_level(&native_plan, &classifier)?;
            for aggregation in &level.aggregations {
                match aggregation {
                    GroupingAggregation::Count => {
                        let aggregation = grouping_aggregation_count()?;
                        grouping_level_add_aggregation(native_level, &aggregation)?;
                    }
                    GroupingAggregation::Average(expression) => {
                        let expression = match expression {
                            GroupingExpression::Attribute(field) => {
                                grouping_expression_attribute(field)?
                            }
                        };
                        let aggregation = grouping_aggregation_average(&expression)?;
                        grouping_level_add_aggregation(native_level, &aggregation)?;
                    }
                }
            }
        }
        engine_group_tree_with_request(self.handle, native_query.0, native_request.0, &native_plan)
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
