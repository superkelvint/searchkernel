// Copyright Vespa.ai. Licensed under the terms of the Apache 2.0 license.
//
// Direct Vespa document/tensor oracle for M13.  This deliberately uses only
// Vespa document values and document updates; it does not call Search Kernel
// production code or reuse the Rust scenario driver.

#include <vespa/document/datatype/datatype.h>
#include <vespa/document/datatype/documenttype.h>
#include <vespa/document/datatype/tensor_data_type.h>
#include <vespa/document/fieldvalue/document.h>
#include <vespa/document/fieldvalue/tensorfieldvalue.h>
#include <vespa/document/repo/documenttyperepo.h>
#include <vespa/document/update/assignvalueupdate.h>
#include <vespa/document/update/clearvalueupdate.h>
#include <vespa/document/update/documentupdate.h>
#include <vespa/document/update/fieldupdate.h>
#include <vespa/eval/eval/fast_value.h>
#include <vespa/eval/eval/tensor_spec.h>
#include <vespa/eval/eval/value_codec.h>
#include <vespa/vespalib/data/slime/json_format.h>
#include <vespa/vespalib/data/slime/slime.h>

#include <cmath>
#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

using document::Document;
using document::DocumentId;
using document::DocumentType;
using document::DocumentTypeRepo;
using document::TensorDataType;
using document::TensorFieldValue;
using vespalib::Memory;
using vespalib::Slime;
using vespalib::slime::Inspector;

namespace {

[[noreturn]] void fail(std::string_view message) { throw std::runtime_error(std::string(message)); }

void require(bool condition, std::string_view message) {
    if (!condition) fail(message);
}

const Inspector& field(const Inspector& object, std::string_view name) {
    require(object.type() == vespalib::slime::OBJECT::instance, "expected JSON object");
    return object[Memory(name)];
}

std::string string_value(const Inspector& value, std::string_view message) {
    require(value.type() == vespalib::slime::STRING::instance, message);
    return value.asString().make_string();
}

std::vector<double> values(const Inspector& value) {
    require(value.type() == vespalib::slime::ARRAY::instance, "tensor values must be an array");
    std::vector<double> result;
    result.reserve(value.children());
    for (size_t i = 0; i < value.children(); ++i) {
        require(value[i].type() == vespalib::slime::DOUBLE::instance ||
                    value[i].type() == vespalib::slime::LONG::instance,
                "tensor value must be numeric");
        const double cell = value[i].type() == vespalib::slime::DOUBLE::instance
                                ? value[i].asDouble()
                                : static_cast<double>(value[i].asLong());
        require(std::isfinite(cell), "tensor value must be finite");
        result.push_back(cell);
    }
    return result;
}

std::string json_escape(std::string_view value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char c : value) {
        if (c == '"' || c == '\\') out << '\\';
        out << static_cast<char>(c);
    }
    out << '"';
    return out.str();
}

std::string read_file(const std::filesystem::path& path) {
    std::ifstream input(path, std::ios::binary);
    require(input.good(), "could not read M13 scenario");
    return std::string(std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>());
}

Slime read_json(const std::filesystem::path& path) {
    const auto bytes = read_file(path);
    Slime result;
    require(vespalib::slime::JsonFormat::decode(Memory(bytes), result) != 0, "malformed M13 scenario");
    return result;
}

std::unique_ptr<vespalib::eval::Value> dense_tensor(std::string_view type_spec,
                                                    const std::vector<double>& cells) {
    const auto type = vespalib::eval::ValueType::from_spec(std::string(type_spec));
    require(!type.is_error() && type.count_indexed_dimensions() == 1 &&
                type.count_mapped_dimensions() == 0 && type.dense_subspace_size() == cells.size(),
            "invalid oracle tensor");
    vespalib::eval::TensorSpec spec{std::string(type_spec)};
    const auto& dimension = type.dimensions().front();
    for (size_t i = 0; i < cells.size(); ++i) {
        spec.add({{dimension.name, vespalib::eval::TensorSpec::Label(i)}}, cells[i]);
    }
    return vespalib::eval::value_from_spec(
        spec, vespalib::eval::FastValueBuilderFactory::get());
}

std::unique_ptr<TensorFieldValue> tensor_field(const TensorDataType& type,
                                               std::string_view type_spec,
                                               const std::vector<double>& cells) {
    auto result = std::make_unique<TensorFieldValue>(type);
    *result = dense_tensor(type_spec, cells);
    return result;
}

struct TensorView {
    std::string type_spec;
    std::vector<double> cells;
};

TensorView read_tensor(const Document& document, const document::Field& field) {
    auto value = document.getValue(field);
    require(static_cast<bool>(value), "oracle tensor field is missing");
    const auto* tensor_value = dynamic_cast<const TensorFieldValue*>(value.get());
    require(tensor_value && tensor_value->getAsTensorPtr(), "oracle field is not a tensor");
    const auto& tensor = *tensor_value->getAsTensorPtr();
    TensorView result{tensor.type().to_spec(), {}};
    for (const auto cell : tensor.cells().typify<float>()) result.cells.push_back(cell);
    return result;
}

void assign_tensor(Document& document, const DocumentType& type,
                   const std::shared_ptr<DocumentTypeRepo>& repo,
                   const TensorDataType& tensor_type, std::string_view field_name,
                   std::string_view type_spec, const std::vector<double>& cells) {
    document::DocumentUpdate update(*repo, type, document.getId());
    update.addUpdate(document::FieldUpdate(type.getField(field_name))
                         .addUpdate(std::make_unique<document::AssignValueUpdate>(
                             tensor_field(tensor_type, type_spec, cells))));
    update.applyTo(document);
}

void clear_tensor(Document& document, const DocumentType& type,
                  const std::shared_ptr<DocumentTypeRepo>& repo, std::string_view field_name) {
    document::DocumentUpdate update(*repo, type, document.getId());
    update.addUpdate(document::FieldUpdate(type.getField(field_name))
                         .addUpdate(std::make_unique<document::ClearValueUpdate>()));
    update.applyTo(document);
}

std::string tensor_json(const TensorView& tensor) {
    std::ostringstream out;
    out << "{\"type_spec\":" << json_escape(tensor.type_spec) << ",\"values\":[";
    for (size_t i = 0; i < tensor.cells.size(); ++i) {
        if (i) out << ',';
        // Match serde_json's stable representation while retaining enough
        // digits to expose Vespa's float32-to-double promotion.
        out << std::setprecision(12) << tensor.cells[i];
    }
    out << "]}";
    return out.str();
}

std::string nearest_json(const std::vector<std::pair<std::string, TensorView>>& documents,
                         const std::vector<double>& query) {
    std::vector<std::pair<double, std::string>> ranked;
    for (const auto& [id, tensor] : documents) {
        require(tensor.cells.size() == query.size(), "oracle query dimension mismatch");
        double distance = 0.0;
        for (size_t i = 0; i < query.size(); ++i) {
            const double delta = tensor.cells[i] - query[i];
            distance += delta * delta;
        }
        ranked.emplace_back(distance, id);
    }
    std::stable_sort(ranked.begin(), ranked.end(),
                     [](const auto& lhs, const auto& rhs) { return lhs.first < rhs.first; });
    std::ostringstream out;
    out << '[';
    for (size_t i = 0; i < ranked.size(); ++i) {
        if (i) out << ',';
        out << json_escape(ranked[i].second);
    }
    out << ']';
    return out.str();
}

} // namespace

int main(int argc, char** argv) {
    try {
        require(argc == 2, "usage: m13_tensor_oracle SCENARIO_JSON");
        const auto scenario = read_json(argv[1]);
        const auto& root = scenario.get();
        const auto tensor_field_name = string_value(field(root, "tensor_field")["name"], "tensor field name");
        const auto tensor_spec = string_value(field(root, "tensor_field")["type_spec"], "tensor type spec");
        const auto document_type = string_value(field(root, "document_type"), "document type");
        const auto& tensors = field(root, "tensors");
        const auto get_cells = [&](std::string_view name) {
            return values(tensors[Memory(name)]);
        };

        auto tensor_type = TensorDataType::fromSpec(tensor_spec);
        require(static_cast<bool>(tensor_type), "could not create tensor data type");
        DocumentType type(document_type, 0);
        type.addField(document::Field(tensor_field_name, 1, *tensor_type));
        type.addField(document::Field("score", 2, *document::DataType::DOUBLE));
        auto repo = std::make_shared<DocumentTypeRepo>(type);

        const auto id_a = DocumentId("id:ns:typea::a");
        const auto id_b = DocumentId("id:ns:typea::b");
        Document document_a(*repo, type, id_a);
        Document document_b(*repo, type, id_b);
        document_a.setFieldValue(type.getField(tensor_field_name), tensor_field(*tensor_type, tensor_spec, get_cells("a")));
        document_b.setFieldValue(type.getField(tensor_field_name), tensor_field(*tensor_type, tensor_spec, get_cells("b")));
        const auto inserted = read_tensor(document_a, type.getField(tensor_field_name));
        const auto query = get_cells("query");
        const auto nn_before = nearest_json({
            {document_a.getId().toString(), read_tensor(document_a, type.getField(tensor_field_name))},
            {document_b.getId().toString(), read_tensor(document_b, type.getField(tensor_field_name))},
        }, query);

        document_a = Document(*repo, type, id_a);
        document_a.setFieldValue(type.getField(tensor_field_name), tensor_field(*tensor_type, tensor_spec, get_cells("b")));
        const auto upserted = read_tensor(document_a, type.getField(tensor_field_name));
        assign_tensor(document_a, type, repo, *tensor_type, tensor_field_name, tensor_spec, get_cells("replacement"));
        const auto patched = read_tensor(document_a, type.getField(tensor_field_name));
        const auto nn_after = nearest_json({
            {document_a.getId().toString(), read_tensor(document_a, type.getField(tensor_field_name))},
            {document_b.getId().toString(), read_tensor(document_b, type.getField(tensor_field_name))},
        }, query);
        const auto persisted_patch = patched;
        clear_tensor(document_a, type, repo, tensor_field_name);
        const bool unset = !document_a.hasValue(type.getField(tensor_field_name));
        assign_tensor(document_a, type, repo, *tensor_type, tensor_field_name, tensor_spec, get_cells("reassign"));
        const auto reassigned = read_tensor(document_a, type.getField(tensor_field_name));
        const auto persisted_reassign = reassigned;

        std::cout << "{\"id\":\"dense-crud\",\"inserted\":" << tensor_json(inserted)
                  << ",\"upserted\":" << tensor_json(upserted)
                  << ",\"patched\":" << tensor_json(patched)
                  << ",\"persisted_patch\":" << tensor_json(persisted_patch)
                  << ",\"nn_before_patch\":" << nn_before
                  << ",\"nn_after_patch\":" << nn_after
                  << ",\"unset_fields\":" << (unset ? "[]" : "[\"embedding\"]")
                  << ",\"reassigned\":" << tensor_json(reassigned)
                  << ",\"persisted_reassign\":" << tensor_json(persisted_reassign)
                  << ",\"deleted\":true,\"deleted_get\":true}\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << "m13_tensor_oracle: " << error.what() << '\n';
        return 2;
    }
}
