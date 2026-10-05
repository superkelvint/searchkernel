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


class DiagnosticTensorBuilderFactory final : public vespalib::eval::ValueBuilderFactory {
    size_t _max_cells;

    template <typename Cell>
    static std::unique_ptr<vespalib::eval::ValueBuilderBase> delegate(
        const vespalib::eval::ValueType &type, bool transient,
        size_t num_mapped_dims, size_t subspace_size,
        size_t expected_subspaces) {
        const auto &factory = vespalib::eval::FastValueBuilderFactory::get();
        if (transient) {
            return factory.create_transient_value_builder<Cell>(
                type, num_mapped_dims, subspace_size, expected_subspaces);
        }
        return factory.create_value_builder<Cell>(
            type, num_mapped_dims, subspace_size, expected_subspaces);
    }

protected:
    std::unique_ptr<vespalib::eval::ValueBuilderBase> create_value_builder_base(
        const vespalib::eval::ValueType &type, bool transient,
        size_t num_mapped_dims, size_t subspace_size,
        size_t expected_subspaces) const override {
        if (subspace_size != 0 &&
            expected_subspaces > _max_cells / subspace_size) {
            throw TensorValidationError(
                "diagnostic_tensor_cell_budget_exceeded",
                "ranking tensor exceeds the diagnostic cell budget");
        }
        switch (type.cell_type()) {
        case vespalib::eval::CellType::DOUBLE:
            return delegate<double>(type, transient, num_mapped_dims,
                                    subspace_size, expected_subspaces);
        case vespalib::eval::CellType::FLOAT:
            return delegate<float>(type, transient, num_mapped_dims,
                                   subspace_size, expected_subspaces);
        case vespalib::eval::CellType::BFLOAT16:
            return delegate<vespalib::BFloat16>(
                type, transient, num_mapped_dims, subspace_size,
                expected_subspaces);
        case vespalib::eval::CellType::INT8:
            return delegate<vespalib::eval::Int8Float>(
                type, transient, num_mapped_dims, subspace_size,
                expected_subspaces);
        }
        throw UnsupportedNativeError(
            "diagnostic_tensor_cell_type_unsupported",
            "ranking tensor cell type is not representable by Search Kernel");
    }

public:
    explicit DiagnosticTensorBuilderFactory(size_t max_cells)
        : _max_cells(max_cells) {}
};

template <typename Cell>
void copy_diagnostic_tensor_cells(const vespalib::eval::TypedCells &cells,
                                  std::vector<double> &output) {
    for (const auto cell : cells.typify<Cell>()) {
        const double value = static_cast<double>(cell);
        if (!std::isfinite(value)) {
            throw UnsupportedNativeError(
                "diagnostic_tensor_non_finite_cell_unsupported",
                "portable diagnostic tensors require finite cell values");
        }
        output.push_back(value);
    }
}

std::vector<double> diagnostic_tensor_cells(
    const vespalib::eval::Value &tensor) {
    const auto cells = tensor.cells();
    std::vector<double> output;
    output.reserve(cells.size);
    switch (cells.type) {
    case vespalib::eval::CellType::DOUBLE:
        copy_diagnostic_tensor_cells<double>(cells, output);
        break;
    case vespalib::eval::CellType::FLOAT:
        copy_diagnostic_tensor_cells<float>(cells, output);
        break;
    case vespalib::eval::CellType::BFLOAT16:
        copy_diagnostic_tensor_cells<vespalib::BFloat16>(cells, output);
        break;
    case vespalib::eval::CellType::INT8:
        copy_diagnostic_tensor_cells<vespalib::eval::Int8Float>(cells, output);
        break;
    default:
        throw UnsupportedNativeError(
            "diagnostic_tensor_cell_type_unsupported",
            "ranking tensor cell type is not representable by Search Kernel");
    }
    return output;
}

DiagnosticTensorState decode_diagnostic_tensor(
    const uint8_t *data, uint32_t length, uint32_t cell_limit) {
    DiagnosticTensorBuilderFactory factory(cell_limit);
    std::unique_ptr<vespalib::eval::Value> tensor;
    try {
        vespalib::nbostream stream(reinterpret_cast<const char *>(data), length);
        tensor = vespalib::eval::decode_value(stream, factory);
        if (stream.size() != 0) {
            throw TensorValidationError(
                "diagnostic_tensor_malformed",
                "ranking tensor payload contains trailing data");
        }
    } catch (const TensorValidationError &) {
        throw;
    } catch (const UnsupportedNativeError &) {
        throw;
    } catch (const vespalib::eval::DecodeValueException &) {
        throw TensorValidationError(
            "diagnostic_tensor_malformed",
            "ranking tensor payload is malformed");
    }
    if (!tensor || tensor->type().is_error()) {
        throw TensorValidationError(
            "diagnostic_tensor_malformed",
            "ranking tensor payload has no valid tensor type");
    }

    auto values = diagnostic_tensor_cells(*tensor);
    if (values.size() > cell_limit) {
        throw TensorValidationError(
            "diagnostic_tensor_cell_budget_exceeded",
            "ranking tensor exceeds the diagnostic cell budget");
    }

    DiagnosticTensorState result;
    const auto &type = tensor->type();
    result.type_spec = type.to_spec();
    const size_t mapped = type.count_mapped_dimensions();
    const size_t indexed = type.count_indexed_dimensions();
    if (mapped == 0) {
        result.kind = SK_DIAGNOSTIC_TENSOR_DENSE;
        result.blocks.push_back({{}, std::move(values)});
        return result;
    }

    result.kind = indexed == 0
        ? SK_DIAGNOSTIC_TENSOR_SPARSE
        : SK_DIAGNOSTIC_TENSOR_MIXED;
    const size_t subspace_size = type.dense_subspace_size();
    if (subspace_size == 0 || values.size() % subspace_size != 0) {
        throw std::runtime_error(
            "decoded diagnostic tensor has an inconsistent dense subspace");
    }
    const size_t expected_blocks = values.size() / subspace_size;
    if (tensor->index().size() != expected_blocks) {
        throw std::runtime_error(
            "decoded diagnostic tensor index size does not match its cells");
    }

    std::vector<vespalib::string_id> labels(mapped);
    std::vector<vespalib::string_id *> label_refs;
    label_refs.reserve(mapped);
    for (auto &label : labels) {
        label_refs.push_back(&label);
    }
    auto view = tensor->index().create_view({});
    view->lookup({});
    size_t subspace_id = 0;
    while (view->next_result(label_refs, subspace_id)) {
        if (subspace_id >= expected_blocks) {
            throw std::runtime_error(
                "decoded diagnostic tensor returned an invalid subspace index");
        }
        DiagnosticTensorBlockState block;
        block.address.reserve(mapped);
        size_t label_index = 0;
        for (const auto &dimension : type.dimensions()) {
            if (dimension.is_mapped()) {
                block.address.emplace_back(
                    dimension.name,
                    vespalib::SharedStringRepo::Handle::string_from_id(
                        labels[label_index++]));
            }
        }
        if (label_index != mapped) {
            throw std::runtime_error(
                "decoded diagnostic tensor address is incomplete");
        }
        const auto begin =
            values.begin() + static_cast<std::ptrdiff_t>(subspace_id * subspace_size);
        block.values.assign(
            begin, begin + static_cast<std::ptrdiff_t>(subspace_size));
        if (result.kind == SK_DIAGNOSTIC_TENSOR_SPARSE &&
            block.values.size() != 1) {
            throw std::runtime_error(
                "decoded sparse diagnostic tensor has a dense subspace");
        }
        result.blocks.push_back(std::move(block));
    }
    if (result.blocks.size() != expected_blocks) {
        throw std::runtime_error(
            "decoded diagnostic tensor did not enumerate every subspace");
    }
    return result;
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
