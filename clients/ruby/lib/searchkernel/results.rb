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
        @id = raw["id"]
        @relevance = Results.relevance(raw["relevance"])
        @score = @relevance
        @fields = Value.optional_entries(raw["fields"])
        @highlights = Results.vector(raw, "highlights") { |highlight| Results.highlight(highlight) }
        @source = raw["source"]
        @types = Results.vector(raw, "types") { |type| Value.string!(type, "Hit type").freeze }
        @match_features = Value.optional_entries(raw["match_features"])
        @summary_features = Value.optional_entries(raw["summary_features"])
        @rank_features = Value.optional_entries(raw["rank_features"])
        @features = @match_features
        @sort_values = Results.vector(raw, "sort_values") { |value| Value.decode(value) }
        freeze
      end
    end

    class ResultGroup
      attr_reader :id, :label, :value, :fields, :children, :groups, :hits,
                  :continuations, :source, :types, :relevance, :limits, :metadata

      def initialize(raw)
        Value.object!(raw, "ResultGroup")
        @id = raw["id"]
        @label = raw["label"]
        @value = Value.decode(raw["value"]) if raw.key?("value")
        @fields = Value.optional_entries(raw["fields"])
        @children = Results.vector(raw, "children") { |node| Results.node(node) }
        @groups = @children&.grep(ResultGroup)&.freeze
        @hits = @children&.grep(Hit)&.freeze
        @continuations = Results.vector(raw, "continuations") { |entry| Results.continuation(entry) }
        @source = raw["source"]
        @types = Results.vector(raw, "types") { |type| Value.string!(type, "group type").freeze }
        @relevance = Results.relevance(raw["relevance"])
        @limits = Results.limits(raw["limits"]) if raw.key?("limits")
        @metadata = Value.optional_entries(raw["metadata"])
        freeze
      end
    end

    class SearchResults
      attr_reader :root, :hits, :groups, :total_hits, :total_hits_relation, :coverage,
                  :timing, :errors, :issues, :trace, :metadata, :continuations, :aggregations

      def initialize(raw)
        Value.object!(raw, "SearchResult")
        @root = Results.node(raw["root"]) unless raw["root"].nil?
        @hits = (@root.is_a?(Hit) ? [@root] : @root&.hits || []).freeze
        @groups = (@root.is_a?(ResultGroup) ? @root.groups || [] : []).freeze
        if raw.key?("total_hits")
          total = Value.object!(raw["total_hits"], "TotalHits")
          @total_hits = Value.decimal_integer!(total.fetch("value", "0"), Value::UINT64)
          @total_hits_relation = total.fetch("relation", "Unspecified")
          raise ProtocolError, "unknown total hits relation" unless %w[Unspecified Exact LowerBound Approximate Unknown].include?(@total_hits_relation)
        end
        @coverage = Results.coverage(raw["coverage"]) if raw.key?("coverage")
        @timing = Results.timing(raw["timing"]) if raw.key?("timing")
        @errors = Results.vector(raw, "errors") { |error| Results.issue(error) }
        @issues = @errors
        @trace = Value.decode(raw["trace"]) if raw.key?("trace")
        @metadata = Value.optional_entries(raw["metadata"])
        @continuations = @root.is_a?(ResultGroup) ? @root.continuations : nil
        @aggregations = @root.is_a?(ResultGroup) ? @root.fields : nil
        freeze
      end
    end

    module_function

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
      case kind
      when "FiniteRelevance" then Value.finite!(raw["value"].fetch("value", 0.0))
      when "NegativeInfinityRelevance" then -Float::INFINITY
      when "PositiveInfinityRelevance" then Float::INFINITY
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
        name == "nodes" ? Value.integer!(value, 0..0xffffffff) : Value.decimal_integer!(value, Value::UINT64)
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
                   metadata: Value.optional_entries(raw["metadata"])).freeze
    end

    def degraded(raw)
      Value.object!(raw, "CoverageDegraded")
      CoverageDegraded.new(match_phase: optional(raw, "match_phase"),
                           timeout: optional(raw, "timeout"),
                           adaptive_timeout: optional(raw, "adaptive_timeout"),
                           ann_timeout: optional(raw, "ann_timeout"),
                           non_ideal_state: optional(raw, "non_ideal_state"),
                           metadata: Value.optional_entries(raw["metadata"])).freeze
    end

    def timing(raw)
      Value.object!(raw, "SearchTiming")
      SearchTiming.new(query_time_seconds: optional(raw, "query_time_seconds"),
                       search_time_seconds: optional(raw, "search_time_seconds"),
                       summary_fetch_time_seconds: optional(raw, "summary_fetch_time_seconds"),
                       metadata: Value.optional_entries(raw["metadata"])).freeze
    end

    def issue(raw)
      Value.object!(raw, "SearchResultError")
      ResultIssue.new(code: Value.decimal_integer!(raw.fetch("code", "0"), Value::INT64),
                      summary: raw["summary"],
                      message: raw["message"], source: raw["source"],
                      stack_trace: raw["stack_trace"], transient: optional(raw, "transient"),
                      metadata: Value.optional_entries(raw["metadata"])).freeze
    end
  end
end
