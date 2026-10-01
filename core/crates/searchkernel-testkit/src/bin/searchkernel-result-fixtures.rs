//! Generate and check the canonical encoded result corpus.

use flatbuffers::FlatBufferBuilder;
use searchkernel_protocol::protocol::v1 as wire;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[path = "../malformed_tensor_fixtures.rs"]
mod malformed_tensor_fixtures;

const DEFAULT_OUTPUT: &str = "core/crates/searchkernel-dispatch/tests/fixtures/portable/responses";
const MALFORMED_OUTPUT: &str = "reference/result-response-conformance/fixtures";
const RESPONSE_IDENTIFIER: &str = "SKRS";

fn encode_fixture(result: &searchkernel_ir::SearchResult) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let payload = searchkernel_ir::wire::v1::search_result_offset(&mut builder, result)
        .expect("canonical result fixture must validate")
        .as_union_value();
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: searchkernel_protocol::protocol::v1::PROTOCOL_VERSION,
            payload_type: wire::ResponsePayload::SearchKernel_Wire_SearchResult,
            payload: Some(payload),
        },
    );
    builder.finish(envelope, Some(RESPONSE_IDENTIFIER));
    builder.finished_data().to_vec()
}

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let check = arguments
        .next()
        .is_some_and(|argument| argument == "--check");
    let output = arguments
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    assert!(
        arguments.next().is_none(),
        "usage: searchkernel-result-fixtures [--check] [output-directory]"
    );

    if !check {
        std::fs::create_dir_all(&output).expect("create fixture output directory");
    }

    let fixtures = searchkernel_testkit::result_fixtures();
    let malformed_fixtures = malformed_tensor_fixtures::malformed_tensor_response_fixtures();
    let expected = fixtures
        .iter()
        .map(|fixture| format!("result-{}.skrs", fixture.name))
        .collect::<BTreeSet<_>>();
    for fixture in fixtures {
        let path = output.join(format!("result-{}.skrs", fixture.name));
        let bytes = encode_fixture(&fixture.result);
        if check {
            let existing = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(existing, bytes, "fixture drifted: {}", path.display());
        } else {
            write_fixture(&path, &bytes);
        }
    }
    let malformed_output = Path::new(MALFORMED_OUTPUT);
    if !check {
        std::fs::create_dir_all(malformed_output).expect("create malformed fixture directory");
    }
    let canonical = encode_fixture(
        &searchkernel_testkit::result_fixtures()
            .into_iter()
            .find(|fixture| fixture.name == "canonical")
            .expect("canonical fixture exists")
            .result,
    );
    let values = encode_fixture(
        &searchkernel_testkit::result_fixtures()
            .into_iter()
            .find(|fixture| fixture.name == "value-variants")
            .expect("value-variants fixture exists")
            .result,
    );
    let tensors = encode_fixture(
        &searchkernel_testkit::result_fixtures()
            .into_iter()
            .find(|fixture| fixture.name == "sparse-mixed-tensors")
            .expect("sparse-mixed-tensors fixture exists")
            .result,
    );
    let malformed = [
        (
            "unknown-result-node-union",
            unknown_result_node_union(canonical.clone()),
        ),
        ("unknown-value-union", unknown_value_union(values.clone())),
        (
            "unknown-relevance-union",
            unknown_relevance_union(canonical.clone()),
        ),
        (
            "unknown-tensor-payload-union",
            unknown_tensor_payload_union(tensors.clone()),
        ),
        (
            "missing-search-result-root",
            missing_search_result_root(canonical.clone()),
        ),
        (
            "missing-search-result-total-hits",
            missing_search_result_total_hits(canonical.clone()),
        ),
        (
            "missing-search-result-root-and-total-hits",
            missing_search_result_root_and_total_hits(canonical.clone()),
        ),
        (
            "missing-continuation-key",
            missing_continuation_key(canonical.clone()),
        ),
        (
            "missing-continuation-value",
            missing_continuation_value(canonical.clone()),
        ),
        (
            "missing-sparse-address-dimension",
            missing_sparse_address_dimension(tensors.clone()),
        ),
        (
            "missing-sparse-address-label",
            missing_sparse_address_label(tensors.clone()),
        ),
        (
            "missing-mixed-address-dimension",
            missing_mixed_address_dimension(tensors.clone()),
        ),
        (
            "missing-mixed-address-label",
            missing_mixed_address_label(tensors.clone()),
        ),
        (
            "missing-value-entry-value",
            missing_value_entry_value(values.clone()),
        ),
        (
            "non-finite-dense-tensor-cell",
            non_finite_dense_tensor_cell(tensors.clone()),
        ),
        (
            "non-finite-sparse-tensor-cell",
            non_finite_sparse_tensor_cell(tensors.clone()),
        ),
        (
            "non-finite-mixed-tensor-cell",
            non_finite_mixed_tensor_cell(tensors),
        ),
        (
            "unknown-response-payload-union",
            unknown_response_payload_union(canonical.clone()),
        ),
        ("duplicate-result-map-key", duplicate_result_map_key(values)),
    ];
    let mut expected_malformed = malformed
        .iter()
        .map(|(name, _)| format!("result-malformed-{name}.skrs"))
        .collect::<BTreeSet<_>>();
    for (name, bytes) in malformed {
        let path = malformed_output.join(format!("result-malformed-{name}.skrs"));
        if check {
            let existing = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(
                existing,
                bytes,
                "malformed fixture drifted: {}",
                path.display()
            );
        } else {
            write_fixture(&path, &bytes);
        }
    }
    for (name, bytes) in malformed_fixtures {
        expected_malformed.insert(format!("{name}.skrs"));
        let path = malformed_output.join(format!("{name}.skrs"));
        if check {
            let existing = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert_eq!(
                existing,
                bytes,
                "malformed fixture drifted: {}",
                path.display()
            );
        } else {
            write_fixture(&path, &bytes);
        }
    }
    if check {
        for entry in std::fs::read_dir(&output).expect("read fixture output directory") {
            let name = entry
                .expect("read fixture directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.starts_with("result-") && name.ends_with(".skrs") {
                assert!(
                    expected.contains(&name),
                    "unexpected result fixture: {name}"
                );
            }
        }
        for entry in std::fs::read_dir(malformed_output).expect("read malformed fixture directory")
        {
            let name = entry
                .expect("read malformed fixture directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            if name.ends_with(".skrs") {
                assert!(
                    expected_malformed.contains(&name),
                    "unexpected malformed fixture: {name}"
                );
            }
        }
    }
}

fn table_field(bytes: &[u8], table: usize, slot: usize) -> usize {
    let vtable = (table as isize - read_i32(bytes, table) as isize) as usize;
    let field_offset = read_u16(bytes, vtable + 4 + slot * 2) as usize;
    assert_ne!(field_offset, 0, "required fixture field is absent");
    table + field_offset
}

fn indirect(bytes: &[u8], field: usize) -> usize {
    field + read_i32(bytes, field) as usize
}

fn vector_table(bytes: &[u8], field: usize, index: usize) -> usize {
    let vector = indirect(bytes, field);
    assert!(index < read_u32(bytes, vector) as usize);
    let element = vector + 4 + index * 4;
    indirect(bytes, element)
}

fn response_payload(bytes: &[u8]) -> usize {
    let root = read_u32(bytes, 0) as usize;
    indirect(bytes, table_field(bytes, root, 2))
}

fn result_root(bytes: &[u8]) -> usize {
    let payload = response_payload(bytes);
    let field = table_field(bytes, payload, 0);
    indirect(bytes, field)
}

fn unknown_result_node_union(mut bytes: Vec<u8>) -> Vec<u8> {
    let node = result_root(&bytes);
    let kind = table_field(&bytes, node, 0);
    bytes[kind] = u8::MAX;
    bytes
}

fn unknown_response_payload_union(mut bytes: Vec<u8>) -> Vec<u8> {
    let root = read_u32(&bytes, 0) as usize;
    let kind = table_field(&bytes, root, 1);
    bytes[kind] = u8::MAX;
    bytes
}

fn unknown_relevance_union(mut bytes: Vec<u8>) -> Vec<u8> {
    let node = result_root(&bytes);
    let group = indirect(&bytes, table_field(&bytes, node, 1));
    let child = vector_table(&bytes, table_field(&bytes, group, 4), 0);
    let hit = indirect(&bytes, table_field(&bytes, child, 1));
    let relevance = indirect(&bytes, table_field(&bytes, hit, 1));
    let kind = table_field(&bytes, relevance, 0);
    bytes[kind] = u8::MAX;
    bytes
}

fn unknown_tensor_payload_union(mut bytes: Vec<u8>) -> Vec<u8> {
    let node = result_root(&bytes);
    let group = indirect(&bytes, table_field(&bytes, node, 1));
    let value = indirect(&bytes, table_field(&bytes, group, 2));
    let tensor = indirect(&bytes, table_field(&bytes, value, 1));
    let kind = table_field(&bytes, tensor, 1);
    bytes[kind] = u8::MAX;
    bytes
}

fn first_result_field_value(bytes: &[u8]) -> usize {
    let node = result_root(bytes);
    let group = indirect(bytes, table_field(bytes, node, 1));
    let fields = table_field(bytes, group, 3);
    let entry = vector_table(bytes, fields, 0);
    indirect(bytes, table_field(bytes, entry, 1))
}

fn unknown_value_union(mut bytes: Vec<u8>) -> Vec<u8> {
    let value = first_result_field_value(&bytes);
    let kind = table_field(&bytes, value, 0);
    bytes[kind] = u8::MAX;
    bytes
}

fn duplicate_result_map_key(mut bytes: Vec<u8>) -> Vec<u8> {
    let node = result_root(&bytes);
    let group = indirect(&bytes, table_field(&bytes, node, 1));
    let fields = table_field(&bytes, group, 3);
    // `int64_max` and `int64_min` have equal-length keys, so replacing the
    // latter's bytes preserves FlatBuffer bounds even as other fixture keys
    // are added or reordered.
    let vector = indirect(&bytes, fields);
    let count = read_u32(&bytes, vector) as usize;
    let entry = (0..count)
        .map(|index| vector_table(&bytes, fields, index))
        .find(|entry| string_value(&bytes, table_field(&bytes, *entry, 0)) == b"int64_min")
        .expect("value-variants map contains int64_min");
    let key_field = table_field(&bytes, entry, 0);
    let string = indirect(&bytes, key_field);
    let length = read_u32(&bytes, string) as usize;
    assert_eq!(length, b"int64_min".len());
    bytes[string + 4..string + 4 + length].copy_from_slice(b"int64_max");
    bytes
}

fn clear_table_field(bytes: &mut Vec<u8>, table: usize, slot: usize) {
    let vtable = (table as isize - read_i32(bytes, table) as isize) as usize;
    let vtable_size = read_u16(bytes, vtable) as usize;
    let field_offset = 4 + slot * 2;
    assert!(
        field_offset + 2 <= vtable_size,
        "fixture field slot is outside its vtable"
    );

    // FlatBuffers tables can share vtables. Copy before changing the selected
    // slot so one malformed fixture does not clear the same field on every
    // table with an identical layout.
    if !bytes.len().is_multiple_of(2) {
        bytes.push(0);
    }
    let copied_vtable = bytes[vtable..vtable + vtable_size].to_vec();
    let copied_vtable_position = bytes.len();
    bytes.extend_from_slice(&copied_vtable);
    let vtable_distance = table as isize - copied_vtable_position as isize;
    bytes[table..table + 4].copy_from_slice(&(vtable_distance as i32).to_le_bytes());

    let offset = copied_vtable_position + field_offset;
    assert_ne!(
        read_u16(bytes, offset),
        0,
        "fixture field is already absent"
    );
    bytes[offset..offset + 2].copy_from_slice(&0u16.to_le_bytes());
}

fn search_result_table(bytes: &[u8]) -> usize {
    response_payload(bytes)
}

fn root_group(bytes: &[u8]) -> usize {
    let node = result_root(bytes);
    indirect(bytes, table_field(bytes, node, 1))
}

fn missing_search_result_root(mut bytes: Vec<u8>) -> Vec<u8> {
    let table = search_result_table(&bytes);
    clear_table_field(&mut bytes, table, 0);
    bytes
}

fn missing_search_result_total_hits(mut bytes: Vec<u8>) -> Vec<u8> {
    let table = search_result_table(&bytes);
    clear_table_field(&mut bytes, table, 1);
    bytes
}

fn missing_search_result_root_and_total_hits(mut bytes: Vec<u8>) -> Vec<u8> {
    let table = search_result_table(&bytes);
    clear_table_field(&mut bytes, table, 0);
    clear_table_field(&mut bytes, table, 1);
    bytes
}

fn first_continuation(bytes: &[u8]) -> usize {
    let group = root_group(bytes);
    vector_table(bytes, table_field(bytes, group, 5), 0)
}

fn missing_continuation_key(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_continuation(&bytes);
    clear_table_field(&mut bytes, entry, 0);
    bytes
}

fn missing_continuation_value(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_continuation(&bytes);
    clear_table_field(&mut bytes, entry, 1);
    bytes
}

fn tensor_from_value(bytes: &[u8], value: usize) -> usize {
    indirect(bytes, table_field(bytes, value, 1))
}

fn mixed_tensor(bytes: &[u8]) -> usize {
    let group = root_group(bytes);
    let value = indirect(bytes, table_field(bytes, group, 2));
    tensor_from_value(bytes, value)
}

fn sparse_tensor(bytes: &[u8]) -> usize {
    let group = root_group(bytes);
    let child = vector_table(bytes, table_field(bytes, group, 4), 0);
    let hit = indirect(bytes, table_field(bytes, child, 1));
    let entry = vector_table(bytes, table_field(bytes, hit, 2), 0);
    let value = indirect(bytes, table_field(bytes, entry, 1));
    tensor_from_value(bytes, value)
}

fn dense_tensor(bytes: &[u8]) -> usize {
    let group = root_group(bytes);
    let entry = vector_table(bytes, table_field(bytes, group, 3), 0);
    let value = indirect(bytes, table_field(bytes, entry, 1));
    tensor_from_value(bytes, value)
}

fn first_sparse_address(bytes: &[u8]) -> usize {
    let tensor = sparse_tensor(bytes);
    let payload = indirect(bytes, table_field(bytes, tensor, 2));
    let cell = vector_table(bytes, table_field(bytes, payload, 0), 0);
    vector_table(bytes, table_field(bytes, cell, 0), 0)
}

fn first_mixed_address(bytes: &[u8]) -> usize {
    let tensor = mixed_tensor(bytes);
    let payload = indirect(bytes, table_field(bytes, tensor, 2));
    let block = vector_table(bytes, table_field(bytes, payload, 0), 0);
    vector_table(bytes, table_field(bytes, block, 0), 0)
}

fn missing_sparse_address_dimension(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_sparse_address(&bytes);
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 2);
    clear_table_field(&mut bytes, group, 3);
    clear_table_field(&mut bytes, entry, 0);
    bytes
}

fn missing_sparse_address_label(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_sparse_address(&bytes);
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 2);
    clear_table_field(&mut bytes, group, 3);
    clear_table_field(&mut bytes, entry, 1);
    bytes
}

fn missing_mixed_address_dimension(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_mixed_address(&bytes);
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 3);
    clear_table_field(&mut bytes, group, 4);
    clear_table_field(&mut bytes, entry, 0);
    bytes
}

fn missing_mixed_address_label(mut bytes: Vec<u8>) -> Vec<u8> {
    let entry = first_mixed_address(&bytes);
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 3);
    clear_table_field(&mut bytes, group, 4);
    clear_table_field(&mut bytes, entry, 1);
    bytes
}

fn missing_value_entry_value(mut bytes: Vec<u8>) -> Vec<u8> {
    let group = root_group(&bytes);
    let entry = vector_table(&bytes, table_field(&bytes, group, 3), 0);
    clear_table_field(&mut bytes, entry, 1);
    bytes
}

fn write_f64(bytes: &mut [u8], offset: usize, value: f64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn non_finite_dense_tensor_cell(mut bytes: Vec<u8>) -> Vec<u8> {
    let tensor = dense_tensor(&bytes);
    let payload = indirect(&bytes, table_field(&bytes, tensor, 2));
    let vector = indirect(&bytes, table_field(&bytes, payload, 0));
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 2);
    clear_table_field(&mut bytes, group, 4);
    write_f64(&mut bytes, vector + 4, f64::NAN);
    bytes
}

fn non_finite_sparse_tensor_cell(mut bytes: Vec<u8>) -> Vec<u8> {
    let tensor = sparse_tensor(&bytes);
    let payload = indirect(&bytes, table_field(&bytes, tensor, 2));
    let cell = vector_table(&bytes, table_field(&bytes, payload, 0), 0);
    let value = table_field(&bytes, cell, 1);
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 2);
    clear_table_field(&mut bytes, group, 3);
    write_f64(&mut bytes, value, f64::INFINITY);
    bytes
}

fn non_finite_mixed_tensor_cell(mut bytes: Vec<u8>) -> Vec<u8> {
    let tensor = mixed_tensor(&bytes);
    let payload = indirect(&bytes, table_field(&bytes, tensor, 2));
    let block = vector_table(&bytes, table_field(&bytes, payload, 0), 0);
    let vector = indirect(&bytes, table_field(&bytes, block, 1));
    let group = root_group(&bytes);
    clear_table_field(&mut bytes, group, 3);
    clear_table_field(&mut bytes, group, 4);
    write_f64(&mut bytes, vector + 4, f64::NEG_INFINITY);
    bytes
}

fn string_value(bytes: &[u8], field: usize) -> &[u8] {
    let string = indirect(bytes, field);
    let length = read_u32(bytes, string) as usize;
    &bytes[string + 4..string + 4 + length]
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn write_fixture(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}
