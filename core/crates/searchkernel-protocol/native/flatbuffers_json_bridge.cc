#include "flatbuffers_json_bridge.h"

#include <cstdlib>
#include <cstring>
#include <cctype>
#include <cstdint>
#include <exception>
#include <map>
#include <string>
#include <string_view>
#include <utility>

#include "flatbuffers/idl.h"
#include "flatbuffers/reflection.h"
#include "flatbuffers/reflection_generated.h"

namespace {

flatbuffers::IDLOptions json_options() {
  flatbuffers::IDLOptions options;
  options.strict_json = true;
  // Search Kernel portable semantics distinguish a present empty string/vector
  // from an absent field. FlatBuffers' JSON parser defaults compact empty
  // strings/vectors to null, which would erase that distinction before the
  // canonical *_to_ir() layer sees the request.
  options.set_empty_strings_to_null = false;
  options.set_empty_vectors_to_null = false;
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

enum class IntegerJsonMode {
  RequireDecimalStrings,
  QuoteDecimalNumbers,
};

// FlatBuffers accepts quoted numeric scalar literals when parsing JSON, but
// GenText prints 64-bit integer scalars as bare JSON numbers. Search Kernel's
// canonical JSON spelling is a decimal string so IEEE-754-only JSON runtimes
// cannot silently round portable Int64/UInt64 values.
//
// This transformer is deliberately schema-driven. It does not know any Search
// Kernel field names or domain semantics: the deserialized BFBS tells it which
// JSON values are LONG/ULONG, while FlatBuffers remains the parser/printer and
// semantic authority.
class IntegerJsonCanonicalizer {
 public:
  IntegerJsonCanonicalizer(const flatbuffers::Parser& parser,
                           std::string_view input, IntegerJsonMode mode,
                           std::string* output, std::string* error)
      : parser_(parser),
        input_(input),
        mode_(mode),
        output_(output),
        error_(error) {}

  bool Run() {
    if (parser_.root_struct_def_ == nullptr) {
      return Fail("BFBS parser has no root table");
    }
    flatbuffers::Type root(flatbuffers::BASE_TYPE_STRUCT,
                           parser_.root_struct_def_);
    if (!Value(root)) return false;
    Whitespace();
    if (position_ != input_.size()) {
      return Fail("trailing JSON after root value");
    }
    return true;
  }

 private:
  class DepthScope {
   public:
    explicit DepthScope(size_t* depth) : depth_(depth) { ++*depth_; }
    ~DepthScope() { --*depth_; }

   private:
    size_t* depth_;
  };

  bool EnterDepth() {
    if (depth_ >= FLATBUFFERS_MAX_PARSING_DEPTH) {
      return Fail("JSON nesting exceeds FlatBuffers parsing depth");
    }
    return true;
  }

  bool Fail(const std::string& message) {
    *error_ = message + " at JSON byte " + std::to_string(position_);
    return false;
  }

  void Whitespace() {
    while (position_ < input_.size() &&
           std::isspace(static_cast<unsigned char>(input_[position_]))) {
      output_->push_back(input_[position_++]);
    }
  }

  bool Consume(char expected) {
    if (position_ >= input_.size() || input_[position_] != expected) {
      return Fail(std::string("expected '") + expected + "'");
    }
    output_->push_back(input_[position_++]);
    return true;
  }

  static void AppendUtf8(uint32_t codepoint, std::string* out) {
    if (codepoint <= 0x7f) {
      out->push_back(static_cast<char>(codepoint));
    } else if (codepoint <= 0x7ff) {
      out->push_back(static_cast<char>(0xc0 | (codepoint >> 6)));
      out->push_back(static_cast<char>(0x80 | (codepoint & 0x3f)));
    } else if (codepoint <= 0xffff) {
      out->push_back(static_cast<char>(0xe0 | (codepoint >> 12)));
      out->push_back(static_cast<char>(0x80 | ((codepoint >> 6) & 0x3f)));
      out->push_back(static_cast<char>(0x80 | (codepoint & 0x3f)));
    } else {
      out->push_back(static_cast<char>(0xf0 | (codepoint >> 18)));
      out->push_back(static_cast<char>(0x80 | ((codepoint >> 12) & 0x3f)));
      out->push_back(static_cast<char>(0x80 | ((codepoint >> 6) & 0x3f)));
      out->push_back(static_cast<char>(0x80 | (codepoint & 0x3f)));
    }
  }

  bool Hex4(uint32_t* value) {
    if (position_ + 4 > input_.size()) return Fail("truncated JSON unicode escape");
    uint32_t result = 0;
    for (size_t i = 0; i < 4; ++i) {
      const char c = input_[position_++];
      result <<= 4;
      if (c >= '0' && c <= '9') {
        result |= static_cast<uint32_t>(c - '0');
      } else if (c >= 'a' && c <= 'f') {
        result |= static_cast<uint32_t>(10 + c - 'a');
      } else if (c >= 'A' && c <= 'F') {
        result |= static_cast<uint32_t>(10 + c - 'A');
      } else {
        return Fail("invalid JSON unicode escape");
      }
    }
    *value = result;
    return true;
  }

  bool String(std::string* decoded = nullptr) {
    if (position_ >= input_.size() || input_[position_] != '"') {
      return Fail("expected JSON string");
    }
    const size_t start = position_++;
    std::string value;
    while (position_ < input_.size()) {
      const unsigned char c = static_cast<unsigned char>(input_[position_++]);
      if (c == '"') {
        output_->append(input_.substr(start, position_ - start));
        if (decoded != nullptr) *decoded = std::move(value);
        return true;
      }
      if (c < 0x20) return Fail("control byte in JSON string");
      if (c != '\\') {
        value.push_back(static_cast<char>(c));
        continue;
      }
      if (position_ >= input_.size()) return Fail("truncated JSON escape");
      const char escaped = input_[position_++];
      switch (escaped) {
        case '"':
        case '\\':
        case '/':
          value.push_back(escaped);
          break;
        case 'b':
          value.push_back('\b');
          break;
        case 'f':
          value.push_back('\f');
          break;
        case 'n':
          value.push_back('\n');
          break;
        case 'r':
          value.push_back('\r');
          break;
        case 't':
          value.push_back('\t');
          break;
        case 'u': {
          uint32_t codepoint = 0;
          if (!Hex4(&codepoint)) return false;
          if (codepoint >= 0xd800 && codepoint <= 0xdbff) {
            if (position_ + 2 > input_.size() || input_[position_] != '\\' ||
                input_[position_ + 1] != 'u') {
              return Fail("missing low surrogate in JSON string");
            }
            position_ += 2;
            uint32_t low = 0;
            if (!Hex4(&low)) return false;
            if (low < 0xdc00 || low > 0xdfff) {
              return Fail("invalid low surrogate in JSON string");
            }
            codepoint =
                0x10000 + ((codepoint - 0xd800) << 10) + (low - 0xdc00);
          } else if (codepoint >= 0xdc00 && codepoint <= 0xdfff) {
            return Fail("unexpected low surrogate in JSON string");
          }
          AppendUtf8(codepoint, &value);
          break;
        }
        default:
          return Fail("invalid JSON escape");
      }
    }
    return Fail("unterminated JSON string");
  }

  bool Scalar(std::string* token = nullptr) {
    const size_t start = position_;
    while (position_ < input_.size()) {
      const char c = input_[position_];
      if (c == ',' || c == ']' || c == '}' ||
          std::isspace(static_cast<unsigned char>(c))) {
        break;
      }
      ++position_;
    }
    if (position_ == start) return Fail("expected JSON scalar");
    if (token != nullptr) {
      *token = std::string(input_.substr(start, position_ - start));
    }
    output_->append(input_.substr(start, position_ - start));
    return true;
  }

  static bool DecimalString(std::string_view value, bool signed_value) {
    if (value.empty()) return false;
    size_t offset = 0;
    if (signed_value && value[0] == '-') {
      offset = 1;
      if (offset == value.size()) return false;
    }
    for (size_t i = offset; i < value.size(); ++i) {
      if (value[i] < '0' || value[i] > '9') return false;
    }
    return true;
  }

  bool Integer64(const flatbuffers::Type& type) {
    Whitespace();
    if (input_.substr(position_, 4) == "null") {
      return Scalar();
    }

    const bool signed_value = type.base_type == flatbuffers::BASE_TYPE_LONG;
    if (position_ < input_.size() && input_[position_] == '"') {
      std::string decoded;
      if (!String(&decoded)) return false;
      if (!DecimalString(decoded, signed_value)) {
        return Fail("64-bit integer JSON string is not decimal");
      }
      return true;
    }

    if (mode_ == IntegerJsonMode::RequireDecimalStrings) {
      return Fail("64-bit integer JSON values must be decimal strings");
    }

    const size_t output_size = output_->size();
    std::string token;
    if (!Scalar(&token)) return false;
    if (!DecimalString(token, signed_value)) {
      return Fail("FlatBuffers printer emitted a non-decimal 64-bit integer");
    }
    output_->resize(output_size);
    output_->push_back('"');
    output_->append(token);
    output_->push_back('"');
    return true;
  }

  bool UnionDiscriminator(const flatbuffers::FieldDef& field,
                          flatbuffers::Type* union_type) {
    Whitespace();
    if (position_ < input_.size() && input_[position_] == '"') {
      std::string name;
      if (!String(&name)) return false;
      if (field.value.type.enum_def != nullptr) {
        if (const auto* value = field.value.type.enum_def->Lookup(name)) {
          *union_type = value->union_type;
        }
      }
      return true;
    }

    std::string token;
    if (!Scalar(&token)) return false;
    if (field.value.type.enum_def != nullptr) {
      if (const auto* value = field.value.type.enum_def->FindByValue(token)) {
        *union_type = value->union_type;
      }
    }
    return true;
  }

  bool Object(const flatbuffers::StructDef& definition) {
    if (!EnterDepth()) return false;
    DepthScope depth_scope(&depth_);
    if (!Consume('{')) return false;
    Whitespace();
    std::map<const flatbuffers::FieldDef*, flatbuffers::Type> union_types;
    bool first = true;
    while (position_ < input_.size() && input_[position_] != '}') {
      if (!first) {
        if (!Consume(',')) return false;
        Whitespace();
      }
      first = false;

      std::string field_name;
      if (!String(&field_name)) return false;
      Whitespace();
      if (!Consume(':')) return false;
      Whitespace();

      const auto* field = definition.fields.Lookup(field_name);
      if (field == nullptr) {
        if (!AnyValue()) return false;
      } else if (field->value.type.base_type ==
                     flatbuffers::BASE_TYPE_UTYPE &&
                 field->sibling_union_field != nullptr) {
        flatbuffers::Type union_type;
        if (!UnionDiscriminator(*field, &union_type)) return false;
        if (union_type.base_type != flatbuffers::BASE_TYPE_NONE) {
          union_types[field->sibling_union_field] = union_type;
        }
      } else if (field->value.type.base_type ==
                     flatbuffers::BASE_TYPE_UNION &&
                 field->sibling_union_field != nullptr) {
        const auto it = union_types.find(field);
        if (it == union_types.end()) {
          return Fail("union payload appeared before its discriminator");
        }
        if (!Value(it->second)) return false;
      } else {
        if (!Value(field->value.type)) return false;
      }
      Whitespace();
    }
    return Consume('}');
  }

  bool Array(const flatbuffers::Type& element_type) {
    if (!EnterDepth()) return false;
    DepthScope depth_scope(&depth_);
    if (!Consume('[')) return false;
    Whitespace();
    bool first = true;
    while (position_ < input_.size() && input_[position_] != ']') {
      if (!first) {
        if (!Consume(',')) return false;
        Whitespace();
      }
      first = false;
      if (!Value(element_type)) return false;
      Whitespace();
    }
    return Consume(']');
  }

  bool AnyObject() {
    if (!EnterDepth()) return false;
    DepthScope depth_scope(&depth_);
    if (!Consume('{')) return false;
    Whitespace();
    bool first = true;
    while (position_ < input_.size() && input_[position_] != '}') {
      if (!first) {
        if (!Consume(',')) return false;
        Whitespace();
      }
      first = false;
      if (!String()) return false;
      Whitespace();
      if (!Consume(':')) return false;
      Whitespace();
      if (!AnyValue()) return false;
      Whitespace();
    }
    return Consume('}');
  }

  bool AnyArray() {
    if (!EnterDepth()) return false;
    DepthScope depth_scope(&depth_);
    if (!Consume('[')) return false;
    Whitespace();
    bool first = true;
    while (position_ < input_.size() && input_[position_] != ']') {
      if (!first) {
        if (!Consume(',')) return false;
        Whitespace();
      }
      first = false;
      if (!AnyValue()) return false;
      Whitespace();
    }
    return Consume(']');
  }

  bool AnyValue() {
    Whitespace();
    if (position_ >= input_.size()) return Fail("expected JSON value");
    switch (input_[position_]) {
      case '{':
        return AnyObject();
      case '[':
        return AnyArray();
      case '"':
        return String();
      default:
        return Scalar();
    }
  }

  bool Value(const flatbuffers::Type& type) {
    Whitespace();
    if (position_ >= input_.size()) return Fail("expected JSON value");

    if ((type.base_type == flatbuffers::BASE_TYPE_LONG ||
         type.base_type == flatbuffers::BASE_TYPE_ULONG) &&
        type.enum_def == nullptr) {
      return Integer64(type);
    }

    switch (type.base_type) {
      case flatbuffers::BASE_TYPE_STRUCT:
        if (type.struct_def == nullptr) return Fail("schema struct has no definition");
        return Object(*type.struct_def);
      case flatbuffers::BASE_TYPE_VECTOR:
      case flatbuffers::BASE_TYPE_VECTOR64:
      case flatbuffers::BASE_TYPE_ARRAY:
        return Array(type.VectorType());
      case flatbuffers::BASE_TYPE_UNION:
        return AnyValue();
      default:
        return AnyValue();
    }
  }

  const flatbuffers::Parser& parser_;
  std::string_view input_;
  IntegerJsonMode mode_;
  std::string* output_;
  std::string* error_;
  size_t position_ = 0;
  size_t depth_ = 0;
};

bool canonicalize_integer_json(const flatbuffers::Parser& parser,
                               std::string_view input, IntegerJsonMode mode,
                               std::string* output, std::string* error) {
  output->clear();
  output->reserve(input.size() + 64);
  IntegerJsonCanonicalizer canonicalizer(parser, input, mode, output, error);
  return canonicalizer.Run();
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
  std::string canonical_input;
  if (!canonicalize_integer_json(
          parser, text, IntegerJsonMode::RequireDecimalStrings,
          &canonical_input, &error)) {
    return fail(2, error, error_ptr, error_len);
  }
  if (!parser.ParseJson(canonical_input.c_str())) {
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

  std::string canonical_text;
  if (!canonicalize_integer_json(
          parser, text, IntegerJsonMode::QuoteDecimalNumbers,
          &canonical_text, &error)) {
    return fail(3, error, error_ptr, error_len);
  }

  auto* memory = static_cast<uint8_t*>(std::malloc(canonical_text.size()));
  if (memory == nullptr) {
    return fail(4, "could not allocate JSON output", error_ptr, error_len);
  }
  std::memcpy(memory, canonical_text.data(), canonical_text.size());
  *out_ptr = memory;
  *out_len = canonical_text.size();
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
