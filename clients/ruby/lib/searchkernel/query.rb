# frozen_string_literal: true

require "json"

require_relative "wire"

module SearchKernel
  module Query
    module_function

    def term(item)
      typed(term_expr(item))
    end

    def contains(field, item)
      Wire.name!(field, "query field")
      typed(
        "expr_type" => "QueryContains",
        "expr" => {
          "field" => { "path" => field },
          "item" => item_expr(item)
        }
      )
    end

    def all_of(*clauses)
      clauses_expr("QueryAnd", clauses)
    end

    def any_of(*clauses)
      clauses_expr("QueryOr", clauses)
    end

    def negate(clause)
      typed("expr_type" => "QueryNot", "expr" => { "clause" => expression(clause) })
    end

    def always_true
      typed("expr_type" => "QueryTrue", "expr" => {})
    end

    def always_false
      typed("expr_type" => "QueryFalse", "expr" => {})
    end

    def eq(field, item) = comparison(field, "Eq", item)
    def ne(field, item) = comparison(field, "Ne", item)
    def lt(field, item) = comparison(field, "Lt", item)
    def lte(field, item) = comparison(field, "Lte", item)
    def gt(field, item) = comparison(field, "Gt", item)
    def gte(field, item) = comparison(field, "Gte", item)

    def range(field, gte: nil, gt: nil, lte: nil, lt: nil)
      Wire.name!(field, "range field")
      raise ArgumentError, "range accepts at most one of gte and gt" unless gte.nil? || gt.nil?
      raise ArgumentError, "range accepts at most one of lte and lt" unless lte.nil? || lt.nil?

      lower = gte.nil? ? gt : gte
      upper = lte.nil? ? lt : lte
      payload = {
        "field" => { "path" => field },
        "lower" => range_bound(lower),
        "upper" => range_bound(upper)
      }
      payload["include_lower"] = { "value" => !gte.nil? } unless lower.nil?
      payload["include_upper"] = { "value" => !lte.nil? } unless upper.nil?
      typed("expr_type" => "QueryRange", "expr" => payload)
    end

    def in_values(field, values)
      Wire.name!(field, "IN field")
      raise ArgumentError, "IN query requires at least one value" unless values.is_a?(Array) && !values.empty?
      typed(
        "expr_type" => "QueryIn",
        "expr" => { "field" => { "path" => field }, "values" => values.map { |item| Value.encode(item) } }
      )
    end

    def phrase(*items) = terms("QueryPhrase", items)
    def near(*items) = terms("QueryNear", items)
    def onear(*items) = terms("QueryONear", items)
    def same_element(*clauses) = clauses_expr("QuerySameElement", clauses)
    def equiv(*clauses) = clauses_expr("QueryEquiv", clauses)

    def fuzzy(item)
      typed("expr_type" => "QueryFuzzy", "expr" => { "value" => Value.encode(item) })
    end

    def matches(field, pattern)
      Wire.name!(field, "matches field")
      Wire.name!(pattern, "matches pattern")
      typed("expr_type" => "QueryMatches", "expr" => { "field" => { "path" => field }, "pattern" => pattern })
    end

    def text(text)
      typed("expr_type" => "QueryText", "expr" => { "text" => text.to_s })
    end

    def user_input(value)
      typed("expr_type" => "QueryUserInput", "expr" => { "value" => value.to_s })
    end

    def user_query
      typed("expr_type" => "QueryUserQuery", "expr" => {})
    end

    def dot_product(field, items) = weighted("QueryDotProduct", field, items)
    def weighted_set(field, items) = weighted("QueryWeightedSet", field, items)
    def wand(field, items) = weighted("QueryWand", field, items)
    def weak_and(*clauses) = clauses_expr("QueryWeakAnd", clauses)

    def rank(match, *ranking)
      typed(
        "expr_type" => "QueryRank",
        "expr" => {
          "match" => expression(match),
          "rank" => ranking.map { |item| expression(item) }
        }
      )
    end

    def labeled(query, label, score)
      Wire.name!(label, "query label")
      unless score.is_a?(Numeric) && score.finite?
        raise ArgumentError, "query label score must be finite"
      end
      typed(
        "expr_type" => "QueryLabeled",
        "expr" => { "query" => expression(query), "label" => label, "score" => score.to_f }
      )
    end

    def non_empty(query)
      typed("expr_type" => "QueryNonEmpty", "expr" => { "query" => expression(query) })
    end

    def geo_location(field, lat:, lon:, radius:)
      Wire.name!(field, "geo field")
      Wire.name!(radius, "geo radius")
      unless [lat, lon].all? { |v| v.is_a?(Numeric) && v.finite? }
        raise ArgumentError, "geo coordinates must be finite"
      end
      typed(
        "expr_type" => "QueryGeoLocation",
        "expr" => {
          "field" => { "path" => field },
          "latitude" => lat.to_f,
          "longitude" => lon.to_f,
          "radius" => radius
        }
      )
    end

    def geo_box(field, south:, west:, north:, east:)
      Wire.name!(field, "geo field")
      values = [south, west, north, east]
      raise ArgumentError, "geo coordinates must be finite" unless values.all? { |v| v.is_a?(Numeric) && v.finite? }
      typed(
        "expr_type" => "QueryGeoBoundingBox",
        "expr" => {
          "field" => { "path" => field },
          "south" => south.to_f,
          "west" => west.to_f,
          "north" => north.to_f,
          "east" => east.to_f
        }
      )
    end

    def nearest_neighbor(document_field, query_tensor, approximate: nil, target_hits: nil,
                         total_target_hits: nil, explore_additional_hits: nil,
                         distance_threshold: nil)
      Wire.name!(document_field, "nearest-neighbor document field")
      Wire.name!(query_tensor, "nearest-neighbor query tensor")
      annotations = {}
      annotations["approximate"] = approximate unless approximate.nil?
      annotations["targetNumHits"] = target_hits unless target_hits.nil?
      annotations["totalTargetHits"] = total_target_hits unless total_target_hits.nil?
      annotations["exploreAdditionalHits"] = explore_additional_hits unless explore_additional_hits.nil?
      annotations["distanceThreshold"] = distance_threshold unless distance_threshold.nil?
      expr = {
        "expr_type" => "QueryNearestNeighbor",
        "expr" => {
          "document_field" => { "path" => document_field },
          "query_tensor" => query_tensor
        }
      }
      expr["annotations"] = Wire.value_entries(annotations) unless annotations.empty?
      typed(expr)
    end

    def predicate(field, features: {}, range_features: {})
      Wire.name!(field, "predicate field")
      typed(
        "expr_type" => "QueryPredicate",
        "expr" => {
          "field" => { "path" => field },
          "features" => { "fields" => Wire.value_entries(features) },
          "range_features" => { "fields" => Wire.value_entries(range_features) }
        }
      )
    end

    def annotate(query, annotations)
      expr = expression(query).dup
      existing = {}
      Array(expr["annotations"]).each do |entry|
        raise ArgumentError, "malformed existing query annotation" unless entry.is_a?(Hash) && entry["key"].is_a?(String)
        existing[entry["key"]] = entry
      end
      Wire.value_entries(annotations).each do |entry|
        raise ArgumentError, "duplicate query annotation: #{entry["key"]}" if existing.key?(entry["key"])
        existing[entry["key"]] = entry
      end
      expr["annotations"] = existing.keys.sort.map { |key| existing.fetch(key) }
      typed(expr)
    end

    def typed(expr)
      { "source_type" => "TypedQuery", "source" => { "query" => expr } }
    end
    private_class_method :typed

    def expression(source)
      unless source.is_a?(Hash) && source["source_type"] == "TypedQuery" &&
             source.dig("source", "query").is_a?(Hash)
        raise ArgumentError, "typed query clauses must come from Query helpers"
      end
      source.dig("source", "query")
    end
    private_class_method :expression

    def term_expr(item)
      { "expr_type" => "TermQuery", "expr" => { "value" => Value.encode(item) } }
    end
    private_class_method :term_expr

    def item_expr(item)
      item.is_a?(Hash) && item["source_type"] == "TypedQuery" ? expression(item) : term_expr(item)
    end
    private_class_method :item_expr

    def clauses_expr(kind, clauses)
      raise ArgumentError, "query operator requires at least one item" if clauses.empty?
      typed("expr_type" => kind, "expr" => { "clauses" => clauses.map { |item| expression(item) } })
    end
    private_class_method :clauses_expr

    def terms(kind, items)
      raise ArgumentError, "#{kind} requires at least one item" if items.empty?
      typed("expr_type" => kind, "expr" => { "terms" => items.map { |item| item_expr(item) } })
    end
    private_class_method :terms

    def comparison(field, operator, item)
      Wire.name!(field, "comparison field")
      typed(
        "expr_type" => "QueryComparison",
        "expr" => { "field" => { "path" => field }, "op" => operator, "value" => Value.encode(item) }
      )
    end
    private_class_method :comparison

    def range_bound(item)
      return { "bound_type" => "UnboundedRangeBound", "bound" => {} } if item.nil?
      { "bound_type" => "FiniteRangeBound", "bound" => { "value" => Value.encode(item) } }
    end
    private_class_method :range_bound

    def weighted(kind, field, items)
      Wire.name!(field, "weighted-query field")
      raise ArgumentError, "#{kind} requires at least one weighted item" unless items.is_a?(Hash) && !items.empty?
      weighted = items.map do |key, weight|
        raise ArgumentError, "weighted query weights must be integers" unless weight.is_a?(Integer)
        { "key" => Value.encode(key), "weight" => weight.to_s }
      end
      weighted.sort_by! { |entry| JSON.generate(entry.fetch("key")) }
      typed("expr_type" => kind, "expr" => { "field" => { "path" => field }, "items" => weighted })
    end
    private_class_method :weighted
  end
end
