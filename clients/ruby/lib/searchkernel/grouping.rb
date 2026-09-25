# frozen_string_literal: true

require_relative "wire"

module SearchKernel
  class GroupingExpression
    attr_reader :wire

    def initialize(wire)
      @wire = wire.freeze
      freeze
    end

    def as(label)
      GroupingOutput.new(self, label.to_s)
    end

    def -@
      GroupingExpression.new(
        "expr_type" => "GroupingUnaryExpression",
        "expr" => { "op" => "Negative", "expression" => @wire }
      )
    end

    def +(other) = binary("Add", other)
    def -(other) = binary("Subtract", other)
    def *(other) = binary("Multiply", other)
    def /(other) = binary("Divide", other)

    private

    def binary(operator, other)
      GroupingExpression.new(
        "expr_type" => "GroupingBinaryExpression",
        "expr" => {
          "op" => operator,
          "left" => @wire,
          "right" => Grouping.expression(other).wire
        }
      )
    end
  end

  GroupingOutput = Struct.new(:expression, :label)
  GroupingOrder = Struct.new(:expression)

  module Grouping
    module_function

    def expression(item)
      item.is_a?(GroupingExpression) ? item : value(item)
    end

    def field(name)
      Wire.name!(name, "grouping field")
      GroupingExpression.new(
        "expr_type" => "GroupingAttributeExpression",
        "expr" => { "field" => name }
      )
    end

    def value(item)
      GroupingExpression.new(
        "expr_type" => "GroupingLiteralExpression",
        "expr" => { "value" => Value.encode(item) }
      )
    end

    def fn(name, *arguments)
      Wire.name!(name, "grouping function")
      GroupingExpression.new(
        "expr_type" => "GroupingFunctionExpression",
        "expr" => { "name" => name, "arguments" => arguments.map { |item| expression(item).wire } }
      )
    end

    def alias_ref(name)
      Wire.name!(name, "grouping alias")
      GroupingExpression.new(
        "expr_type" => "GroupingAliasReference",
        "expr" => { "name" => name }
      )
    end

    def member(base, name)
      Wire.name!(name, "grouping member")
      GroupingExpression.new(
        "expr_type" => "GroupingMemberExpression",
        "expr" => { "base" => expression(base).wire, "member" => name }
      )
    end

    def lookup(base, key)
      GroupingExpression.new(
        "expr_type" => "GroupingMapLookupExpression",
        "expr" => { "base" => expression(base).wire, "key" => expression(key).wire }
      )
    end

    def asc(item) = GroupingOrder.new(expression(item))
    def desc(item) = GroupingOrder.new(-expression(item))
  end

  module Agg
    module_function

    def count = aggregation("CountAggregation", {})
    def sum(field) = aggregation("SumAggregation", "expression" => Grouping.field(field).wire)
    def avg(field) = aggregation("AvgAggregation", "expression" => Grouping.field(field).wire)
    def min(field) = aggregation("MinAggregation", "expression" => Grouping.field(field).wire)
    def max(field) = aggregation("MaxAggregation", "expression" => Grouping.field(field).wire)
    def stddev(field) = aggregation("StdDevAggregation", "expression" => Grouping.field(field).wire)

    def summary(name = nil)
      payload = {}
      payload["summary"] = name if name
      aggregation("SummaryAggregation", payload)
    end

    def aggregation(kind, payload)
      GroupingExpression.new(
        "expr_type" => "GroupingAggregationExpression",
        "expr" => { "aggregation_type" => kind, "aggregation" => payload }
      )
    end
    private_class_method :aggregation
  end

  module GroupingFilter
    module_function

    def regex(expression, pattern)
      {
        "expr_type" => "GroupingFilterRegex",
        "expr" => { "expression" => Grouping.expression(expression).wire, "pattern" => pattern.to_s }
      }
    end

    def range(expression, lower, upper, include_lower: true, include_upper: false)
      {
        "expr_type" => "GroupingFilterRange",
        "expr" => {
          "expression" => Grouping.expression(expression).wire,
          "lower" => Grouping.expression(lower).wire,
          "upper" => Grouping.expression(upper).wire,
          "include_lower" => { "value" => !!include_lower },
          "include_upper" => { "value" => !!include_upper }
        }
      }
    end

    def is_true(expression)
      {
        "expr_type" => "GroupingFilterIsTrue",
        "expr" => { "expression" => Grouping.expression(expression).wire }
      }
    end

    def all_of(*clauses) = combine("GroupingFilterAnd", clauses)
    def any_of(*clauses) = combine("GroupingFilterOr", clauses)

    def negate(clause)
      { "expr_type" => "GroupingFilterNot", "expr" => { "clause" => validate(clause) } }
    end

    def combine(kind, clauses)
      raise ArgumentError, "grouping filter requires at least one clause" if clauses.empty?
      { "expr_type" => kind, "expr" => { "clauses" => clauses.map { |clause| validate(clause) } } }
    end

    def validate(clause)
      unless clause.is_a?(Hash) && clause["expr_type"].is_a?(String) && clause["expr"].is_a?(Hash)
        raise ArgumentError, "grouping filter must come from GroupingFilter helpers"
      end
      clause
    end
    private_class_method :validate
    private_class_method :combine
  end

  class Group
    def self.by(field)
      new(field)
    end

    def initialize(field)
      @expression = field.is_a?(GroupingExpression) ? field : Grouping.field(field)
      @limit = nil
      @unlimited = false
      @precision = nil
      @orders = []
      @outputs = []
      @subgroups = []
      @aliases = []
      @keep = nil
      @filter = nil
      @where = nil
      @continuations = []
    end

    def limit(value)
      @limit = Wire.uint32!(value, "grouping limit")
      @unlimited = false
      self
    end

    def unlimited
      @limit = nil
      @unlimited = true
      self
    end

    def precision(value)
      @precision = Wire.uint32!(value, "grouping precision")
      self
    end

    def order_by(*orders)
      @orders.concat(orders.map { |item| item.is_a?(GroupingOrder) ? item : Grouping.asc(item) })
      self
    end

    def output(*outputs)
      @outputs.concat(outputs.map { |item| item.is_a?(GroupingOutput) ? item : item.as("") })
      self
    end

    def subgroup(group)
      raise ArgumentError, "subgroup must be a Group" unless group.is_a?(Group)
      @subgroups << group
      self
    end

    def alias(name, expression)
      Wire.name!(name, "grouping alias")
      @aliases << [name, Grouping.expression(expression)]
      self
    end

    def keep(filter)
      @keep = filter
      self
    end

    def filter(filter)
      @filter = GroupingFilter.send(:validate, filter)
      self
    end

    def keep(filter)
      @keep = GroupingFilter.send(:validate, filter)
      self
    end

    def where(expression)
      @where = Grouping.expression(expression)
      self
    end

    def continuation(token)
      Wire.name!(token, "grouping continuation")
      @continuations << token
      self
    end

    def to_request
      request = {
        "source_type" => "StructuredGroupingSource",
        "source" => { "root" => { "operations" => operations } }
      }
      request["continuations"] = @continuations.dup unless @continuations.empty?
      request
    end

    def self.facet_request(field, name, limit, ordering)
      ordering ||= "count_desc"
      order_expr = case ordering
                   when "count_desc" then -Agg.count
                   when "count_asc" then Agg.count
                   when "value_desc" then -Grouping.field(field)
                   when "value_asc" then Grouping.field(field)
                   else raise ArgumentError, "facet sort must be count_desc, count_asc, value_asc, or value_desc"
                   end
      each_ops = [
        operation("GroupingMaxOperation", {
          "value_type" => "FiniteGroupingMax",
          "value" => { "value" => Wire.uint32!(limit, "facet limit") }
        }),
        operation("GroupingOrderOperation", {
          "expressions" => [{ "expression" => order_expr.wire }]
        }),
        operation("GroupingOutputOperation", {
          "outputs" => [{ "expression" => Agg.count.wire }]
        })
      ]
      {
        "source_type" => "StructuredGroupingSource",
        "source" => {
          "root" => {
            "label" => name,
            "operations" => [
              operation("GroupingGroupOperation", { "expression" => Grouping.field(field).wire }),
              operation("GroupingEachOperation", { "operations" => each_ops })
            ]
          }
        }
      }
    end

    def self.root_aggregation_request(outputs)
      {
        "source_type" => "StructuredGroupingSource",
        "source" => {
          "root" => {
            "operations" => [
              operation("GroupingOutputOperation", {
                "outputs" => outputs.map do |output|
                  {
                    "label" => output.label.empty? ? nil : output.label,
                    "expression" => output.expression.wire
                  }.compact
                end
              })
            ]
          }
        }
      }
    end

    def self.operation(kind, payload)
      { "operation_type" => kind, "operation" => payload }
    end

    protected

    def operations
      [
        self.class.operation("GroupingGroupOperation", { "expression" => @expression.wire }),
        self.class.operation("GroupingEachOperation", { "operations" => body_operations })
      ]
    end

    def body_operations
      result = @aliases.map do |name, expression|
        self.class.operation("GroupingAliasOperation", { "name" => name, "expression" => expression.wire })
      end
      if @limit
        result << self.class.operation(
          "GroupingMaxOperation",
          { "value_type" => "FiniteGroupingMax", "value" => { "value" => @limit } }
        )
      elsif @unlimited
        result << self.class.operation(
          "GroupingMaxOperation",
          { "value_type" => "InfiniteGroupingMax", "value" => {} }
        )
      end
      if @precision
        result << self.class.operation("GroupingPrecisionOperation", { "value" => @precision })
      end
      unless @orders.empty?
        result << self.class.operation(
          "GroupingOrderOperation",
          { "expressions" => @orders.map { |order| { "expression" => order.expression.wire } } }
        )
      end
      result << self.class.operation("GroupingFilterOperation", { "filter" => @filter }) if @filter
      result << self.class.operation("GroupingKeepOperation", { "filter" => @keep }) if @keep
      result << self.class.operation("GroupingWhereOperation", { "expression" => @where.wire }) if @where
      unless @outputs.empty?
        result << self.class.operation(
          "GroupingOutputOperation",
          {
            "outputs" => @outputs.map do |output|
              item = { "expression" => output.expression.wire }
              item["label"] = output.label unless output.label.empty?
              item
            end
          }
        )
      end
      @subgroups.each do |subgroup|
        result << self.class.operation("GroupingAllOperation", { "operations" => subgroup.send(:operations) })
      end
      result
    end
  end
end
