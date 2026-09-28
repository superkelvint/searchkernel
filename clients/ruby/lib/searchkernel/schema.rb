# frozen_string_literal: true

require_relative "wire"

module SearchKernel
  class FieldSpec
    PRIMITIVE = {
      string: "String",
      text: "String",
      keyword: "String",
      int64: "Int64",
      float64: "Float64",
      bool: "Bool"
    }.freeze

    def initialize(kind:, matching: nil, stored: nil, searchable: nil, filterable: nil, sortable: nil, facetable: nil)
      @kind = kind
      @matching = matching
      @stored = stored
      @searchable = searchable
      @filterable = filterable
      @sortable = sortable
      @facetable = facetable
    end

    def definition(name)
      Wire.name!(name, "field name")
      primitive = PRIMITIVE.fetch(@kind)
      result = {
        "name" => name,
        "data_type" => {
          "kind_type" => "PrimitiveDataType",
          "kind" => { "kind" => primitive }
        }
      }
      {
        "stored" => @stored,
        "searchable" => @searchable,
        "filterable" => @filterable,
        "sortable" => @sortable,
        "facetable" => @facetable
      }.each do |key, value|
        result[key] = Wire.optional_bool(value) unless value.nil?
      end
      result["matching"] = { "mode" => @matching } if @matching
      result
    end
  end

  class Text < FieldSpec
    def initialize(**kwargs)
      super(kind: :text, matching: "Text", **kwargs)
    end
  end

  class Keyword < FieldSpec
    def initialize(**kwargs)
      super(kind: :keyword, matching: "Exact", **kwargs)
    end
  end

  class IntegerField < FieldSpec
    def initialize(**kwargs)
      super(kind: :int64, **kwargs)
    end
  end

  class FloatField < FieldSpec
    def initialize(**kwargs)
      super(kind: :float64, **kwargs)
    end
  end

  class BooleanField < FieldSpec
    def initialize(**kwargs)
      super(kind: :bool, **kwargs)
    end
  end

  class PositionField < FieldSpec
    def initialize(**kwargs)
      super(kind: :string, **kwargs)
    end

    def definition(name)
      result = super
      result["data_type"] = { "kind_type" => "PositionDataType", "kind" => {} }
      result.delete("matching")
      result
    end
  end

  class StructField < FieldSpec
    def initialize(fields, **kwargs)
      raise ArgumentError, "struct fields must be a non-empty Hash" unless fields.is_a?(Hash) && !fields.empty?
      @fields = fields.dup
      super(kind: :string, **kwargs)
    end

    def definition(name)
      result = super
      result["data_type"] = {
        "kind_type" => "StructDataType",
        "kind" => {
          "fields" => @fields.keys.sort_by(&:to_s).map do |raw_name|
            field_name = raw_name.to_s
            specification = @fields.fetch(raw_name)
            unless specification.respond_to?(:definition)
              raise ArgumentError, "unsupported struct field specification for #{field_name.inspect}"
            end
            child = specification.definition(field_name)
            { "name" => field_name, "data_type" => child.fetch("data_type") }
          end
        }
      }
      result.delete("matching")
      result
    end
  end

  class ArrayField < FieldSpec
    def initialize(element, **kwargs)
      raise ArgumentError, "array element must be a field specification" unless element.respond_to?(:definition)
      @element = element
      super(kind: :string, **kwargs)
    end

    def definition(name)
      result = super
      element = @element.definition("element")
      result["data_type"] = {
        "kind_type" => "ArrayDataType",
        "kind" => { "element_type" => element.fetch("data_type") }
      }
      result.delete("matching")
      result
    end
  end

  HnswIndexConfig = Struct.new(
    :max_links_per_node,
    :neighbors_to_explore_at_insert,
    keyword_init: true
  ) do
    def definition
      result = {}
      unless max_links_per_node.nil?
        result["max_links_per_node"] =
          SearchKernel::Wire.optional_uint32(max_links_per_node, "max_links_per_node")
      end
      unless neighbors_to_explore_at_insert.nil?
        result["neighbors_to_explore_at_insert"] =
          SearchKernel::Wire.optional_uint32(neighbors_to_explore_at_insert, "neighbors_to_explore_at_insert")
      end
      result
    end
  end

  class TensorField < FieldSpec
    DISTANCE_METRICS = {
      euclidean: "Euclidean",
      angular: "Angular",
      dot_product: "DotProduct",
      dotproduct: "DotProduct",
      prenormalized_angular: "PrenormalizedAngular",
      geo_degrees: "GeoDegrees",
      geodegrees: "GeoDegrees",
      hamming: "Hamming"
    }.freeze

    def initialize(type_spec, distance_metric: nil, hnsw: nil, **kwargs)
      super(kind: :string, **kwargs)
      raise ArgumentError, "tensor type spec must be a non-empty string" unless type_spec.is_a?(String) && !type_spec.empty?
      @type_spec = type_spec
      @distance_metric = distance_metric
      @hnsw = hnsw
    end

    def definition(name)
      Wire.name!(name, "field name")
      result = {
        "name" => name,
        "data_type" => {
          "kind_type" => "TensorDataType",
          "kind" => { "type_spec" => @type_spec }
        }
      }
      {
        "stored" => @stored,
        "searchable" => @searchable,
        "filterable" => @filterable,
        "sortable" => @sortable,
        "facetable" => @facetable
      }.each do |key, value|
        result[key] = Wire.optional_bool(value) unless value.nil?
      end

      metric = if @distance_metric
        DISTANCE_METRICS.fetch(@distance_metric.to_s.downcase.to_sym) do
          raise ArgumentError, "unsupported tensor distance metric: #{@distance_metric}"
        end
      end
      if @hnsw
        raise ArgumentError, "hnsw must be an HnswIndexConfig" unless @hnsw.respond_to?(:definition)
        result["index"] = {
          "enabled" => { "value" => true },
          "hnsw" => @hnsw.definition
        }
      end
      if metric || @hnsw
        result["attribute"] = { "enabled" => { "value" => true } }
        result["attribute"]["distance_metric"] = metric if metric
      end
      result
    end
  end

  class RankProfile
    def initialize(name)
      Wire.name!(name, "rank profile name")
      @name = name
      @first_phase = nil
      @second_phase = nil
      @rerank_count = nil
      @match_features = []
      @query_inputs = {}
    end

    def first_phase(expression)
      Wire.name!(expression, "first-phase expression")
      @first_phase = expression
      self
    end

    def second_phase(expression)
      Wire.name!(expression, "second-phase expression")
      @second_phase = expression
      self
    end

    def rerank_count(count)
      @rerank_count = Wire.uint32!(count, "rerank count")
      self
    end

    def match_feature(expression)
      Wire.name!(expression, "match feature")
      @match_features << expression
      self
    end

    def query_input(name, type_spec)
      Wire.name!(name, "query input name")
      Wire.name!(type_spec, "query input type")
      @query_inputs[name] = type_spec
      self
    end

    def definition
      result = { "name" => @name }
      unless @query_inputs.empty?
        result["inputs"] = @query_inputs.keys.sort.map do |name|
          type_spec = @query_inputs.fetch(name)
          {
            "name" => name,
            "data_type" => {
              "kind_type" => "TensorDataType",
              "kind" => { "type_spec" => type_spec }
            },
            "default_value" => Value.encode(Tensor.dense(type_spec, []))
          }
        end
      end
      result["first_phase"] = { "expression" => @first_phase } if @first_phase
      if @second_phase || !@rerank_count.nil?
        result["second_phase"] = {}
        result["second_phase"]["expression"] = @second_phase if @second_phase
        result["second_phase"]["rerank_count"] = { "value" => @rerank_count } unless @rerank_count.nil?
      end
      result["match_features"] = @match_features.dup unless @match_features.empty?
      result
    end
  end

  module Schema
    module_function

    def index_definition(fields, rank_profiles = nil)
      raise ArgumentError, "fields must be a Hash" unless fields.is_a?(Hash)
      definitions = fields.keys.sort_by(&:to_s).map do |raw_name|
        name = raw_name.to_s
        specification = fields.fetch(raw_name)
        unless specification.respond_to?(:definition)
          raise ArgumentError, "unsupported field specification for #{name.inspect}"
        end
        specification.definition(name)
      end
      profiles = Array(rank_profiles).map do |profile|
        unless profile.respond_to?(:definition)
          raise ArgumentError, "unsupported rank profile #{profile.inspect}"
        end
        profile.definition
      end
      result = { "fields" => definitions }
      result["rank_profiles"] = profiles unless profiles.empty?
      result
    end
  end
end
