use super::*;

pub(super) fn request_value_entries_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    values: &std::collections::BTreeMap<String, ir::RequestValueIr>,
) -> Option<
    flatbuffers::WIPOffset<
        flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::ValueEntry<'a>>>,
    >,
> {
    (!values.is_empty()).then(|| {
        let entries = values
            .iter()
            .map(|(key, value)| {
                let key = builder.create_string(key);
                let value = request_value_offset(builder, value);
                api::ValueEntry::create(
                    builder,
                    &api::ValueEntryArgs {
                        key: Some(key),
                        value: Some(value),
                    },
                )
            })
            .collect::<Vec<_>>();
        builder.create_vector(&entries)
    })
}

pub(super) fn request_value_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    value: &ir::RequestValueIr,
) -> flatbuffers::WIPOffset<api::Value<'a>> {
    let (kind_type, kind) = match value {
        ir::RequestValueIr::Null => (
            api::ValueKind::NullValueValue,
            api::NullValueValue::create(
                builder,
                &api::NullValueValueArgs {
                    value: api::NullValue::Null,
                },
            )
            .as_union_value(),
        ),
        ir::RequestValueIr::Bool(value) => (
            api::ValueKind::BoolValue,
            api::BoolValue::create(builder, &api::BoolValueArgs { value: *value }).as_union_value(),
        ),
        ir::RequestValueIr::Int64(value) => (
            api::ValueKind::Int64Value,
            api::Int64Value::create(builder, &api::Int64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::UInt64(value) => (
            api::ValueKind::UInt64Value,
            api::UInt64Value::create(builder, &api::UInt64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::Float64(value) => (
            api::ValueKind::Float64Value,
            api::Float64Value::create(builder, &api::Float64ValueArgs { value: *value })
                .as_union_value(),
        ),
        ir::RequestValueIr::NonFiniteFloat(value) => {
            let kind = match value {
                ir::RequestNonFiniteFloatIr::NaN => api::NonFiniteFloatKind::NaN,
                ir::RequestNonFiniteFloatIr::PositiveInfinity => {
                    api::NonFiniteFloatKind::PositiveInfinity
                }
                ir::RequestNonFiniteFloatIr::NegativeInfinity => {
                    api::NonFiniteFloatKind::NegativeInfinity
                }
            };
            (
                api::ValueKind::NonFiniteFloatValue,
                api::NonFiniteFloatValue::create(builder, &api::NonFiniteFloatValueArgs { kind })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::String(value) => {
            let value = builder.create_string(value);
            (
                api::ValueKind::StringValue,
                api::StringValue::create(builder, &api::StringValueArgs { value: Some(value) })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::Bytes(value) => {
            let value = builder.create_vector(value);
            (
                api::ValueKind::BytesValue,
                api::BytesValue::create(builder, &api::BytesValueArgs { value: Some(value) })
                    .as_union_value(),
            )
        }
        ir::RequestValueIr::List(values) => {
            let values = values
                .iter()
                .map(|value| request_value_offset(builder, value))
                .collect::<Vec<_>>();
            let values = builder.create_vector(&values);
            (
                api::ValueKind::ValueList,
                api::ValueList::create(
                    builder,
                    &api::ValueListArgs {
                        values: Some(values),
                    },
                )
                .as_union_value(),
            )
        }
        ir::RequestValueIr::Object(values) => {
            let entries = values
                .iter()
                .map(|(key, value)| {
                    let key = builder.create_string(key);
                    let value = request_value_offset(builder, value);
                    api::ValueEntry::create(
                        builder,
                        &api::ValueEntryArgs {
                            key: Some(key),
                            value: Some(value),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let values = builder.create_vector(&entries);
            (
                api::ValueKind::ValueObject,
                api::ValueObject::create(
                    builder,
                    &api::ValueObjectArgs {
                        fields: Some(values),
                    },
                )
                .as_union_value(),
            )
        }
        ir::RequestValueIr::Tensor(value) => (
            api::ValueKind::TensorValue,
            request_tensor_offset(builder, value).as_union_value(),
        ),
    };
    api::Value::create(
        builder,
        &api::ValueArgs {
            kind_type,
            kind: Some(kind),
        },
    )
}

pub(super) fn tensor_address_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    address: &std::collections::BTreeMap<String, String>,
) -> flatbuffers::WIPOffset<
    flatbuffers::Vector<'a, flatbuffers::ForwardsUOffset<api::TensorAddressEntry<'a>>>,
> {
    let values = address
        .iter()
        .map(|(dimension, label)| {
            let dimension = builder.create_string(dimension);
            let label = builder.create_string(label);
            api::TensorAddressEntry::create(
                builder,
                &api::TensorAddressEntryArgs {
                    dimension: Some(dimension),
                    label: Some(label),
                },
            )
        })
        .collect::<Vec<_>>();
    builder.create_vector(&values)
}

pub(super) fn request_tensor_offset<'a>(
    builder: &mut flatbuffers::FlatBufferBuilder<'a>,
    tensor: &ir::RequestTensorIr,
) -> flatbuffers::WIPOffset<api::TensorValue<'a>> {
    let type_spec = builder.create_string(&tensor.type_spec);
    let (payload_type, payload) = match &tensor.payload {
        ir::RequestTensorPayloadIr::Dense(values) => {
            // Absent and empty vectors decode identically. Omitting this vector
            // keeps empty dense tensors valid through the BFBS JSON parser,
            // which can misalign an explicit empty vector of doubles.
            let values = (!values.is_empty()).then(|| builder.create_vector(values));
            let value = api::DenseTensorValues::create(
                builder,
                &api::DenseTensorValuesArgs {
                    values,
                },
            );
            (
                api::TensorPayloadKind::DenseTensorValues,
                value.as_union_value(),
            )
        }
        ir::RequestTensorPayloadIr::Sparse(cells) => {
            let cells = cells
                .iter()
                .map(|cell| {
                    let address = tensor_address_offset(builder, &cell.address);
                    api::TensorCell::create(
                        builder,
                        &api::TensorCellArgs {
                            address: Some(address),
                            value: cell.value,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let cells = builder.create_vector(&cells);
            let value = api::SparseTensorCells::create(
                builder,
                &api::SparseTensorCellsArgs { cells: Some(cells) },
            );
            (
                api::TensorPayloadKind::SparseTensorCells,
                value.as_union_value(),
            )
        }
        ir::RequestTensorPayloadIr::Mixed(blocks) => {
            let blocks = blocks
                .iter()
                .map(|block| {
                    let address = tensor_address_offset(builder, &block.address);
                    let values = builder.create_vector(&block.values);
                    api::TensorBlock::create(
                        builder,
                        &api::TensorBlockArgs {
                            address: Some(address),
                            values: Some(values),
                        },
                    )
                })
                .collect::<Vec<_>>();
            let blocks = builder.create_vector(&blocks);
            let value = api::MixedTensorBlocks::create(
                builder,
                &api::MixedTensorBlocksArgs {
                    blocks: Some(blocks),
                },
            );
            (
                api::TensorPayloadKind::MixedTensorBlocks,
                value.as_union_value(),
            )
        }
    };
    api::TensorValue::create(
        builder,
        &api::TensorValueArgs {
            type_spec: Some(type_spec),
            payload_type,
            payload: Some(payload),
        },
    )
}
