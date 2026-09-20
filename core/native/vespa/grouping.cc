// Search Kernel native adapter: grouping lowering and result decoding.
// This file is included by abi.cc so the private adapter remains one native
// boundary while responsibilities stay physically searchable by subsystem.

std::unique_ptr<sk_grouping_result_value> grouping_result_value(
    const search::expression::ResultNode &node) {
    auto value = std::make_unique<sk_grouping_result_value>();
    if (node.getClass().inherits(StringResultNode::classId)) {
        HoldString string(node);
        value->kind = SK_GROUPING_RESULT_VALUE_STRING;
        value->string = std::string(string);
    } else if (node.getClass().inherits(Int64ResultNode::classId)) {
        value->kind = SK_GROUPING_RESULT_VALUE_INTEGER;
        value->integer = static_cast<const Int64ResultNode &>(node).getInteger();
    } else if (node.getClass().inherits(FloatResultNode::classId)) {
        value->kind = SK_GROUPING_RESULT_VALUE_FLOAT;
        value->floating = static_cast<const FloatResultNode &>(node).getFloat();
    } else {
        throw UnsupportedNativeError(
            "grouping_result_value_kind",
            "Vespa grouping result value is not supported by the native ABI");
    }
    return value;
}

std::unique_ptr<sk_grouping_tree_output> grouping_tree_output(
    const AggregationResult &aggregation) {
    auto output = std::make_unique<sk_grouping_tree_output>();
    output->kind = SK_GROUPING_TREE_OUTPUT_VALUE;
    output->value = std::make_unique<sk_grouping_result_value>();
    if (aggregation.getClass().inherits(CountAggregationResult::classId)) {
        const auto count = static_cast<const CountAggregationResult &>(aggregation).getCount();
        if (count > static_cast<uint64_t>(std::numeric_limits<int64_t>::max())) {
            throw std::runtime_error("grouping count cannot be represented as int64");
        }
        output->value->kind = SK_GROUPING_RESULT_VALUE_INTEGER;
        output->value->integer = static_cast<int64_t>(count);
    } else if (aggregation.getClass().inherits(AverageAggregationResult::classId)) {
        output->value->kind = SK_GROUPING_RESULT_VALUE_FLOAT;
        output->value->floating = static_cast<const AverageAggregationResult &>(aggregation)
                                     .getAverage()
                                     .getFloat();
    } else if (aggregation.getClass().inherits(SumAggregationResult::classId) ||
               aggregation.getClass().inherits(MinAggregationResult::classId) ||
               aggregation.getClass().inherits(MaxAggregationResult::classId) ||
               aggregation.getClass().inherits(StandardDeviationAggregationResult::classId) ||
               aggregation.getClass().inherits(XorAggregationResult::classId)) {
        auto value = grouping_result_value(*aggregation.getResult());
        output->value = std::move(value);
    } else if (aggregation.getClass().inherits(QuantileAggregationResult::classId)) {
        output->value->kind = SK_GROUPING_RESULT_VALUE_QUANTILES;
        for (const auto &result : static_cast<const QuantileAggregationResult &>(aggregation).quantile_results()) {
            output->value->quantiles.emplace_back(result.quantile, result.value);
        }
    } else if (aggregation.getClass().inherits(HitsAggregationResult::classId)) {
        output->kind = SK_GROUPING_TREE_OUTPUT_HIT_LIST;
        output->value.reset();
        class HitIdVisitor final : public vespalib::ObjectVisitor {
        public:
            std::vector<std::string> ids;

            void openStruct(std::string_view, std::string_view) override {}
            void closeStruct() override {}
            void visitBool(std::string_view, bool) override {}
            void visitInt(std::string_view name, int64_t value) override {
                if (name == "docId") ids.push_back(std::to_string(value));
            }
            void visitFloat(std::string_view, double) override {}
            void visitString(std::string_view name, std::string_view value) override {
                if (name == "docId") ids.emplace_back(value);
            }
            void visitNull(std::string_view) override {}
            void visitNotImplemented() override {}
        } visitor;
        aggregation.getResult()->visitMembers(visitor);
        output->hit_ids = std::move(visitor.ids);
    } else {
        throw UnsupportedNativeError(
            "grouping_output_kind",
            "Vespa grouping output is not supported by the M9 native ABI");
    }
    return output;
}

std::unique_ptr<sk_grouping_tree_node> grouping_tree_node(
    const Group &group, bool include_key) {
    auto node = std::make_unique<sk_grouping_tree_node>();
    if (include_key) {
        node->key = grouping_result_value(group.getId());
    }
    node->outputs.reserve(group.getAggrSize());
    for (uint32_t index = 0; index < group.getAggrSize(); ++index) {
        node->outputs.push_back(grouping_tree_output(group.getAggregationResult(index)));
    }
    node->groups.reserve(group.getChildrenSize());
    std::vector<const Group *> children;
    children.reserve(group.getChildrenSize());
    for (uint32_t index = 0; index < group.getChildrenSize(); ++index) {
        children.push_back(&group.getChild(index));
    }
    // Vespa serializes the result tree in stable id order for merging. The
    // public grouping result, however, must expose the native order-by rank.
    // Reuse Vespa's own comparator here; no wrapper-side aggregation or sort
    // semantics are being reimplemented.
    // The order-by expressions live on the group prototype copied into each
    // child, while the parent owns the child array being emitted.
    const bool ordered_children =
        !children.empty() && children.front()->getOrderBySize() != 0;
    if (ordered_children) {
        std::stable_sort(children.begin(), children.end(), [](const Group *lhs, const Group *rhs) {
            return lhs->cmpRank(*rhs) < 0;
        });
    }
    auto append_group = [&node](std::unique_ptr<sk_grouping_tree_node> lowered) {
        if (lowered->key && lowered->key->kind == SK_GROUPING_RESULT_VALUE_INTEGER &&
            (lowered->key->integer == std::numeric_limits<int32_t>::min() ||
             lowered->key->integer == std::numeric_limits<int64_t>::min())) {
            return;
        }
        node->groups.push_back(std::move(lowered));
    };
    if (ordered_children) {
        for (const Group *child : children) {
            append_group(grouping_tree_node(*child, true));
        }
    } else {
        for (uint32_t index = 0; index < group.getChildrenSize(); ++index) {
            append_group(grouping_tree_node(group.getChild(index), true));
        }
    }
    return node;
}

#ifdef SK_ENABLE_HARDENING_FAILPOINTS
void attach_grouping_faults(
    sk_grouping_tree_node &node,
    const std::shared_ptr<const ResultFaultController> &faults) {
    node.result_faults = faults;
    if (node.key) {
        node.key->result_faults = faults;
    }
    for (auto &output : node.outputs) {
        output->result_faults = faults;
        if (output->value) {
            output->value->result_faults = faults;
        }
    }
    for (auto &group : node.groups) {
        attach_grouping_faults(*group, faults);
    }
}
#endif

void attach_grouping_aggregation(sk_grouping_level &level,
                                  const sk_grouping_aggregation &aggregation) {
    sk_grouping_aggregation copy;
    copy.kind = aggregation.kind;
    if (aggregation.expression) {
        copy.expression.reset(aggregation.expression->clone());
    }
    copy.quantiles = aggregation.quantiles;
    copy.summary_class = aggregation.summary_class;
    copy.max_hits = aggregation.max_hits;
    level.aggregations.push_back(std::move(copy));
}

std::unique_ptr<sk_grouping_expression> make_grouping_attribute_expression(
    const char *field) {
    validate_field_name(field);
    if (*field == '\0') {
        throw std::invalid_argument("grouping expression field must not be empty");
    }
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<AttributeNode>(field);
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_aggregation_reference_expression(
    uint32_t index) {
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<AggregationRefNode>(index);
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_literal_string_expression(
    const char *value) {
    validate_string_size(value, MAX_RANK_PROFILE_BYTES, "grouping string literal");
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<ConstantNode>(
        std::make_unique<StringResultNode>(value));
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_literal_integer_expression(
    int64_t value) {
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<ConstantNode>(
        std::make_unique<Int64ResultNode>(value));
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_literal_float_expression(
    double value) {
    if (!std::isfinite(value)) {
        throw std::invalid_argument("grouping float literal must be finite");
    }
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<ConstantNode>(
        std::make_unique<FloatResultNode>(value));
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_literal_boolean_expression(
    bool value) {
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<ConstantNode>(
        std::make_unique<BoolResultNode>(value));
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_unary_expression(
    const sk_grouping_expression &input) {
    if (!input.expression) throw std::invalid_argument("grouping expression is empty");
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::make_unique<NegateFunctionNode>(
        std::unique_ptr<search::expression::ExpressionNode>(input.expression->clone()));
    return expression;
}

std::unique_ptr<sk_grouping_expression> make_grouping_binary_expression(
    uint32_t operator_kind, const sk_grouping_expression &left,
    const sk_grouping_expression &right) {
    if (!left.expression || !right.expression) {
        throw std::invalid_argument("grouping binary expression operand is empty");
    }
    std::unique_ptr<search::expression::MultiArgFunctionNode> function;
    switch (operator_kind) {
    case SK_GROUPING_BINARY_ADD:
        function = std::make_unique<AddFunctionNode>();
        break;
    case SK_GROUPING_BINARY_SUBTRACT: {
        auto subtract = std::make_unique<AddFunctionNode>();
        subtract->appendArg(std::unique_ptr<search::expression::ExpressionNode>(left.expression->clone()));
        subtract->appendArg(std::make_unique<NegateFunctionNode>(
            std::unique_ptr<search::expression::ExpressionNode>(right.expression->clone())));
        auto expression = std::make_unique<sk_grouping_expression>();
        expression->expression = std::move(subtract);
        return expression;
    }
    case SK_GROUPING_BINARY_MULTIPLY:
        function = std::make_unique<MultiplyFunctionNode>();
        break;
    case SK_GROUPING_BINARY_DIVIDE:
        function = std::make_unique<DivideFunctionNode>();
        break;
    case SK_GROUPING_BINARY_MODULO:
        function = std::make_unique<ModuloFunctionNode>();
        break;
    default:
        throw std::invalid_argument("unknown grouping binary operator");
    }
    function->appendArg(std::unique_ptr<search::expression::ExpressionNode>(left.expression->clone()));
    function->appendArg(std::unique_ptr<search::expression::ExpressionNode>(right.expression->clone()));
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::move(function);
    return expression;
}

[[maybe_unused]] std::unique_ptr<sk_grouping_expression> make_grouping_function_expression(
    const char *name, const sk_grouping_expression *const *arguments,
    uint32_t argument_count) {
    validate_string_size(name, MAX_SCHEMA_IDENTIFIER_BYTES, "grouping function name");
    if (argument_count != 0 && !arguments) {
        throw std::invalid_argument("grouping function arguments are null");
    }
    std::unique_ptr<search::expression::MultiArgFunctionNode> function;
    if (std::strcmp(name, "cat") == 0) {
        function = std::make_unique<CatFunctionNode>();
    } else if (std::strcmp(name, "reverse") == 0) {
        function = std::make_unique<ReverseFunctionNode>();
    } else if (std::strcmp(name, "normalize") == 0) {
        function = std::make_unique<NormalizeSubjectFunctionNode>();
    } else if (std::strcmp(name, "strlen") == 0) {
        function = std::make_unique<StrLenFunctionNode>();
    } else if (std::strcmp(name, "xor") == 0) {
        function = std::make_unique<XorFunctionNode>();
    } else {
        throw UnsupportedNativeError(
            "grouping_function",
            "grouping function is not available in the pinned Vespa expression ABI");
    }
    for (uint32_t index = 0; index < argument_count; ++index) {
        if (!arguments[index] || !arguments[index]->expression) {
            throw std::invalid_argument("grouping function argument is empty");
        }
        function->appendArg(std::unique_ptr<search::expression::ExpressionNode>(
            arguments[index]->expression->clone()));
    }
    auto expression = std::make_unique<sk_grouping_expression>();
    expression->expression = std::move(function);
    return expression;
}
