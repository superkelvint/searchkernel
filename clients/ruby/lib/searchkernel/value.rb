# frozen_string_literal: true

require_relative "errors"

module SearchKernel
  # Standard FlatBuffers JSON representation of the canonical Value union.
  # Ruby integers are arbitrary precision; the wire range is checked explicitly.
  module Value
    PREFIX = "SearchKernel_V2_"
    INT64 = (-2**63)..(2**63 - 1)
    UINT64 = 0..(2**64 - 1)
    KINDS = %w[NullValueValue BoolValue Int64Value UInt64Value Float64Value NonFiniteFloatValue StringValue BytesValue ValueList ValueObject TensorValue].freeze
    NON_FINITE = { "NaN" => Float::NAN, "PositiveInfinity" => Float::INFINITY, "NegativeInfinity" => -Float::INFINITY }.freeze

    UInt = Struct.new(:value)
    Bytes = Struct.new(:value)
    Tensor = Struct.new(:type_spec, :encoding, :payload)
    module_function

    def uint64(integer)
      check_integer(integer, UINT64)
      UInt.new(integer).freeze
    end

    def bytes(value)
      raise ArgumentError, "bytes must be a byte string" unless value.is_a?(String)
      Bytes.new(value.b.dup.freeze).freeze
    end

    def tensor(type_spec, encoding, payload)
      Tensor.new(type_spec, encoding, payload).freeze
    end

    def encode(item)
      kind, payload = case item
      when nil then ["NullValueValue", { "value" => "Null" }]
      when true, false then ["BoolValue", { "value" => item }]
      when UInt then ["UInt64Value", { "value" => check_integer(item.value, UINT64).to_s }]
      when Integer then ["Int64Value", { "value" => check_integer(item, INT64).to_s }]
      when Float
        raise ArgumentError, "non-finite request float" unless item.finite?
        ["Float64Value", { "value" => item }]
      when Bytes
        raise ArgumentError, "bytes must be a byte string" unless item.value.is_a?(String)
        ["BytesValue", { "value" => item.value.bytes }]
      when String then ["StringValue", { "value" => item }]
      when Array then ["ValueList", { "values" => item.map { |child| encode(child) } }]
      when Hash
        raise ArgumentError, "object keys must be strings" unless item.keys.all? { |key| key.is_a?(String) }
        ["ValueObject", { "fields" => item.sort.map { |key, child| { "key" => key, "value" => encode(child) } } }]
      when Tensor then ["TensorValue", encode_tensor(item)]
      else raise ArgumentError, "unsupported portable value: #{item.class}"
      end
      { "kind_type" => kind, "kind" => payload }
    end

    def decode(item)
      object!(item, "Value")
      kind = discriminator!(item, "kind", KINDS)
      payload = object!(item["kind"], kind)
      case kind
      when "NullValueValue"
        raise ProtocolError, "invalid NullValue" unless payload["value"] == "Null"
        nil
      when "BoolValue"
        value = payload.fetch("value", false) # FlatBuffers omits default false.
        raise ProtocolError, "invalid BoolValue" unless value == true || value == false
        value
      when "Int64Value" then decimal_integer!(payload.fetch("value", "0"), INT64)
      when "UInt64Value" then decimal_integer!(payload.fetch("value", "0"), UINT64)
      when "Float64Value" then finite!(payload.fetch("value", 0.0))
      when "NonFiniteFloatValue"
        NON_FINITE.fetch(payload["kind"]) { raise ProtocolError, "unknown non-finite float kind" }
      when "StringValue"
        string!(payload["value"], "StringValue")
      when "BytesValue"
        vector!(payload["value"], "BytesValue").map { |byte| integer!(byte, 0..255) }.pack("C*").b.freeze
      when "ValueList"
        vector!(payload["values"], "ValueList").map { |child| decode(child) }.freeze
      when "ValueObject" then entries(payload["fields"])
      when "TensorValue" then decode_tensor(payload)
      end
    end

    def entries(items)
      vector!(items, "ValueEntry").each_with_object({}) do |entry, values|
        object!(entry, "ValueEntry")
        key = string!(entry["key"], "ValueEntry key")
        raise ProtocolError, "duplicate ValueEntry key #{key.inspect}" if values.key?(key)
        values[key.freeze] = decode(entry["value"])
      end.freeze
    end

    def optional_entries(items)
      items.nil? ? nil : entries(items)
    end

    def object!(value, label)
      raise ProtocolError, "#{label} must be an object" unless value.is_a?(Hash)
      value
    end

    def vector!(value, label)
      raise ProtocolError, "#{label} must be a vector" unless value.is_a?(Array)
      value
    end

    def string!(value, label)
      raise ProtocolError, "#{label} must be a string" unless value.is_a?(String)
      value
    end

    def discriminator!(item, field, names)
      type = item["#{field}_type"]
      raise ProtocolError, "unknown or missing #{field} discriminator" unless type.is_a?(String)
      name = type
      raise ProtocolError, "unknown #{field} discriminator #{type}" unless names.include?(name)
      raise ProtocolError, "missing #{field} payload" unless item[field].is_a?(Hash)
      name
    end

    def check_integer(value, range)
      raise ArgumentError, "integer out of portable range" unless value.is_a?(Integer) && range.cover?(value)
      value
    end

    def integer!(value, range)
      raise ProtocolError, "invalid portable integer" unless value.is_a?(Integer) && range.cover?(value)
      value
    end

    def decimal_integer!(value, range)
      if value.is_a?(Integer)
        raise ProtocolError, "portable 64-bit integer out of range" unless range.cover?(value)
        return value
      end
      unless value.is_a?(String) && value.match?(/\A-?(?:0|[1-9][0-9]*)\z/)
        raise ProtocolError, "invalid portable 64-bit decimal string"
      end
      integer = Integer(value, 10)
      raise ProtocolError, "portable 64-bit integer out of range" unless range.cover?(integer)
      integer
    end

    def finite!(value)
      raise ProtocolError, "invalid finite float" unless value.is_a?(Numeric) && !value.is_a?(Complex) && value.finite?
      result = value.to_f
      raise ProtocolError, "invalid finite float" unless result.finite?
      result
    end

    def encode_tensor(tensor)
      type_spec = tensor.type_spec
      raise ArgumentError, "tensor type_spec must be a string" unless type_spec.is_a?(String) && !type_spec.empty?
      kind, payload = case tensor.encoding
      when :dense
        values = tensor_cells(tensor.payload)
        # FlatBuffers standard JSON may omit an empty vector. Doing so preserves
        # the declaration-only empty-tensor sentinel while avoiding a malformed
        # zero-length f64 vector from the schema-text JSON parser.
        ["DenseTensorValues", values.empty? ? {} : { "values" => values }]
      when :sparse then ["SparseTensorCells", { "cells" => tensor_records(tensor.payload, :cell) }]
      when :mixed then ["MixedTensorBlocks", { "blocks" => tensor_records(tensor.payload, :block) }]
      else raise ArgumentError, "unknown tensor encoding"
      end
      { "type_spec" => type_spec, "payload_type" => kind, "payload" => payload }
    end

    def tensor_cells(items)
      raise ArgumentError, "tensor cells must be an array" unless items.is_a?(Array)
      items.map do |cell|
        raise ArgumentError, "tensor cell must be finite" unless cell.is_a?(Numeric) && !cell.is_a?(Complex) && cell.finite?
        converted = cell.to_f
        raise ArgumentError, "tensor cell must be finite" unless converted.finite?
        converted
      end
    end

    def tensor_address(items)
      raise ArgumentError, "tensor address must be an array" unless items.is_a?(Array)
      seen = {}
      items.map do |entry|
        raise ArgumentError, "invalid tensor address" unless entry.is_a?(Hash) && entry["dimension"].is_a?(String) && entry["label"].is_a?(String)
        raise ArgumentError, "duplicate tensor dimension" if seen[entry["dimension"]]
        seen[entry["dimension"]] = true
        { "dimension" => entry["dimension"], "label" => entry["label"] }
      end
    end

    def tensor_records(items, kind)
      raise ArgumentError, "tensor records must be an array" unless items.is_a?(Array)
      items.map do |entry|
        raise ArgumentError, "invalid tensor record" unless entry.is_a?(Hash)
        address = tensor_address(entry["address"])
        kind == :cell ? { "address" => address, "value" => tensor_cells([entry["value"]]).first } :
          { "address" => address, "values" => tensor_cells(entry["values"]) }
      end
    end

    def decode_tensor(payload)
      type_spec = string!(payload["type_spec"], "tensor type_spec")
      raise ProtocolError, "empty tensor type_spec" if type_spec.empty?
      kind = discriminator!(payload, "payload", %w[DenseTensorValues SparseTensorCells MixedTensorBlocks])
      data = payload["payload"]
      allowed_payload_keys = {
        "DenseTensorValues" => ["values"],
        "SparseTensorCells" => ["cells"],
        "MixedTensorBlocks" => ["blocks"]
      }.fetch(kind)
      unless (data.keys - allowed_payload_keys).empty?
        raise ProtocolError, "mismatched #{kind} payload"
      end
      key, value = case kind
      when "DenseTensorValues" then ["dense", decoded_cells(data.fetch("values", []))]
      when "SparseTensorCells" then ["sparse", decoded_records(data["cells"], :cell)]
      when "MixedTensorBlocks" then ["mixed", decoded_records(data["blocks"], :block)]
      end
      { "$searchkernel_tensor" => { "type_spec" => type_spec, key => value }.freeze }.freeze
    end

    def decoded_cells(items)
      vector!(items, "tensor cells").map { |cell| finite!(cell) }.freeze
    end

    def decoded_records(items, kind)
      vector!(items, "tensor records").map do |record|
        object!(record, "tensor record")
        seen = {}
        address = vector!(record["address"], "tensor address").map do |entry|
          object!(entry, "tensor address entry")
          dimension = string!(entry["dimension"], "tensor dimension")
          raise ProtocolError, "duplicate tensor dimension" if seen[dimension]
          seen[dimension] = true
          { "dimension" => dimension, "label" => string!(entry["label"], "tensor label") }.freeze
        end.freeze
        kind == :cell ? { "address" => address, "value" => finite!(record.fetch("value", 0.0)) }.freeze :
          { "address" => address, "values" => decoded_cells(record["values"]) }.freeze
      end.freeze
    end
  end
end
