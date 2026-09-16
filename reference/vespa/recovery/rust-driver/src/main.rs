use flatbuffers::root;
use searchkernel::{Field, IndexDefinition, IndexRuntime};
use searchkernel_protocol::v1 as api;
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::CString;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type AnyResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BASE_1: &str = "recovery-base-1";
const BASE_2: &str = "recovery-base-2";
const BASE_3: &str = "recovery-base-3";
const INSERT_TARGET: &str = "recovery-insert-target";
const UPDATE_VALUE: i64 = 40;
const MATRIX_REPETITIONS: usize = 3;
const REPEATED_CYCLES: usize = 20;

fn insert_target(cycle: usize) -> String {
    if cycle == usize::MAX {
        INSERT_TARGET.to_owned()
    } else {
        format!("recovery-insert-{cycle}")
    }
}

#[allow(warnings)]
mod ffi_wire {
    pub mod search_kernel {
        pub mod ffi {
            pub mod v1 {
                pub use searchkernel_protocol::v1::*;
                mod request_payload_generated {
                    include!("../../../../../core/crates/searchkernel-ffi/generated/search_kernel/ffi/v1/request_payload_generated.rs");
                }
                pub use request_payload_generated::*;
                mod request_envelope_generated {
                    include!("../../../../../core/crates/searchkernel-ffi/generated/search_kernel/ffi/v1/request_envelope_generated.rs");
                }
                pub use request_envelope_generated::*;
                mod response_payload_generated {
                    include!("../../../../../core/crates/searchkernel-ffi/generated/search_kernel/ffi/v1/response_payload_generated.rs");
                }
                pub use response_payload_generated::*;
                mod response_envelope_generated {
                    include!("../../../../../core/crates/searchkernel-ffi/generated/search_kernel/ffi/v1/response_envelope_generated.rs");
                }
                pub use response_envelope_generated::*;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Product,
    C5,
}

impl Boundary {
    fn name(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::C5 => "c5",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Insert,
    Update,
    Delete,
    Close,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Close => "close",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Before,
    After,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

type State = BTreeMap<String, i64>;
type Identity = Vec<(PathBuf, Vec<u8>)>;

fn product_definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("recovery")
        .field(Field::text("title").stored(true))
        .field(Field::int64("qty").stored(true).filterable(true))
}

fn product_document(id: &str, qty: i64) -> JsonValue {
    serde_json::json!({"title": format!("recovery document {id}"), "qty": qty})
}

fn seed_state() -> State {
    State::from([
        (BASE_1.to_owned(), 10),
        (BASE_2.to_owned(), 20),
        (BASE_3.to_owned(), 30),
    ])
}

fn identity(root: &Path) -> AnyResult<Identity> {
    let relative = [
        "manifest/index.json",
        "manifest/index-definition.bfbs",
        "manifest/portable-index-definition.bfbs",
        "config/searchkernel-schema.json",
        "config/indexschema.cfg",
    ];
    relative
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            Ok((
                path.clone(),
                fs::read(&path)
                    .map_err(|error| format!("read identity file {}: {error}", path.display()))?,
            ))
        })
        .collect()
}

fn assert_identity(snapshot: &Identity) -> AnyResult<()> {
    for (path, expected) in snapshot {
        let actual = fs::read(path)?;
        if &actual != expected {
            return Err(format!("durable identity changed: {}", path.display()).into());
        }
    }
    Ok(())
}

fn assert_root_shape(root: &Path) -> AnyResult<()> {
    for relative in [
        "manifest/index.json",
        "manifest/index-definition.bfbs",
        "data",
    ] {
        if !root.join(relative).exists() {
            return Err(format!("reopened index is missing {relative}").into());
        }
    }
    Ok(())
}

fn product_schema(runtime: &IndexRuntime) -> AnyResult<()> {
    let description = runtime.describe()?;
    if description.index_name() != Some("recovery") {
        return Err("product describe returned the wrong index name".into());
    }
    let fields: Vec<_> = description
        .fields()
        .iter()
        .map(|field| field.name())
        .collect();
    if fields != ["title", "qty"] {
        return Err(format!("product describe returned unexpected fields: {fields:?}").into());
    }
    Ok(())
}

fn product_state(runtime: &IndexRuntime) -> AnyResult<State> {
    product_schema(runtime)?;
    let index = runtime.index();
    let mut hits: Vec<String> = index
        .browse()
        .limit(100)
        .execute::<JsonValue>()?
        .hits
        .into_iter()
        .map(|hit| hit.id.to_string())
        .collect();
    hits.sort();
    let mut state = State::new();
    for id in hits {
        let document: JsonValue = index
            .get(&id)?
            .ok_or_else(|| format!("search returned a document that get cannot read: {id}"))?;
        let qty = document
            .get("qty")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("document {id} has no integer qty"))?;
        state.insert(id, qty);
    }
    Ok(state)
}

fn product_create(root: &Path) -> AnyResult<Identity> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent)?;
    }
    let runtime = IndexRuntime::create(root, product_definition())?;
    let index = runtime.index();
    for (id, qty) in seed_state() {
        index.insert(&id, &product_document(&id, qty))?;
    }
    product_schema(&runtime)?;
    runtime.close()?;
    assert_root_shape(root)?;
    identity(root)
}

fn product_open(root: &Path) -> AnyResult<IndexRuntime> {
    Ok(IndexRuntime::open(root)?)
}

fn product_insert(root: &Path, id: &str, qty: i64) -> AnyResult<()> {
    let runtime = product_open(root)?;
    runtime.index().insert(id, &product_document(id, qty))?;
    runtime.close()?;
    Ok(())
}

fn product_seed_delete_target(root: &Path, id: &str) -> AnyResult<()> {
    product_insert(root, id, 70)
}

fn product_crash(root: &Path, operation: Operation, phase: Phase, cycle: usize) -> AnyResult<()> {
    let runtime = product_open(root)?;
    if matches!(phase, Phase::Before) {
        abort_with_marker(root, operation, phase);
    }
    let index = runtime.index();
    match operation {
        Operation::Insert => {
            let id = insert_target(cycle);
            index.insert(&id, &product_document(&id, 40))?;
        }
        Operation::Update => {
            index.patch(BASE_2, searchkernel::Patch::new().set("qty", UPDATE_VALUE))?
        }
        Operation::Delete => {
            let id = if cycle == usize::MAX {
                BASE_3.to_owned()
            } else {
                format!("recovery-delete-{cycle}")
            };
            if !index.delete(id)? {
                return Err(
                    format!("product delete target was absent: recovery-delete-{cycle}").into(),
                );
            }
        }
        Operation::Close => runtime.close()?,
    }
    abort_with_marker(root, operation, phase);
}

fn abort_with_marker(root: &Path, operation: Operation, phase: Phase) -> ! {
    let marker = root.join(format!(
        ".crash-marker-{}-{}",
        operation.name(),
        phase.name()
    ));
    fs::write(marker, format!("{}:{}", operation.name(), phase.name()))
        .expect("write crash marker");
    std::process::abort()
}

fn product_verify(
    root: &Path,
    identity: &Identity,
    baseline: &State,
    operation: Operation,
    cycle: usize,
) -> AnyResult<State> {
    assert_root_shape(root)?;
    assert_identity(identity)?;
    let runtime = product_open(root)?;
    let state = product_state(&runtime)?;
    let expected = allowed_states(baseline, operation, cycle);
    if !expected.iter().any(|candidate| candidate == &state) {
        return Err(format!(
            "product recovered unexpected state for {}: {state:?}",
            operation.name()
        )
        .into());
    }
    runtime.close()?;
    Ok(state)
}

fn c5_definition() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let name = builder.create_string("recovery");
    let title_name = builder.create_string("title");
    let qty_name = builder.create_string("qty");
    let title_stored = api::OptionalBool::new(true);
    let title_searchable = api::OptionalBool::new(true);
    let qty_stored = api::OptionalBool::new(true);
    let qty_filterable = api::OptionalBool::new(true);
    let title = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(title_name),
            kind: api::FieldKind::Text,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&title_stored),
            searchable: Some(&title_searchable),
            ..Default::default()
        },
    );
    let qty = api::FieldDefinition::create(
        &mut builder,
        &api::FieldDefinitionArgs {
            name: Some(qty_name),
            kind: api::FieldKind::Int64,
            cardinality: api::FieldCardinality::Single,
            stored: Some(&qty_stored),
            filterable: Some(&qty_filterable),
            ..Default::default()
        },
    );
    let fields = builder.create_vector(&[title, qty]);
    let definition = api::IndexDefinition::create(
        &mut builder,
        &api::IndexDefinitionArgs {
            name: Some(name),
            fields: Some(fields),
            ..Default::default()
        },
    );
    builder.finish(definition, None);
    builder.finished_data().to_vec()
}

fn c5_value_string<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &str,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let text = builder.create_string(value);
    let scalar = api::StringValue::create(builder, &api::StringValueArgs { value: Some(text) });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::StringValue,
            kind: Some(scalar.as_union_value()),
        },
    )
}

fn c5_value_int<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: i64,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let scalar = api::Int64Value::create(builder, &api::Int64ValueArgs { value });
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type: api::ValueKind::Int64Value,
            kind: Some(scalar.as_union_value()),
        },
    )
}

fn c5_insert_inner(id: &str, qty: i64) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let document_id = id.to_owned();
    let id = builder.create_string(id);
    let title_key = builder.create_string("title");
    let qty_key = builder.create_string("qty");
    let title = c5_value_string(&mut builder, &format!("recovery document {document_id}"));
    let qty = c5_value_int(&mut builder, qty);
    let entries = [
        api::ValueEntry::create(
            &mut builder,
            &api::ValueEntryArgs {
                key: Some(title_key),
                value: Some(title),
            },
        ),
        api::ValueEntry::create(
            &mut builder,
            &api::ValueEntryArgs {
                key: Some(qty_key),
                value: Some(qty),
            },
        ),
    ];
    let fields = builder.create_vector(&entries);
    let document = api::Document::create(
        &mut builder,
        &api::DocumentArgs {
            id: Some(id),
            fields: Some(fields),
        },
    );
    let request = api::InsertRequest::create(
        &mut builder,
        &api::InsertRequestArgs {
            document: Some(document),
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_patch_inner(id: &str, qty: i64) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let field = builder.create_string("qty");
    let value = c5_value_int(&mut builder, qty);
    let set = api::SetField::create(
        &mut builder,
        &api::SetFieldArgs {
            field: Some(field),
            value: Some(value),
        },
    );
    let operation = api::PatchOperation::create(
        &mut builder,
        &api::PatchOperationArgs {
            operation_type: api::PatchOperationKind::SetField,
            operation: Some(set.as_union_value()),
        },
    );
    let operations = builder.create_vector(&[operation]);
    let request = api::PatchRequest::create(
        &mut builder,
        &api::PatchRequestArgs {
            id: Some(id),
            operations: Some(operations),
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_delete_inner(id: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let request =
        api::DeleteRequest::create(&mut builder, &api::DeleteRequestArgs { id: Some(id) });
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_get_inner(id: &str) -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let id = builder.create_string(id);
    let request = api::GetRequest::create(
        &mut builder,
        &api::GetRequestArgs {
            id: Some(id),
            projection: None,
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_search_inner() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let query = builder.create_string("");
    let request = api::SearchRequest::create(
        &mut builder,
        &api::SearchRequestArgs {
            query: Some(query),
            limit: Some(&api::OptionalUInt32::new(100)),
            ..Default::default()
        },
    );
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_describe_inner() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let request = api::DescribeRequest::create(&mut builder, &api::DescribeRequestArgs::default());
    builder.finish(request, None);
    builder.finished_data().to_vec()
}

fn c5_envelope(kind: u8, inner: &[u8]) -> Vec<u8> {
    let root_offset = u32::from_le_bytes(inner[..4].try_into().expect("inner root")) as usize;
    let mut copied = Vec::with_capacity(inner.len() + 4);
    copied.resize(4, 0);
    copied.extend_from_slice(inner);
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let inner_vector = builder.create_byte_string(&copied);
    let payload = flatbuffers::WIPOffset::<flatbuffers::UnionWIPOffset>::new(
        inner_vector.value() - 8 - root_offset as u32,
    );
    let envelope = ffi_wire::search_kernel::ffi::v1::RequestEnvelope::create(
        &mut builder,
        &ffi_wire::search_kernel::ffi::v1::RequestEnvelopeArgs {
            protocol_version: 1,
            payload_type: ffi_wire::search_kernel::ffi::v1::RequestPayload(kind),
            payload: Some(payload),
        },
    );
    builder.finish(
        envelope,
        Some(ffi_wire::search_kernel::ffi::v1::REQUEST_ENVELOPE_IDENTIFIER),
    );
    builder.finished_data().to_vec()
}

fn c5_call(handle: searchkernel_ffi::SkHandle, request: &[u8]) -> AnyResult<Vec<u8>> {
    let mut data = std::ptr::null_mut();
    let mut length = 0;
    let mut allocation_id = 0;
    // SAFETY: request and output pointers remain valid for the duration of the ABI call.
    let status = unsafe {
        searchkernel_ffi::sk_call(
            handle,
            request.as_ptr(),
            request.len(),
            &mut data,
            &mut length,
            &mut allocation_id,
        )
    };
    if status != searchkernel_ffi::SK_STATUS_OK {
        return Err(format!("C5 call returned status {status}").into());
    }
    if data.is_null() || length == 0 || allocation_id == 0 {
        return Err("C5 call returned an invalid output triple".into());
    }
    // SAFETY: a successful call owns exactly `length` bytes at `data`.
    let response = unsafe { std::slice::from_raw_parts(data, length).to_vec() };
    // SAFETY: release the exact pointer/token pair returned by the ABI.
    unsafe { searchkernel_ffi::sk_buffer_free(data, allocation_id) };
    Ok(response)
}

fn c5_payload(handle: searchkernel_ffi::SkHandle, kind: u8, inner: Vec<u8>) -> AnyResult<Vec<u8>> {
    c5_call(handle, &c5_envelope(kind, &inner))
}

fn c5_create(root: &Path) -> AnyResult<Identity> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent)?;
    }
    let path = CString::new(root.to_string_lossy().as_bytes())?;
    let definition = c5_definition();
    let mut handle = 0;
    let mut error = vec![0_u8; 1024];
    // SAFETY: all pointers reference live, correctly sized storage.
    let status = unsafe {
        searchkernel_ffi::sk_create(
            path.as_ptr(),
            definition.as_ptr(),
            definition.len(),
            &mut handle,
            error.as_mut_ptr(),
            error.len(),
        )
    };
    if status != searchkernel_ffi::SK_STATUS_OK || handle == 0 {
        return Err(format!("C5 create returned status {status}").into());
    }
    let insert_kind =
        ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_InsertRequest.0;
    for (id, qty) in seed_state() {
        c5_payload(handle, insert_kind, c5_insert_inner(&id, qty))?;
    }
    c5_close(handle)?;
    assert_root_shape(root)?;
    identity(root)
}

fn c5_open(root: &Path) -> AnyResult<searchkernel_ffi::SkHandle> {
    let path = CString::new(root.to_string_lossy().as_bytes())?;
    let mut handle = 0;
    let mut error = vec![0_u8; 1024];
    // SAFETY: path is NUL-terminated and output buffers are writable.
    let status = unsafe {
        searchkernel_ffi::sk_open(path.as_ptr(), &mut handle, error.as_mut_ptr(), error.len())
    };
    if status != searchkernel_ffi::SK_STATUS_OK || handle == 0 {
        return Err(format!("C5 open returned status {status}").into());
    }
    Ok(handle)
}

fn c5_close(handle: searchkernel_ffi::SkHandle) -> AnyResult<()> {
    let mut error = vec![0_u8; 1024];
    let status = searchkernel_ffi::sk_close(handle, error.as_mut_ptr(), error.len());
    if status != searchkernel_ffi::SK_STATUS_OK {
        return Err(format!("C5 close returned status {status}").into());
    }
    Ok(())
}

fn c5_state(handle: searchkernel_ffi::SkHandle) -> AnyResult<State> {
    let search_kind =
        ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_SearchRequest.0;
    let response = c5_payload(handle, search_kind, c5_search_inner())?;
    let envelope = root::<ffi_wire::search_kernel::ffi::v1::ResponseEnvelope>(&response)?;
    let result = envelope
        .payload_as_search_kernel_v1_search_result()
        .ok_or("C5 search response has the wrong payload")?;
    let total = result
        .total_hits()
        .ok_or("C5 search response has no total")?
        .value();
    let mut state = State::new();
    if let Some(hits) = result.hits() {
        for hit in hits.iter() {
            let id = hit.id().ok_or("C5 search hit has no ID")?.to_owned();
            state.insert(id, 0);
        }
    }
    if total != state.len() as u64 {
        return Err(format!("C5 search total {total} differs from hits {}", state.len()).into());
    }
    let get_kind = ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_GetRequest.0;
    for id in state.clone().keys() {
        let response = c5_payload(handle, get_kind, c5_get_inner(id))?;
        let envelope = root::<ffi_wire::search_kernel::ffi::v1::ResponseEnvelope>(&response)?;
        let result = envelope
            .payload_as_search_kernel_v1_get_result()
            .ok_or("C5 get response has the wrong payload")?;
        let document = result.document().ok_or("C5 get lost a searched document")?;
        let qty = document
            .fields()
            .ok_or("C5 document has no fields")?
            .iter()
            .find(|entry| entry.key() == "qty")
            .and_then(|entry| entry.value())
            .and_then(|value| value.kind_as_int_64_value())
            .ok_or("C5 document has no integer qty")?
            .value();
        state.insert(id.to_owned(), qty);
    }
    Ok(state)
}

fn c5_schema(handle: searchkernel_ffi::SkHandle) -> AnyResult<()> {
    let kind = ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_DescribeRequest.0;
    let response = c5_payload(handle, kind, c5_describe_inner())?;
    let envelope = root::<ffi_wire::search_kernel::ffi::v1::ResponseEnvelope>(&response)?;
    let definition = envelope
        .payload_as_search_kernel_v1_describe_result()
        .and_then(|result| result.index())
        .ok_or("C5 describe response has no index definition")?;
    if definition.name() != Some("recovery") {
        return Err("C5 describe returned the wrong index name".into());
    }
    let fields: Vec<_> = definition
        .fields()
        .ok_or("C5 describe response has no fields")?
        .iter()
        .map(|field| (field.name().unwrap_or(""), field.kind()))
        .collect();
    if fields.len() != 2
        || fields[0].0 != "title"
        || fields[0].1 != api::FieldKind::Text
        || fields[1].0 != "qty"
        || fields[1].1 != api::FieldKind::Int64
    {
        return Err(format!("C5 describe returned unexpected fields: {fields:?}").into());
    }
    Ok(())
}

fn c5_insert(root: &Path, id: &str, qty: i64) -> AnyResult<()> {
    let handle = c5_open(root)?;
    let kind = ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_InsertRequest.0;
    c5_payload(handle, kind, c5_insert_inner(id, qty))?;
    c5_close(handle)
}

fn c5_seed_delete_target(root: &Path, id: &str) -> AnyResult<()> {
    c5_insert(root, id, 70)
}

fn c5_crash(root: &Path, operation: Operation, phase: Phase, cycle: usize) -> AnyResult<()> {
    let handle = c5_open(root)?;
    if matches!(phase, Phase::Before) {
        abort_with_marker(root, operation, phase);
    }
    let (kind, request) = match operation {
        Operation::Insert => (
            ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_InsertRequest.0,
            {
                let id = insert_target(cycle);
                c5_insert_inner(&id, 40)
            },
        ),
        Operation::Update => (
            ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_PatchRequest.0,
            c5_patch_inner(BASE_2, UPDATE_VALUE),
        ),
        Operation::Delete => {
            let id = if cycle == usize::MAX {
                BASE_3.to_owned()
            } else {
                format!("recovery-delete-{cycle}")
            };
            (
                ffi_wire::search_kernel::ffi::v1::RequestPayload::SearchKernel_V1_DeleteRequest.0,
                c5_delete_inner(&id),
            )
        }
        Operation::Close => {
            c5_close(handle)?;
            abort_with_marker(root, operation, phase);
        }
    };
    c5_payload(handle, kind, request)?;
    abort_with_marker(root, operation, phase)
}

fn c5_verify(
    root: &Path,
    identity: &Identity,
    baseline: &State,
    operation: Operation,
    cycle: usize,
) -> AnyResult<State> {
    assert_root_shape(root)?;
    assert_identity(identity)?;
    let handle = c5_open(root)?;
    c5_schema(handle)?;
    let state = c5_state(handle)?;
    let expected = allowed_states(baseline, operation, cycle);
    if !expected.iter().any(|candidate| candidate == &state) {
        return Err(format!(
            "C5 recovered unexpected state for {}: {state:?}",
            operation.name()
        )
        .into());
    }
    c5_close(handle)?;
    Ok(state)
}

fn allowed_states(baseline: &State, operation: Operation, cycle: usize) -> Vec<State> {
    let mut states = vec![baseline.clone()];
    match operation {
        Operation::Insert => {
            let id = insert_target(cycle);
            let mut post = baseline.clone();
            post.insert(id, 40);
            states.push(post);
        }
        Operation::Update => {
            let mut post = baseline.clone();
            post.insert(BASE_2.to_owned(), UPDATE_VALUE);
            states.push(post);
        }
        Operation::Delete => {
            let id = if cycle == usize::MAX {
                BASE_3.to_owned()
            } else {
                format!("recovery-delete-{cycle}")
            };
            let mut post = baseline.clone();
            post.remove(&id);
            states.push(post);
        }
        Operation::Close => {}
    }
    states
}

fn marker_path(root: &Path, operation: Operation, phase: Phase) -> PathBuf {
    root.join(format!(
        ".crash-marker-{}-{}",
        operation.name(),
        phase.name()
    ))
}

fn run_child(
    boundary: Boundary,
    operation: Operation,
    phase: Phase,
    root: &Path,
    cycle: usize,
) -> AnyResult<()> {
    match boundary {
        Boundary::Product => product_crash(root, operation, phase, cycle),
        Boundary::C5 => c5_crash(root, operation, phase, cycle),
    }
}

fn spawn_crash(
    boundary: Boundary,
    operation: Operation,
    phase: Phase,
    root: &Path,
    cycle: usize,
) -> AnyResult<()> {
    let executable = env::current_exe()?;
    let status = Command::new(executable)
        .arg("child")
        .arg(boundary.name())
        .arg(operation.name())
        .arg(phase.name())
        .arg(root)
        .arg(cycle.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.signal() != Some(6) {
        return Err(format!(
            "{} {} {} child did not abort with SIGABRT: {status:?}",
            boundary.name(),
            operation.name(),
            phase.name()
        )
        .into());
    }
    let marker = marker_path(root, operation, phase);
    let actual = fs::read_to_string(&marker)?;
    let expected = format!("{}:{}", operation.name(), phase.name());
    if actual != expected {
        return Err(format!("crash marker was {actual:?}, expected {expected:?}").into());
    }
    fs::remove_file(marker)?;
    Ok(())
}

fn setup(boundary: Boundary, root: &Path) -> AnyResult<Identity> {
    match boundary {
        Boundary::Product => product_create(root),
        Boundary::C5 => c5_create(root),
    }
}

fn verify(
    boundary: Boundary,
    root: &Path,
    identity: &Identity,
    baseline: &State,
    operation: Operation,
    cycle: usize,
) -> AnyResult<State> {
    match boundary {
        Boundary::Product => product_verify(root, identity, baseline, operation, cycle),
        Boundary::C5 => c5_verify(root, identity, baseline, operation, cycle),
    }
}

fn prepare_delete_target(boundary: Boundary, root: &Path, id: &str) -> AnyResult<()> {
    match boundary {
        Boundary::Product => product_seed_delete_target(root, id),
        Boundary::C5 => c5_seed_delete_target(root, id),
    }
}

fn assert_missing_open(boundary: Boundary, root: &Path) -> AnyResult<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    match boundary {
        Boundary::Product => {
            if IndexRuntime::open(root).is_ok() {
                return Err("product open unexpectedly created a missing index".into());
            }
        }
        Boundary::C5 => {
            let path = CString::new(root.to_string_lossy().as_bytes())?;
            let mut handle = 0;
            let mut error = vec![0_u8; 1024];
            // SAFETY: path and output buffers are valid for this call.
            let status = unsafe {
                searchkernel_ffi::sk_open(
                    path.as_ptr(),
                    &mut handle,
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
            if status == searchkernel_ffi::SK_STATUS_OK || handle != 0 {
                return Err("C5 open unexpectedly created a missing index".into());
            }
        }
    }
    if root.exists() {
        return Err("missing-index open created a directory".into());
    }
    Ok(())
}

fn run_matrix(root: &Path) -> AnyResult<()> {
    let operations = [
        Operation::Insert,
        Operation::Update,
        Operation::Delete,
        Operation::Close,
    ];
    let phases = [Phase::Before, Phase::After];
    for boundary in [Boundary::Product, Boundary::C5] {
        for repetition in 0..MATRIX_REPETITIONS {
            for operation in operations {
                for phase in phases {
                    let case_root = root.join(format!(
                        "matrix-{}-{repetition}-{}-{}",
                        boundary.name(),
                        operation.name(),
                        phase.name()
                    ));
                    let identity = setup(boundary, &case_root)?;
                    let baseline = seed_state();
                    spawn_crash(boundary, operation, phase, &case_root, usize::MAX)?;
                    verify(
                        boundary,
                        &case_root,
                        &identity,
                        &baseline,
                        operation,
                        usize::MAX,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn run_repeated(root: &Path) -> AnyResult<()> {
    for boundary in [Boundary::Product, Boundary::C5] {
        let case_root = root.join(format!("repeated-{}", boundary.name()));
        let identity = setup(boundary, &case_root)?;
        let mut model = seed_state();
        for cycle in 0..REPEATED_CYCLES {
            let operation = match cycle % 4 {
                0 => Operation::Insert,
                1 => Operation::Update,
                2 => Operation::Delete,
                _ => Operation::Close,
            };
            let phase = if (cycle / 4) % 2 == 0 {
                Phase::Before
            } else {
                Phase::After
            };
            if matches!(operation, Operation::Delete) {
                let id = format!("recovery-delete-{cycle}");
                prepare_delete_target(boundary, &case_root, &id)?;
                model.insert(id, 70);
            }
            spawn_crash(boundary, operation, phase, &case_root, cycle)?;
            model = verify(boundary, &case_root, &identity, &model, operation, cycle)?;
        }
    }
    Ok(())
}

fn parse_boundary(value: &str) -> AnyResult<Boundary> {
    match value {
        "product" => Ok(Boundary::Product),
        "c5" => Ok(Boundary::C5),
        _ => Err(format!("unknown boundary {value}").into()),
    }
}

fn parse_operation(value: &str) -> AnyResult<Operation> {
    match value {
        "insert" => Ok(Operation::Insert),
        "update" => Ok(Operation::Update),
        "delete" => Ok(Operation::Delete),
        "close" => Ok(Operation::Close),
        _ => Err(format!("unknown operation {value}").into()),
    }
}

fn parse_phase(value: &str) -> AnyResult<Phase> {
    match value {
        "before" => Ok(Phase::Before),
        "after" => Ok(Phase::After),
        _ => Err(format!("unknown phase {value}").into()),
    }
}

fn main() -> AnyResult<()> {
    let mut args = env::args_os();
    let _program = args.next();
    let mode = args.next().ok_or("missing mode")?;
    match mode.to_string_lossy().as_ref() {
        "child" => {
            let boundary =
                parse_boundary(&args.next().ok_or("missing boundary")?.to_string_lossy())?;
            let operation =
                parse_operation(&args.next().ok_or("missing operation")?.to_string_lossy())?;
            let phase = parse_phase(&args.next().ok_or("missing phase")?.to_string_lossy())?;
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            let cycle = args
                .next()
                .ok_or("missing cycle")?
                .to_string_lossy()
                .parse::<usize>()?;
            run_child(boundary, operation, phase, &root, cycle)
        }
        "matrix" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            run_matrix(&root)?;
            println!("recovery matrix boundaries=2 cases=8 repetitions={MATRIX_REPETITIONS}");
            Ok(())
        }
        "repeat" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            run_repeated(&root)?;
            println!("recovery repeated boundaries=2 cycles={REPEATED_CYCLES}");
            Ok(())
        }
        "missing" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            for boundary in [Boundary::Product, Boundary::C5] {
                assert_missing_open(boundary, &root.join(boundary.name()))?;
            }
            println!("recovery missing-open boundaries=2 created=false");
            Ok(())
        }
        other => Err(format!("unknown mode {other}").into()),
    }
}
