// Search Kernel native adapter: tensor validation and conversion.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

std::string encode_dense_tensor(const char *type_spec, const double *values,
                                uint32_t value_count) {
    if (!type_spec || !values) {
        throw std::invalid_argument("dense tensor arguments must not be null");
    }
    validate_string_size(type_spec, MAX_RANK_PROFILE_BYTES, "tensor type spec");
    const auto type = vespalib::eval::ValueType::from_spec(type_spec);
    if (type.is_error()) {
        throw TensorValidationError("tensor_type_spec_invalid", "tensor type specification is malformed");
    }
    if (type.count_indexed_dimensions() != 1 || type.count_mapped_dimensions() != 0) {
        throw TensorValidationError("tensor_shape_unsupported",
                                    "dense tensors must have exactly one indexed dimension and no mapped dimensions");
    }
    if (type.dense_subspace_size() != value_count) {
        throw TensorValidationError("tensor_cell_count_mismatch",
                                    "dense tensor cell count does not match its type spec");
    }
    vespalib::eval::TensorSpec spec(type_spec);
    const auto &dimension = type.dimensions().front();
    for (uint32_t index = 0; index < value_count; ++index) {
        if (!std::isfinite(values[index])) {
            throw TensorValidationError("tensor_non_finite_cell", "tensor cell values must be finite");
        }
        spec.add({{dimension.name, vespalib::eval::TensorSpec::Label(index)}}, values[index]);
    }
    auto tensor = vespalib::eval::value_from_spec(
        spec, vespalib::eval::FastValueBuilderFactory::get());
    if (!tensor) throw std::runtime_error("could not construct dense tensor");
    vespalib::nbostream stream;
    vespalib::eval::encode_value(*tensor, stream);
    return std::string(stream.data(), stream.size());
}
template <typename Cell>
void copy_tensor_cells(const vespalib::eval::TypedCells &cells,
                       std::vector<double> &output) {
    for (const auto cell : cells.typify<Cell>()) {
        const double value = static_cast<double>(cell);
        if (!std::isfinite(value)) {
            throw TensorValidationError("tensor_readback_non_finite", "native tensor contains a non-finite cell");
        }
        output.push_back(value);
    }
}

std::vector<double> dense_tensor_cells(const vespalib::eval::Value &tensor) {
    const auto &type = tensor.type();
    if (type.is_error() || type.count_indexed_dimensions() != 1 ||
        type.count_mapped_dimensions() != 0 || type.dense_subspace_size() == 0) {
        throw TensorValidationError("tensor_readback_invalid", "native tensor readback is not a supported dense tensor");
    }
    const auto cells = tensor.cells();
    if (cells.size != type.dense_subspace_size()) {
        throw std::runtime_error("native tensor cell count does not match its type");
    }
    std::vector<double> output;
    output.reserve(cells.size);
    switch (cells.type) {
    case vespalib::eval::CellType::DOUBLE:
        copy_tensor_cells<double>(cells, output);
        break;
    case vespalib::eval::CellType::FLOAT:
        copy_tensor_cells<float>(cells, output);
        break;
    case vespalib::eval::CellType::BFLOAT16:
        copy_tensor_cells<vespalib::BFloat16>(cells, output);
        break;
    case vespalib::eval::CellType::INT8:
        copy_tensor_cells<vespalib::eval::Int8Float>(cells, output);
        break;
    default:
        throw std::runtime_error("native tensor cell type is unsupported");
    }
    return output;
}

OwnedValue make_dense_tensor_value(const char *type_spec, const double *values,
                                   uint32_t value_count) {
    OwnedValue result;
    result.kind = SK_VALUE_TENSOR;
    result.tensor_type = type_spec;
    result.string = encode_dense_tensor(type_spec, values, value_count);
    if (value_count != 0) {
        result.tensor_cells.assign(values, values + value_count);
    }
    return result;
}
