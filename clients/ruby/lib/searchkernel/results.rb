# frozen_string_literal: true

require_relative "value"

module SearchKernel
  module Results
    ResultLimits = Struct.new(:from, :to, keyword_init: true)
    ResultContinuation = Struct.new(:key, :value, keyword_init: true)
    CoverageDegraded = Struct.new(:match_phase, :timeout, :adaptive_timeout, :ann_timeout,
                                  :non_ideal_state, :metadata, keyword_init: true)
    Coverage = Struct.new(:coverage_percent, :documents, :full, :nodes, :results,
                          :results_full, :degraded, :metadata, keyword_init: true)
    SearchTiming = Struct.new(:query_time_seconds, :search_time_seconds,
                              :summary_fetch_time_seconds, :metadata, keyword_init: true)
    ResultIssue = Struct.new(:code, :summary, :message, :source, :stack_trace,
                             :transient, :metadata, keyword_init: true)

    class Hit
      attr_reader :id, :relevance, :score, :fields, :highlights, :source, :types,
                  :match_features, :summary_features, :rank_features, :features, :sort_values

      def initialize(raw)
        Value.object!(raw, "Hit")
        @id = raw["id"] || ""
        @relevance = Results.relevance(raw["relevance"])
        @score = @relevance
        @fields = Value.optional_entries(raw["fields"]) || {}.freeze
        @highlights = Results.vector(raw, "highlights") { |highlight| Results.highlight(highlight) } || [].freeze
        @source = raw["source"]
        @types = Results.vector(raw, "types") { |type| Value.string!(type, "Hit type").freeze } || [].freeze
        @match_features = Value.optional_entries(raw["match_features"]) || {}.freeze
        @summary_features = Value.optional_entries(raw["summary_features"]) || {}.freeze
        @rank_features = Value.optional_entries(raw["rank_features"]) || {}.freeze
        @features = @match_features
        @sort_values = Results.vector(raw, "sort_values") { |value| Value.decode(value) } || [].freeze
        freeze
      end
    end

    class ResultGroup
      attr_reader :id, :label, :value, :fields, :children, :groups, :hits,
                  :continuations, :source, :types, :relevance, :limits, :metadata, :_field_entries

      def initialize(raw)
        Value.object!(raw, "ResultGroup")
        @id = raw["id"]
        @label = raw["label"]
        @value = Value.decode(raw["value"]) if raw.key?("value")
        @_field_entries = Results.value_entries_with_kinds(raw["fields"])
        @fields = Value.optional_entries(raw["fields"]) || {}.freeze
        @children = Results.vector(raw, "children") { |node| Results.node(node) } || [].freeze
        @groups = @children.grep(ResultGroup).freeze
        @hits = @children.grep(Hit).freeze
        @continuations = Results.vector(raw, "continuations") { |entry| Results.continuation(entry) } || [].freeze
        @source = raw["source"]
        @types = Results.vector(raw, "types") { |type| Value.string!(type, "group type").freeze } || [].freeze
        @relevance = Results.relevance(raw["relevance"])
        @limits = Results.limits(raw["limits"]) if raw.key?("limits")
        @metadata = Value.optional_entries(raw["metadata"]) || {}.freeze
        freeze
      end
    end

    class SearchResults
      attr_reader :root, :hits, :groups, :total_hits, :total_hits_relation, :coverage,
                  :timing, :errors, :issues, :trace, :metadata, :continuations, :aggregations, :facets

      def initialize(raw)
        Value.object!(raw, "SearchResult")
        raise ProtocolError, "SearchResult.root is required" unless raw.key?("root") && !raw["root"].nil?

        @root = Results.node(raw["root"])
        @hits = (@root.is_a?(Hit) ? [@root] : @root.hits || []).freeze
        @groups = (@root.is_a?(ResultGroup) ? @root.groups || [] : []).freeze
        raise ProtocolError, "SearchResult.total_hits is required" unless raw.key?("total_hits") && !raw["total_hits"].nil?

        total = Value.object!(raw["total_hits"], "TotalHits")
        @total_hits = Value.decimal_integer!(total.fetch("value", "0"), Value::UINT64)
        @total_hits_relation = total.fetch("relation", "Unspecified")
        raise ProtocolError, "unknown total hits relation" unless %w[Unspecified Exact LowerBound Approximate Unknown].include?(@total_hits_relation)
        @coverage = Results.coverage(raw["coverage"]) if raw.key?("coverage")
        @timing = Results.timing(raw["timing"]) if raw.key?("timing")
        @errors = Results.vector(raw, "errors") { |error| Results.issue(error) } || [].freeze
        @issues = @errors
        @trace = Value.decode(raw["trace"]) if raw.key?("trace")
        @metadata = Value.optional_entries(raw["metadata"]) || {}.freeze
        @continuations = @root.is_a?(ResultGroup) ? @root.continuations : [].freeze
        @aggregations = @root.is_a?(ResultGroup) ? @root.fields : {}.freeze
        @facets = {}.freeze
        freeze
      end

      # Facets are a strict, request-derived projection over the canonical
      # recursive grouping tree. Generic response groups are never inferred as
      # facets without the builder supplying the requested field/name context.
      #
      # SearchResults remains immutable: projecting a requested facet returns a
      # new frozen wrapper that shares the already-frozen canonical tree.
      def project_facet(expected_label, result_name, expected_field = nil)
        expected_field ||= expected_label
        candidates = @groups.select do |group|
          (group.id == expected_field || group.id.nil?) && group.label == expected_label &&
            group.value.nil? && (!group.fields || group.fields.empty?) &&
            (!group.continuations || group.continuations.empty?) && group.source.nil? &&
            (!group.types || group.types.empty?) && group.relevance.nil? && group.limits.nil? &&
            (!group.metadata || group.metadata.empty?) &&
            (group.groups || []).length == (group.children || []).length
        end
        return self unless candidates.length == 1

        buckets = []
        (candidates.first.groups || []).each do |child|
          optimized = child.id.nil? && child.label.nil?
          generic = child.id == "grouping-0" && child.label == expected_label
          return self unless optimized || generic
          return self unless child.children.nil? || child.children.empty?
          return self unless child.continuations.nil? || child.continuations.empty?
          return self unless child.source.nil? && (child.types.nil? || child.types.empty?) && child.relevance.nil?
          return self unless child.limits.nil? && (child.metadata.nil? || child.metadata.empty?)

          entries = child._field_entries || []
          return self unless entries.length == 1
          key, kind, count = entries.first
          return self unless key == "count"
          accepted_kinds = generic ? %w[Int64Value UInt64Value] : %w[UInt64Value]
          return self unless accepted_kinds.include?(kind)
          return self unless count.is_a?(Integer)
          return self if generic && count.negative?

          buckets << { "value" => child.value, "count" => count }.freeze
        end

        projected = dup
        projected.instance_variable_set(
          :@facets,
          @facets.merge(result_name => buckets.freeze).freeze
        )
        projected.freeze
      end
    end

    module_function

    def value_entries_with_kinds(items)
      return nil if items.nil?

      seen = {}
      Value.vector!(items, "ValueEntry").map do |entry|
        Value.object!(entry, "ValueEntry")
        key = Value.string!(entry["key"], "ValueEntry key")
        raise ProtocolError, "duplicate ValueEntry key #{key.inspect}" if seen.key?(key)

        raw_value = Value.object!(entry["value"], "ValueEntry value")
        kind = Value.discriminator!(raw_value, "kind", Value::KINDS)
        decoded = Value.decode(raw_value)
        seen[key] = true
        [key.freeze, kind.freeze, decoded].freeze
      end.freeze
    end

    def vector(raw, key)
      return nil unless raw.key?(key)
      Value.vector!(raw[key], key).map { |value| yield value }.freeze
    end

    def node(raw)
      Value.object!(raw, "ResultNode")
      kind = Value.discriminator!(raw, "node", %w[Hit ResultGroup])
      kind == "Hit" ? Hit.new(raw["node"]) : ResultGroup.new(raw["node"])
    end

    def relevance(raw)
      return nil if raw.nil?
      Value.object!(raw, "Relevance")
      kind = Value.discriminator!(raw, "value", %w[FiniteRelevance NegativeInfinityRelevance PositiveInfinityRelevance])
      payload = raw["value"]
      case kind
      when "FiniteRelevance"
        Value.finite!(payload.fetch("value", 0.0))
      when "NegativeInfinityRelevance"
        raise ProtocolError, "mismatched NegativeInfinityRelevance payload" if payload.key?("value")
        -Float::INFINITY
      when "PositiveInfinityRelevance"
        raise ProtocolError, "mismatched PositiveInfinityRelevance payload" if payload.key?("value")
        Float::INFINITY
      end
    end

    def highlight(raw)
      Value.object!(raw, "HighlightResult")
      {
        "field" => Value.string!(raw["field"], "highlight field"),
        "fragments" => vector(raw, "fragments") { |fragment| highlight_fragment(fragment) }
      }.freeze
    end

    def highlight_fragment(fragment)
      Value.object!(fragment, "HighlightFragment")
      { "text" => Value.string!(fragment["text"], "highlight text"),
        "matches" => vector(fragment, "matches") { |span| highlight_span(span) } }.freeze
    end

    def highlight_span(span)
      Value.object!(span, "HighlightSpan")
      [Value.integer!(span.fetch("start_byte", 0), 0..0xffffffff),
       Value.integer!(span.fetch("end_byte", 0), 0..0xffffffff)].freeze
    end

    def continuation(raw)
      Value.object!(raw, "StringEntry")
      ResultContinuation.new(key: Value.string!(raw["key"], "continuation key"),
                             value: Value.string!(raw["value"], "continuation value")).freeze
    end

    def limits(raw)
      Value.object!(raw, "ResultLimits")
      ResultLimits.new(from: raw.key?("from") ? Value.decode(raw["from"]) : nil,
                       to: raw.key?("to") ? Value.decode(raw["to"]) : nil).freeze
    end

    def optional(raw, name)
      return nil unless raw.key?(name)
      wrapper = Value.object!(raw[name], name)
      value = wrapper["value"]
      if %w[full match_phase timeout adaptive_timeout ann_timeout non_ideal_state transient].include?(name)
        value = false unless wrapper.key?("value")
        raise ProtocolError, "invalid #{name}" unless value == true || value == false
      elsif %w[documents results results_full nodes].include?(name)
        value = name == "nodes" ? 0 : "0" unless wrapper.key?("value")
        value = if name == "nodes"
                  Value.integer!(value, 0..0xffffffff)
                else
                  Value.decimal_integer!(value, Value::UINT64)
                end
      else
        value = 0.0 unless wrapper.key?("value")
        value = Value.finite!(value)
      end
      value
    end

    def coverage(raw)
      Value.object!(raw, "Coverage")
      Coverage.new(coverage_percent: optional(raw, "coverage_percent"),
                   documents: optional(raw, "documents"), full: optional(raw, "full"),
                   nodes: optional(raw, "nodes"), results: optional(raw, "results"),
                   results_full: optional(raw, "results_full"),
                   degraded: raw.key?("degraded") ? degraded(raw["degraded"]) : nil,
                   metadata: Value.optional_entries(raw["metadata"]) || {}.freeze).freeze
    end

    def degraded(raw)
      Value.object!(raw, "CoverageDegraded")
      CoverageDegraded.new(match_phase: optional(raw, "match_phase"),
                           timeout: optional(raw, "timeout"),
                           adaptive_timeout: optional(raw, "adaptive_timeout"),
                           ann_timeout: optional(raw, "ann_timeout"),
                           non_ideal_state: optional(raw, "non_ideal_state"),
                           metadata: Value.optional_entries(raw["metadata"]) || {}.freeze).freeze
    end

    def timing(raw)
      Value.object!(raw, "SearchTiming")
      SearchTiming.new(query_time_seconds: optional(raw, "query_time_seconds"),
                       search_time_seconds: optional(raw, "search_time_seconds"),
                       summary_fetch_time_seconds: optional(raw, "summary_fetch_time_seconds"),
                       metadata: Value.optional_entries(raw["metadata"]) || {}.freeze).freeze
    end

    def issue(raw)
      Value.object!(raw, "SearchResultError")
      ResultIssue.new(code: Value.decimal_integer!(raw.fetch("code", "0"), Value::INT64),
                      summary: raw["summary"],
                      message: raw["message"], source: raw["source"],
                      stack_trace: raw["stack_trace"], transient: optional(raw, "transient"),
                      metadata: Value.optional_entries(raw["metadata"]) || {}.freeze).freeze
    end
  end
end
