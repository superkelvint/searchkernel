# frozen_string_literal: true

require_relative "client"
require_relative "grouping"
require_relative "http_transport"
require_relative "managed_local"
require_relative "query"
require_relative "results"
require_relative "schema"
require_relative "wire"

module SearchKernel
  class SearchBuilder
    attr_reader :index, :query_source

    def initialize(index, query_source)
      @index = index
      @query_source = query_source
      @filters = []
      @search_fields = []
      @sorts = []
      @fields = nil
      @summary = nil
      @highlights = nil
      @facets = []
      @groups = []
      @aggregates = []
      @ranking = nil
      @inputs = {}
      @rank_properties = {}
      @properties = {}
      @limit = nil
      @offset = nil
      @paged = false
      @timeout_ms = nil
      @presentation_format = nil
      @bolding = nil
      @presentation_timing = nil
      @query_model = {}
      @query_profile = nil
      @search_chain = nil
      @raw_groupings = []
    end

    def search_field(field, weight: nil)
      Wire.name!(field, "search field")
      item = { "field" => field }
      unless weight.nil?
        unless weight.is_a?(Numeric) && weight.finite?
          raise ArgumentError, "search-field weight must be finite"
        end
        item["weight"] = { "value" => weight.to_f }
      end
      @search_fields << item
      self
    end

    def search_fields(*fields)
      fields.flatten.each { |field| search_field(field.to_s) }
      self
    end

    def filter(source, bindings = nil)
      raise ArgumentError, "filter source must not be empty" unless source.is_a?(String) && !source.strip.empty?
      @filters << { "source" => source, "bindings" => bindings || {} }
      self
    end

    def sort(field, direction, missing: nil)
      Wire.name!(field, "sort field")
      sort = {
        "target" => {
          "target_type" => "FieldSortTarget",
          "target" => { "field" => field }
        },
        "direction" => sort_direction(direction)
      }
      sort["missing"] = missing_policy(missing) unless missing.nil?
      @sorts << sort
      self
    end

    def sort_relevance(direction = "desc")
      @sorts << {
        "target" => { "target_type" => "RelevanceSortTarget", "target" => {} },
        "direction" => sort_direction(direction)
      }
      self
    end

    def sort_feature(feature, direction = "desc")
      Wire.name!(feature, "rank feature")
      @sorts << {
        "target" => { "target_type" => "RankFeatureSortTarget", "target" => { "feature" => feature } },
        "direction" => sort_direction(direction)
      }
      self
    end

    def raw_sort(expression, direction = "asc")
      Wire.name!(expression, "raw sort expression")
      @sorts << {
        "target" => { "target_type" => "RawSortTarget", "target" => { "expression" => expression } },
        "direction" => sort_direction(direction)
      }
      self
    end

    def fields(*names)
      @fields = names.flatten.map { |name| Wire.name!(name.to_s, "projection field") }
      self
    end

    def summary(name)
      @summary = Wire.name!(name, "summary name")
      self
    end

    def highlight(*names)
      @highlights = names.flatten.map { |name| Wire.name!(name.to_s, "highlight field") }
      self
    end

    def format(name)
      @presentation_format = Wire.name!(name, "presentation format")
      self
    end

    def bolding(enabled)
      @bolding = !!enabled
      self
    end

    def timing(enabled)
      @presentation_timing = !!enabled
      self
    end

    def facet(field, name: nil, limit: nil, sort: nil)
      Wire.name!(field, "facet field")
      @facets << {
        field: field,
        name: name || field,
        limit: limit.nil? ? 10 : Wire.uint32!(limit, "facet limit"),
        sort: sort
      }
      self
    end

    def facets(*fields)
      fields.flatten.each { |field| facet(field.to_s) }
      self
    end

    def group(group)
      raise ArgumentError, "group expects a Group" unless group.is_a?(Group)
      @groups << group
      self
    end

    def raw_grouping(expression)
      Wire.name!(expression, "raw grouping expression")
      @raw_groupings << {
        "source_type" => "RawGroupingSource",
        "source" => { "expression" => expression }
      }
      self
    end

    def aggregate(expression)
      output = expression.is_a?(GroupingOutput) ? expression : Grouping.expression(expression).as("")
      @aggregates << output
      self
    end

    def ranking(profile)
      @ranking = Wire.name!(profile, "ranking profile")
      self
    end

    def input(name, item)
      Wire.name!(name, "query input name")
      @inputs[name] = { type: "LiteralQueryInput", value: item }
      self
    end

    def input_expr(name, expression)
      Wire.name!(name, "query input name")
      Wire.name!(expression, "query input expression")
      @inputs[name] = { type: "ExpressionQueryInput", value: expression }
      self
    end

    def rank_property(name, item)
      Wire.name!(name, "rank property name")
      raise TypeError, "rank property value must be a string" unless item.is_a?(String)
      @rank_properties[name] = item
      self
    end

    def rank_properties(properties)
      normalized = properties.map do |name, value|
        normalized_name = name.to_s
        Wire.name!(normalized_name, "rank property name")
        raise TypeError, "rank property value must be a string" unless value.is_a?(String)
        [normalized_name, value]
      end
      normalized.each { |name, value| @rank_properties[name] = value }
      self
    end

    def property(name, item)
      Wire.name!(name, "property name")
      @properties[name] = item
      self
    end

    def properties(properties)
      properties.each { |name, value| property(name.to_s, value) }
      self
    end

    def language(value) = query_model_value("language", value)
    def locale(value) = query_model_value("locale", value)
    def default_index(value) = query_model_value("default_index", value)
    def sources(*values) = query_model_value("sources", values.flatten.map(&:to_s))
    def restrict(*values) = query_model_value("restrict", values.flatten.map(&:to_s))
    def search_path(value) = query_model_value("search_path", value)

    def search_group(value)
      @query_model["search_group"] = { "value" => Wire.uint32!(value, "search group") }
      self
    end

    def query_string(value) = query_model_value("query_string", value)

    def query_model_type(composite: "", tokenization: "", syntax: "", profile: "", yql_default: nil)
      type = {
        "composite" => composite.to_s,
        "tokenization" => tokenization.to_s,
        "syntax" => syntax.to_s,
        "profile" => profile.to_s
      }
      type["yql_default"] = { "value" => !!yql_default } unless yql_default.nil?
      @query_model["type"] = type
      self
    end

    def query_profile(value)
      @query_profile = Wire.name!(value, "query profile")
      self
    end

    def search_chain(value)
      @search_chain = Wire.name!(value, "search chain")
      self
    end

    def limit(item)
      value = Wire.uint32!(item, "limit")
      raise ArgumentError, "limit cannot follow page()" if @paged
      @limit = value
      self
    end

    def offset(item)
      value = Wire.uint32!(item, "offset")
      raise ArgumentError, "offset cannot follow page()" if @paged
      @offset = value
      self
    end

    def page(number, size)
      number = Wire.uint32!(number, "page number")
      size = Wire.uint32!(size, "page size")
      raise ArgumentError, "invalid page() combination" if number.zero? || !@limit.nil? || !@offset.nil?
      computed = (number - 1) * size
      raise ArgumentError, "page range exceeds uint32" if number * size > 0xffff_ffff
      @paged = true
      @offset = computed
      @limit = size
      self
    end

    def timeout(milliseconds)
      @timeout_ms = Wire.uint32!(milliseconds, "timeout")
      self
    end

    def request
      result = { "query" => @query_source }
      result["search_fields"] = { "fields" => @search_fields } unless @search_fields.empty?
      unless @filters.empty?
        result["filters"] = {
          "textual" => @filters.map do |clause|
            { "source" => clause.fetch("source"), "bindings" => Wire.value_entries(clause.fetch("bindings")) }
          end
        }
      end
      result["sort"] = @sorts.dup unless @sorts.empty?

      grouping = @groups.map(&:to_request)
      @facets.each do |facet|
        grouping << Group.facet_request(
          facet.fetch(:field),
          facet.fetch(:name),
          facet.fetch(:limit),
          facet.fetch(:sort)
        )
      end
      grouping << Group.root_aggregation_request(@aggregates) unless @aggregates.empty?
      grouping.concat(@raw_groupings)
      result["grouping"] = grouping unless grouping.empty?

      if @ranking || !@rank_properties.empty?
        result["ranking"] = { "properties" => Wire.string_entries(@rank_properties) }
        result["ranking"]["profile"] = @ranking unless @ranking.nil?
      end

      unless @inputs.empty?
        result["inputs"] = @inputs.keys.sort.map do |name|
          item = @inputs.fetch(name)
          payload = if item.fetch(:type) == "ExpressionQueryInput"
            { "expression" => item.fetch(:value) }
          else
            { "value" => Value.encode(item.fetch(:value)) }
          end
          { "name" => name, "input_type" => item.fetch(:type), "input" => payload }
        end
      end

      result["limit"] = { "value" => @limit } unless @limit.nil?
      result["offset"] = { "value" => @offset } unless @offset.nil?
      result["timeout_ms"] = { "value" => @timeout_ms } unless @timeout_ms.nil?

      presentation = {}
      presentation["summary"] = @summary if @summary
      presentation["fields"] = { "fields" => @fields } unless @fields.nil?
      presentation["highlighting"] = { "fields" => @highlights } unless @highlights.nil?
      presentation["format"] = @presentation_format if @presentation_format
      presentation["bolding"] = { "value" => @bolding } unless @bolding.nil?
      presentation["timing"] = { "value" => @presentation_timing } unless @presentation_timing.nil?
      result["presentation"] = presentation unless presentation.empty?

      result["query_model"] = @query_model.dup unless @query_model.empty?
      result["query_profile"] = @query_profile if @query_profile
      result["search_chain"] = @search_chain if @search_chain
      result["properties"] = Wire.value_entries(@properties) unless @properties.empty?
      result
    end

    def execute
      results = @index.execute_search_request(request)
      @facets.each do |facet|
        results = results.project_facet(
          facet.fetch(:name),
          facet.fetch(:name),
          facet.fetch(:field)
        )
      end
      results
    end

    private

    def sort_direction(direction)
      { "asc" => "Asc", "desc" => "Desc" }.fetch(direction.to_s.downcase) do
        raise ArgumentError, "sort direction must be asc or desc"
      end
    end

    def missing_policy(missing)
      case missing
      when "first", :first
        { "policy_type" => "MissingFirst", "policy" => {} }
      when "last", :last
        { "policy_type" => "MissingLast", "policy" => {} }
      else
        { "policy_type" => "MissingAs", "policy" => { "value" => Value.encode(missing) } }
      end
    end

    def query_model_value(key, value)
      @query_model[key] = value
      self
    end

    public

    def inspect
      @index.inspect_search_request(request)
    end
  end

  class YqlBuilder < SearchBuilder
    def initialize(index, source)
      raise ArgumentError, "YQL source must not be empty" unless source.is_a?(String) && !source.strip.empty?
      super(index, { "source_type" => "YqlQuery", "source" => { "source" => source } })
    end
  end

  class PatchBuilder
    def initialize(index, identifier)
      @index = index
      @identifier = identifier
      @operations = []
    end

    def set(field, item)
      Wire.name!(field, "patch field")
      @operations << {
        "operation_type" => "SetField",
        "operation" => { "field" => field, "value" => Value.encode(item) }
      }
      self
    end

    def unset(field)
      Wire.name!(field, "patch field")
      @operations << {
        "operation_type" => "UnsetField",
        "operation" => { "field" => field }
      }
      self
    end

    def execute
      @index.execute_patch(@identifier, @operations)
    end
  end

  class UpdateBuilder
    def initialize(index, identifier)
      @index = index
      @identifier = identifier
      @operations = []
      @condition = nil
      @create = nil
    end

    def set(field, item)
      operation(field, "AssignUpdate", "value" => Value.encode(item))
    end
    alias assign set

    def unset(field)
      operation(field, "ClearUpdate", {})
    end
    alias clear unset

    def add(field, item)
      operation(field, "AddUpdate", "value" => Value.encode(item))
    end

    def remove(field, item)
      operation(field, "RemoveUpdate", "value" => Value.encode(item))
    end

    def increment(field, item) = arithmetic(field, "Increment", item)
    def decrement(field, item) = arithmetic(field, "Decrement", item)
    def multiply(field, item) = arithmetic(field, "Multiply", item)
    def divide(field, item) = arithmetic(field, "Divide", item)

    def condition(expression)
      @condition = Wire.name!(expression, "update condition")
      self
    end

    def create(enabled = true)
      @create = !!enabled
      self
    end

    def execute
      @index.execute_update(@identifier, @operations, @condition, @create)
    end

    private

    def operation(field, kind, payload)
      Wire.name!(field, "update field path")
      @operations << {
        "target" => { "path" => field },
        "operation_type" => kind,
        "operation" => payload
      }
      self
    end

    def arithmetic(field, operator, item)
      operation(
        field,
        "ArithmeticUpdate",
        "operator" => operator,
        "operand" => Value.encode(item)
      )
    end
  end

  class Index
    attr_reader :client

    def self.connect(base_url, target:, timeout: 30.0)
      client = Client.new(HttpTransport.new(base_url, timeout: timeout), target: target)
      payload = client.open_index({})
      raise ProtocolError, "OpenIndexResult.index must be an object" unless payload["index"].is_a?(Hash)
      client.bind_generation(payload["generation"])
      new(client)
    rescue StandardError
      client&.close
      raise
    end

    def self.create(location, fields:, rank_profiles: nil, target: nil, timeout: 30.0)
      definition = Schema.index_definition(fields, rank_profiles)
      if target
        client = Client.new(HttpTransport.new(location, timeout: timeout), target: target)
        payload = client.create_index("definition" => definition)
        index = payload["index"]
        raise ProtocolError, "CreateIndexResult.index must be an object" unless index.is_a?(Hash)
        client.bind_generation(payload["generation"])
        return new(client)
      end

      index_path, data_root, local_target = ManagedLocalHost.location(location)
      ManagedLocalHost.prepare_data_root(data_root)
      reserved = false
      ManagedLocalHost.reserve_path(index_path, local_target)
      reserved = true
      owner = ManagedLocalHost.acquire(data_root)
      client = Client.new(
        HttpTransport.new(owner.base_url, timeout: timeout),
        target: local_target
      )
      payload = client.create_index("definition" => definition)
      index = payload["index"]
      raise ProtocolError, "CreateIndexResult.index must be an object" unless index.is_a?(Hash)
      client.bind_generation(payload["generation"])
      new(client, owner: owner)
    rescue StandardError
      client&.close
      owner&.close
      ManagedLocalHost.cleanup_reservation(index_path, local_target) if reserved
      raise
    end

    def self.open(location, target: nil, timeout: 30.0)
      if target
        client = Client.new(HttpTransport.new(location, timeout: timeout), target: target)
        payload = client.open_index({})
        index = payload["index"]
        raise ProtocolError, "OpenIndexResult.index must be an object" unless index.is_a?(Hash)
        client.bind_generation(payload["generation"])
        return new(client)
      end

      index_path, data_root, local_target = ManagedLocalHost.location(location)
      ManagedLocalHost.prepare_data_root(data_root)
      ManagedLocalHost.migrate_legacy(index_path, data_root, local_target)
      ManagedLocalHost.ensure_marker(index_path, local_target) if
        File.directory?(File.join(data_root, local_target)) && !File.exist?(index_path)
      owner = ManagedLocalHost.acquire(data_root)
      client = Client.new(
        HttpTransport.new(owner.base_url, timeout: timeout),
        target: local_target
      )
      payload = client.open_index({})
      index = payload["index"]
      raise ProtocolError, "OpenIndexResult.index must be an object" unless index.is_a?(Hash)
      client.bind_generation(payload["generation"])
      new(client, owner: owner)
    rescue StandardError
      client&.close
      owner&.close
      raise
    end

    def self.delete(base_url, target:, timeout: 30.0)
      client = Client.new(HttpTransport.new(base_url, timeout: timeout), target: target)
      client.delete_index({})
      true
    ensure
      client&.close
    end

    def initialize(client, owner: nil)
      @client = client
      @client.require_open_before_operations unless @client.target.nil?
      @owner = owner
      @closed = false
    end

    def insert(identifier, fields = nil)
      document = document_argument(identifier, fields)
      payload = @client.insert("document" => Wire.document(document))
      Wire.result_id(payload, "InsertResult")
    end

    def upsert(identifier, fields = nil, condition: nil)
      document = document_argument(identifier, fields)
      request = { "document" => Wire.document(document) }
      request["condition"] = condition unless condition.nil?
      payload = @client.upsert(request)
      Wire.result_id(payload, "UpsertResult")
    end

    def get(identifier, projection = nil)
      Wire.name!(identifier, "document id")
      request = { "id" => identifier }
      request["projection"] = { "fields" => Array(projection).map(&:to_s) } unless projection.nil?
      payload = @client.get(request)
      document = payload["document"]
      return nil if document.nil?
      Wire.decode_document(document)
    end

    def delete(identifier, condition: nil)
      Wire.name!(identifier, "document id")
      request = { "id" => identifier }
      request["condition"] = condition unless condition.nil?
      payload = @client.delete(request)
      deleted = payload.fetch("deleted", false)
      raise ProtocolError, "DeleteResult.deleted must be boolean" unless deleted == true || deleted == false
      deleted
    end

    def search(text)
      raise ArgumentError, "search text must not be empty; use browse for match-all" unless text.is_a?(String) && !text.strip.empty?
      SearchBuilder.new(
        self,
        { "source_type" => "SearchKernelTextQuery", "source" => { "text" => text } }
      )
    end

    def browse
      SearchBuilder.new(self, { "source_type" => "MatchAllQuery", "source" => {} })
    end

    def query(query_source)
      unless query_source.is_a?(Hash) && query_source["source_type"] == "TypedQuery"
        raise ArgumentError, "query expects a Query helper result"
      end
      SearchBuilder.new(self, query_source)
    end

    def yql(source)
      YqlBuilder.new(self, source)
    end

    def patch(identifier)
      Wire.name!(identifier, "document id")
      PatchBuilder.new(self, identifier)
    end

    def update(identifier)
      Wire.name!(identifier, "document id")
      UpdateBuilder.new(self, identifier)
    end

    def analyze(text, language: nil, field: nil)
      raise ArgumentError, "analysis text must be a string" unless text.is_a?(String)
      request = { "text" => text }
      request["language"] = language unless language.nil?
      unless field.nil?
        request["target_type"] = "AnalyzeFieldTarget"
        request["target"] = { "field" => Wire.name!(field, "analysis field") }
      end
      payload = @client.analyze(request)
      tokens = payload.fetch("tokens", [])
      raise ProtocolError, "AnalyzeResult.tokens must be a list" unless tokens.is_a?(Array)
      tokens.map do |token|
        raise ProtocolError, "malformed AnalyzeResult token" unless token.is_a?(Hash)
        surface = token["surface"]
        term = token["term"]
        raise ProtocolError, "malformed AnalyzeResult token" unless surface.is_a?(String) && term.is_a?(String)
        {
          "surface" => surface,
          "term" => term,
          "position" => response_uint32(token.fetch("position", 0), "token position"),
          "start_byte" => response_uint32(token.fetch("start_byte", 0), "token start_byte"),
          "end_byte" => response_uint32(token.fetch("end_byte", 0), "token end_byte")
        }
      end
    end

    def describe
      payload = @client.describe({})
      index = payload["index"]
      raise ProtocolError, "DescribeResult.index must be an object" unless index.is_a?(Hash)
      index
    end

    def inspect_query(request)
      payload = @client.inspect_query("request" => request)
      resolved = payload["resolved_request"]
      raise ProtocolError, "InspectQueryResult.resolved_request must be an object" unless resolved.is_a?(Hash)
      resolved
    end

    def query_diagnostics(request)
      @client.query_diagnostics(request)
    end

    def close
      return if @closed

      @closed = true
      first_error = nil
      begin
        @client.close
      rescue StandardError => error
        first_error = error
      ensure
        begin
          @owner&.close
        rescue StandardError => error
          first_error ||= error
        ensure
          @owner = nil
        end
      end

      raise first_error if first_error

      nil
    end

    def closed?
      @closed
    end

    def execute_search_request(request)
      Results::SearchResults.new(@client.search(request))
    end

    def inspect_search_request(request)
      inspect_query(request)
    end

    def execute_patch(identifier, operations)
      payload = @client.patch("id" => identifier, "operations" => operations)
      Wire.result_id(payload, "PatchResult")
    end

    def execute_update(identifier, operations, condition, create)
      request = { "id" => identifier, "operations" => operations }
      request["condition"] = condition unless condition.nil?
      request["create"] = { "value" => create } unless create.nil?
      payload = @client.update(request)
      Wire.result_id(payload, "UpdateResult")
    end

    private

    def response_uint32(value, label)
      unless value.is_a?(::Integer) && value.between?(0, 0xffff_ffff)
        raise ProtocolError, "#{label} must be a uint32"
      end
      value
    end

    def document_argument(identifier, fields)
      if identifier.is_a?(Hash)
        raise ArgumentError, "fields cannot accompany an _id document" unless fields.nil?
        return identifier
      end
      Wire.name!(identifier, "document id")
      raise ArgumentError, "fields must be a Hash" unless fields.is_a?(Hash)
      { "_id" => identifier, **fields }
    end
  end
end
