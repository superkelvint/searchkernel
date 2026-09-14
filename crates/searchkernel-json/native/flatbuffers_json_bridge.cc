#include "flatbuffers_json_bridge.h"

#include <cstdlib>
#include <cstring>
#include <exception>
#include <string>

#include "flatbuffers/idl.h"
#include "flatbuffers/reflection.h"
#include "flatbuffers/reflection_generated.h"

namespace {

flatbuffers::IDLOptions json_options() {
  flatbuffers::IDLOptions options;
  options.strict_json = true;
  return options;
}

void set_error(const std::string& message, char** error_ptr, size_t* error_len) {
  if (message.empty()) return;
  auto* memory = static_cast<char*>(std::malloc(message.size()));
  if (memory == nullptr) return;
  std::memcpy(memory, message.data(), message.size());
  *error_ptr = memory;
  *error_len = message.size();
}

int fail(int status, const std::string& message, char** error_ptr,
         size_t* error_len) {
  set_error(message, error_ptr, error_len);
  return status;
}

bool deserialize_schema(const uint8_t* bfbs, size_t bfbs_len,
                        flatbuffers::Parser* parser, std::string* error) {
  if (!parser->Deserialize(bfbs, bfbs_len)) {
    *error = parser->error_.empty() ? "could not deserialize BFBS" : parser->error_;
    return false;
  }
  return true;
}

bool verify_buffer(const uint8_t* bfbs, size_t bfbs_len, const uint8_t* buffer,
                   size_t buffer_len, std::string* error) {
  flatbuffers::Verifier schema_verifier(bfbs, bfbs_len);
  if (!::reflection::VerifySchemaBuffer(schema_verifier)) {
    *error = "BFBS reflection schema is invalid";
    return false;
  }
  const auto* schema = ::reflection::GetSchema(bfbs);
  const auto* root = schema->root_table();
  if (root == nullptr) {
    *error = "BFBS reflection schema has no root table";
    return false;
  }
  if (!flatbuffers::Verify(*schema, *root, buffer, buffer_len)) {
    *error = "FlatBuffer failed reflection verification";
    return false;
  }
  if (schema->file_ident() != nullptr &&
      !flatbuffers::BufferHasIdentifier(buffer, schema->file_ident()->c_str(),
                                        false)) {
    *error = "FlatBuffer has the wrong file identifier";
    return false;
  }
  return true;
}

int json_to_flatbuffer(const uint8_t* bfbs, size_t bfbs_len, const uint8_t* json,
                      size_t json_len, uint8_t** out_ptr, size_t* out_len,
                      char** error_ptr, size_t* error_len) {
  flatbuffers::Parser parser(json_options());
  std::string error;
  if (!deserialize_schema(bfbs, bfbs_len, &parser, &error)) {
    return fail(1, error, error_ptr, error_len);
  }

  if (json_len != 0 && std::memchr(json, '\0', json_len) != nullptr) {
    return fail(2, "JSON input contains NUL byte", error_ptr, error_len);
  }
  std::string text(reinterpret_cast<const char*>(json), json_len);
  if (!parser.ParseJson(text.c_str())) {
    return fail(2, parser.error_, error_ptr, error_len);
  }

  const auto size = parser.builder_.GetSize();
  auto* memory = static_cast<uint8_t*>(std::malloc(size));
  if (memory == nullptr) {
    return fail(4, "could not allocate FlatBuffer output", error_ptr, error_len);
  }
  std::memcpy(memory, parser.builder_.GetBufferPointer(), size);
  *out_ptr = memory;
  *out_len = size;
  return 0;
}

int flatbuffer_to_json(const uint8_t* bfbs, size_t bfbs_len,
                       const uint8_t* buffer, size_t buffer_len,
                       uint8_t** out_ptr, size_t* out_len, char** error_ptr,
                       size_t* error_len) {
  flatbuffers::Parser parser(json_options());
  std::string error;
  if (!deserialize_schema(bfbs, bfbs_len, &parser, &error)) {
    return fail(1, error, error_ptr, error_len);
  }
  if (!verify_buffer(bfbs, bfbs_len, buffer, buffer_len, &error)) {
    return fail(3, error, error_ptr, error_len);
  }

  std::string text;
  if (const char* generation_error =
          flatbuffers::GenText(parser, buffer, &text);
      generation_error != nullptr) {
    return fail(3, generation_error, error_ptr, error_len);
  }
  auto* memory = static_cast<uint8_t*>(std::malloc(text.size()));
  if (memory == nullptr) {
    return fail(4, "could not allocate JSON output", error_ptr, error_len);
  }
  std::memcpy(memory, text.data(), text.size());
  *out_ptr = memory;
  *out_len = text.size();
  return 0;
}

}  // namespace

extern "C" int sk_fbjson_json_to_flatbuffer(
    const uint8_t* bfbs, size_t bfbs_len, const uint8_t* json, size_t json_len,
    uint8_t** out_ptr, size_t* out_len, char** error_ptr, size_t* error_len) {
  if (out_ptr == nullptr || out_len == nullptr || error_ptr == nullptr ||
      error_len == nullptr || (bfbs == nullptr && bfbs_len != 0) ||
      (json == nullptr && json_len != 0)) {
    return 4;
  }
  *out_ptr = nullptr;
  *out_len = 0;
  *error_ptr = nullptr;
  *error_len = 0;
  try {
    return json_to_flatbuffer(bfbs, bfbs_len, json, json_len, out_ptr, out_len,
                              error_ptr, error_len);
  } catch (const std::exception& exception) {
    return fail(4, exception.what(), error_ptr, error_len);
  } catch (...) {
    return fail(4, "unknown native exception", error_ptr, error_len);
  }
}

extern "C" int sk_fbjson_flatbuffer_to_json(
    const uint8_t* bfbs, size_t bfbs_len, const uint8_t* buffer,
    size_t buffer_len, uint8_t** out_ptr, size_t* out_len, char** error_ptr,
    size_t* error_len) {
  if (out_ptr == nullptr || out_len == nullptr || error_ptr == nullptr ||
      error_len == nullptr || (bfbs == nullptr && bfbs_len != 0) ||
      (buffer == nullptr && buffer_len != 0)) {
    return 4;
  }
  *out_ptr = nullptr;
  *out_len = 0;
  *error_ptr = nullptr;
  *error_len = 0;
  try {
    return flatbuffer_to_json(bfbs, bfbs_len, buffer, buffer_len, out_ptr,
                              out_len, error_ptr, error_len);
  } catch (const std::exception& exception) {
    return fail(4, exception.what(), error_ptr, error_len);
  } catch (...) {
    return fail(4, "unknown native exception", error_ptr, error_len);
  }
}

extern "C" void sk_fbjson_free(void* ptr) { std::free(ptr); }
