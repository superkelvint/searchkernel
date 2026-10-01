//! Deliberately malformed wire responses for decoder conformance tests.
//!
//! These fixtures bypass Search Kernel IR because IR correctly refuses to
//! encode non-finite tensor cells. They exercise the decoder's fail-closed
//! behavior for adversarial FlatBuffers responses.

use flatbuffers::FlatBufferBuilder;
use searchkernel_protocol::protocol::v1::{self as wire, PROTOCOL_VERSION};

fn address<'a>(
    builder: &mut FlatBufferBuilder<'a>,
) -> flatbuffers::WIPOffset<wire::TensorAddressEntry<'a>> {
    let dimension = builder.create_string("category");
    let label = builder.create_string("books");
    wire::TensorAddressEntry::create(
        builder,
        &wire::TensorAddressEntryArgs {
            dimension: Some(dimension),
            label: Some(label),
        },
    )
}

fn malformed_tensor_payload(
    builder: &mut FlatBufferBuilder<'_>,
    name: &'static str,
) -> (
    wire::TensorPayloadKind,
    flatbuffers::WIPOffset<flatbuffers::UnionWIPOffset>,
    &'static str,
) {
    match name {
        "dense-nan" => {
            let values = builder.create_vector(&[f64::NAN]);
            let payload = wire::DenseTensorValues::create(
                builder,
                &wire::DenseTensorValuesArgs {
                    values: Some(values),
                },
            );
            (
                wire::TensorPayloadKind::DenseTensorValues,
                payload.as_union_value(),
                "tensor<float>(x[1])",
            )
        }
        "sparse-positive-infinity" => {
            let cell_address = address(builder);
            let cell_address = builder.create_vector(&[cell_address]);
            let cell = wire::TensorCell::create(
                builder,
                &wire::TensorCellArgs {
                    address: Some(cell_address),
                    value: f64::INFINITY,
                },
            );
            let cells = builder.create_vector(&[cell]);
            let payload = wire::SparseTensorCells::create(
                builder,
                &wire::SparseTensorCellsArgs { cells: Some(cells) },
            );
            (
                wire::TensorPayloadKind::SparseTensorCells,
                payload.as_union_value(),
                "tensor<float>(category{})",
            )
        }
        "mixed-negative-infinity" => {
            let block_address = address(builder);
            let block_address = builder.create_vector(&[block_address]);
            let values = builder.create_vector(&[f64::NEG_INFINITY]);
            let block = wire::TensorBlock::create(
                builder,
                &wire::TensorBlockArgs {
                    address: Some(block_address),
                    values: Some(values),
                },
            );
            let blocks = builder.create_vector(&[block]);
            let payload = wire::MixedTensorBlocks::create(
                builder,
                &wire::MixedTensorBlocksArgs {
                    blocks: Some(blocks),
                },
            );
            (
                wire::TensorPayloadKind::MixedTensorBlocks,
                payload.as_union_value(),
                "tensor<float>(category{},x[1])",
            )
        }
        _ => unreachable!("only the named malformed tensor fixtures are supported"),
    }
}

fn encode(name: &'static str) -> Vec<u8> {
    let mut builder = FlatBufferBuilder::new();
    let (payload_type, payload, type_spec) = malformed_tensor_payload(&mut builder, name);
    let type_spec = builder.create_string(type_spec);
    let tensor = wire::TensorValue::create(
        &mut builder,
        &wire::TensorValueArgs {
            type_spec: Some(type_spec),
            payload_type,
            payload: Some(payload),
        },
    );
    let value = wire::Value::create(
        &mut builder,
        &wire::ValueArgs {
            kind_type: wire::ValueKind::TensorValue,
            kind: Some(tensor.as_union_value()),
        },
    );
    let key = builder.create_string("tensor");
    let field = wire::ValueEntry::create(
        &mut builder,
        &wire::ValueEntryArgs {
            key: Some(key),
            value: Some(value),
        },
    );
    let fields = builder.create_vector(&[field]);
    let id = builder.create_string("malformed-tensor");
    let hit = wire::Hit::create(
        &mut builder,
        &wire::HitArgs {
            id: Some(id),
            fields: Some(fields),
            ..Default::default()
        },
    );
    let child = wire::ResultNode::create(
        &mut builder,
        &wire::ResultNodeArgs {
            node_type: wire::ResultNodeKind::Hit,
            node: Some(hit.as_union_value()),
        },
    );
    let children = builder.create_vector(&[child]);
    let group = wire::ResultGroup::create(
        &mut builder,
        &wire::ResultGroupArgs {
            children: Some(children),
            ..Default::default()
        },
    );
    let root = wire::ResultNode::create(
        &mut builder,
        &wire::ResultNodeArgs {
            node_type: wire::ResultNodeKind::ResultGroup,
            node: Some(group.as_union_value()),
        },
    );
    let total_hits = wire::TotalHits::create(
        &mut builder,
        &wire::TotalHitsArgs {
            value: 1,
            relation: wire::TotalHitsRelation::Exact,
        },
    );
    let result = wire::SearchResult::create(
        &mut builder,
        &wire::SearchResultArgs {
            root: Some(root),
            total_hits: Some(total_hits),
            ..Default::default()
        },
    );
    let envelope = wire::ResponseEnvelope::create(
        &mut builder,
        &wire::ResponseEnvelopeArgs {
            protocol_version: PROTOCOL_VERSION,
            payload_type: wire::ResponsePayload::SearchKernel_Wire_SearchResult,
            payload: Some(result.as_union_value()),
        },
    );
    builder.finish(envelope, Some("SKRS"));
    builder.finished_data().to_vec()
}

/// Raw responses containing NaN or infinity inside each portable tensor encoding.
pub fn malformed_tensor_response_fixtures() -> Vec<(&'static str, Vec<u8>)> {
    [
        ("dense-nan", "invalid-tensor-dense-nan"),
        (
            "sparse-positive-infinity",
            "invalid-tensor-sparse-positive-infinity",
        ),
        (
            "mixed-negative-infinity",
            "invalid-tensor-mixed-negative-infinity",
        ),
    ]
    .into_iter()
    .map(|(encoding, filename)| (filename, encode(encoding)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_tensor_responses_keep_required_search_result_fields() {
        for (name, bytes) in malformed_tensor_response_fixtures() {
            let envelope = flatbuffers::root::<wire::ResponseEnvelope>(&bytes)
                .expect("malformed tensor fixture must be a valid response envelope");
            let result = envelope
                .payload_as_search_kernel_wire_search_result()
                .expect("malformed tensor fixture must contain a SearchResult");
            assert!(
                result.root().is_some(),
                "{name} must retain SearchResult.root"
            );
            assert!(
                result.total_hits().is_some(),
                "{name} must retain SearchResult.total_hits"
            );
        }
    }
}
