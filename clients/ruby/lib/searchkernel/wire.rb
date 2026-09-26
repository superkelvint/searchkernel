# frozen_string_literal: true

require_relative "value"
require_relative "errors"

module SearchKernel
  # Thin host-language helpers for constructing the standard FlatBuffers JSON
  # representation. This module owns Ruby ergonomics only; semantic validation
  # remains in searchkernel-ir.
  module Wire
    module_function

    def name!(value, label)
      raise ArgumentError, "#{label} must be a non-empty string" unless value.is_a?(String) && !value.empty?
      value
    end

    def uint32!(value, label)
      unless value.is_a?(Integer) && !value.is_a?(TrueClass) && !value.is_a?(FalseClass) &&
             value.between?(0, 0xffff_ffff)
        raise ArgumentError, "#{label} must be a uint32"
      end
      value
    end

    def value_entries(values)
      raise ArgumentError, "value entries must be a Hash" unless values.is_a?(Hash)

      normalized = {}
      values.each do |key, value|
        key = key.to_s if key.is_a?(Symbol)
        raise ArgumentError, "value entry keys must be strings" unless key.is_a?(String)
        raise ArgumentError, "duplicate value entry key #{key.inspect}" if normalized.key?(key)
        normalized[key] = value
      end
      normalized.keys.sort.map { |key| { "key" => key, "value" => Value.encode(normalized.fetch(key)) } }
    end

    def string_entries(values)
      raise ArgumentError, "string entries must be a Hash" unless values.is_a?(Hash)

      normalized = {}
      values.each do |key, value|
        key = key.to_s if key.is_a?(Symbol)
        raise ArgumentError, "string entry keys must be strings" unless key.is_a?(String)
        raise ArgumentError, "duplicate string entry key #{key.inspect}" if normalized.key?(key)
        normalized[key] = value.to_s
      end
      normalized.keys.sort.map { |key| { "key" => key, "value" => normalized.fetch(key) } }
    end

    def optional_bool(value)
      value.nil? ? nil : { "value" => !!value }
    end

    def optional_uint32(value, label)
      value.nil? ? nil : { "value" => uint32!(value, label) }
    end

    def document(document)
      raise ArgumentError, "document must be a Hash" unless document.is_a?(Hash)
      copy = document.dup
      identifier = copy.delete("_id")
      identifier = copy.delete(:_id) if identifier.nil?
      name!(identifier, "document _id")
      {
        "id" => identifier,
        "fields" => value_entries(copy.transform_keys { |key| key.is_a?(Symbol) ? key.to_s : key })
      }
    end

    def decode_document(raw)
      raise ProtocolError, "Document must be an object" unless raw.is_a?(Hash)
      identifier = raw["id"]
      raise ProtocolError, "Document.id must be a string" unless identifier.is_a?(String)
      fields = raw.key?("fields") ? Value.entries(raw["fields"]) : {}
      { "_id" => identifier, **fields }
    end

    def result_id(payload, label)
      raise ProtocolError, "#{label} must be an object" unless payload.is_a?(Hash)
      identifier = payload["id"]
      raise ProtocolError, "#{label}.id must be a non-empty string" unless identifier.is_a?(String) && !identifier.empty?
      identifier
    end
  end

  class Tensor
    class << self
      def dense(type_spec, values)
        Value.tensor(type_spec, :dense, values)
      end

      def sparse(type_spec, cells)
        Value.tensor(type_spec, :sparse, cells)
      end

      def mixed(type_spec, blocks)
        Value.tensor(type_spec, :mixed, blocks)
      end
    end
  end

  class << self
    def value(item)
      Value.encode(item)
    end

    def uint64(integer)
      Value.uint64(integer)
    end

    def bytes(value)
      Value.bytes(value)
    end
  end
end
