import { AggregationKind } from "../semantic.js";
import { assertUInt32 } from "../internal/scalars.js";
import { AvgAggregationT } from "../semantic.js";
import { CountAggregationT } from "../semantic.js";
import { FiniteGroupingMaxT } from "../semantic.js";
import { GroupingAggregationExpressionT } from "../semantic.js";
import { GroupingAliasOperationT } from "../semantic.js";
import { GroupingAliasReferenceT } from "../semantic.js";
import { GroupingAllOperationT } from "../semantic.js";
import { GroupingAttributeExpressionT } from "../semantic.js";
import { GroupingBinaryExpressionT } from "../semantic.js";
import { GroupingBinaryOperator } from "../semantic.js";
import { GroupingEachOperationT } from "../semantic.js";
import { GroupingExpressionKind } from "../semantic.js";
import { GroupingExpressionT } from "../semantic.js";
import { GroupingFilterAndT } from "../semantic.js";
import { GroupingFilterExprKind } from "../semantic.js";
import { GroupingFilterExprT } from "../semantic.js";
import { GroupingFilterIsTrueT } from "../semantic.js";
import { GroupingFilterNotT } from "../semantic.js";
import { GroupingFilterOperationT } from "../semantic.js";
import { GroupingFilterOrT } from "../semantic.js";
import { GroupingFilterRegexT } from "../semantic.js";
import { GroupingFunctionExpressionT } from "../semantic.js";
import { GroupingGroupOperationT } from "../semantic.js";
import { GroupingKeepOperationT } from "../semantic.js";
import { GroupingLiteralExpressionT } from "../semantic.js";
import { GroupingMapLookupExpressionT } from "../semantic.js";
import { GroupingMaxKind } from "../semantic.js";
import { GroupingMaxOperationT } from "../semantic.js";
import { GroupingMemberExpressionT } from "../semantic.js";
import { GroupingOperationKind } from "../semantic.js";
import { GroupingOperationT } from "../semantic.js";
import { GroupingOrderExprT } from "../semantic.js";
import { GroupingOrderOperationT } from "../semantic.js";
import { GroupingOutputOperationT } from "../semantic.js";
import { GroupingOutputT } from "../semantic.js";
import { GroupingPrecisionOperationT } from "../semantic.js";
import { GroupingRequestT } from "../semantic.js";
import { GroupingSourceKind } from "../semantic.js";
import { GroupingUnaryExpressionT } from "../semantic.js";
import { GroupingUnaryOperator } from "../semantic.js";
import { MaxAggregationT } from "../semantic.js";
import { MinAggregationT } from "../semantic.js";
import { StdDevAggregationT } from "../semantic.js";
import { StructuredGroupingSourceT } from "../semantic.js";
import { SumAggregationT } from "../semantic.js";
import { SummaryAggregationT } from "../semantic.js";
import { value, type ValueInput } from "../client/values.js";

function expr(kind: GroupingExpressionKind, payload: GroupingExpressionT["expr"]): GroupingExpressionT {
  return new GroupingExpressionT(kind, payload);
}

export class GroupingExpr {
  constructor(readonly model: GroupingExpressionT) {}

  as(label: string): GroupingOutput {
    return new GroupingOutput(this, label);
  }

  as_(label: string): GroupingOutput {
    return this.as(label);
  }

  neg(): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingUnaryExpression,
        new GroupingUnaryExpressionT(GroupingUnaryOperator.Negative, this.model),
      ),
    );
  }

  add(other: GroupingInput): GroupingExpr { return binary(GroupingBinaryOperator.Add, this, other); }
  sub(other: GroupingInput): GroupingExpr { return binary(GroupingBinaryOperator.Subtract, this, other); }
  mul(other: GroupingInput): GroupingExpr { return binary(GroupingBinaryOperator.Multiply, this, other); }
  div(other: GroupingInput): GroupingExpr { return binary(GroupingBinaryOperator.Divide, this, other); }
}

export class GroupingOutput {
  constructor(readonly expression: GroupingExpr, readonly label: string) {}
}

export class GroupingOrder {
  constructor(readonly expression: GroupingExpr) {}
}

export type GroupingInput = GroupingExpr | ValueInput;

function asExpr(input: GroupingInput): GroupingExpr {
  return input instanceof GroupingExpr ? input : g.value(input);
}

function binary(operator: GroupingBinaryOperator, left: GroupingInput, right: GroupingInput): GroupingExpr {
  return new GroupingExpr(
    expr(
      GroupingExpressionKind.GroupingBinaryExpression,
      new GroupingBinaryExpressionT(operator, asExpr(left).model, asExpr(right).model),
    ),
  );
}

export const g = {
  field(name: string): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingAttributeExpression,
        new GroupingAttributeExpressionT(name),
      ),
    );
  },

  value(input: ValueInput): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingLiteralExpression,
        new GroupingLiteralExpressionT(value(input)),
      ),
    );
  },

  fn(name: string, ...arguments_: GroupingInput[]): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingFunctionExpression,
        new GroupingFunctionExpressionT(name, arguments_.map((argument) => asExpr(argument).model)),
      ),
    );
  },

  aliasRef(name: string): GroupingExpr {
    return new GroupingExpr(
      expr(GroupingExpressionKind.GroupingAliasReference, new GroupingAliasReferenceT(name)),
    );
  },

  member(base: GroupingInput, name: string): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingMemberExpression,
        new GroupingMemberExpressionT(asExpr(base).model, name),
      ),
    );
  },

  lookup(base: GroupingInput, key: GroupingInput): GroupingExpr {
    return new GroupingExpr(
      expr(
        GroupingExpressionKind.GroupingMapLookupExpression,
        new GroupingMapLookupExpressionT(asExpr(base).model, asExpr(key).model),
      ),
    );
  },
};

function aggregation(kind: AggregationKind, payload: GroupingAggregationExpressionT["aggregation"]): GroupingExpr {
  return new GroupingExpr(
    expr(
      GroupingExpressionKind.GroupingAggregationExpression,
      new GroupingAggregationExpressionT(kind, payload),
    ),
  );
}

export const agg = {
  count(): GroupingExpr {
    return aggregation(AggregationKind.CountAggregation, new CountAggregationT());
  },
  sum(field: string): GroupingExpr {
    return aggregation(AggregationKind.SumAggregation, new SumAggregationT(g.field(field).model));
  },
  avg(field: string): GroupingExpr {
    return aggregation(AggregationKind.AvgAggregation, new AvgAggregationT(g.field(field).model));
  },
  min(field: string): GroupingExpr {
    return aggregation(AggregationKind.MinAggregation, new MinAggregationT(g.field(field).model));
  },
  max(field: string): GroupingExpr {
    return aggregation(AggregationKind.MaxAggregation, new MaxAggregationT(g.field(field).model));
  },
  stddev(field: string): GroupingExpr {
    return aggregation(AggregationKind.StdDevAggregation, new StdDevAggregationT(g.field(field).model));
  },
  summary(name?: string): GroupingExpr {
    return aggregation(AggregationKind.SummaryAggregation, new SummaryAggregationT(name ?? null));
  },
};

export function asc(input: GroupingInput): GroupingOrder {
  return new GroupingOrder(asExpr(input));
}

export function desc(input: GroupingInput): GroupingOrder {
  return new GroupingOrder(asExpr(input).neg());
}

export const gf = {
  regex(input: GroupingInput, pattern: string): GroupingFilterExprT {
    return new GroupingFilterExprT(
      GroupingFilterExprKind.GroupingFilterRegex,
      new GroupingFilterRegexT(asExpr(input).model, pattern),
    );
  },

  isTrue(input: GroupingInput): GroupingFilterExprT {
    return new GroupingFilterExprT(
      GroupingFilterExprKind.GroupingFilterIsTrue,
      new GroupingFilterIsTrueT(asExpr(input).model),
    );
  },

  allOf(...clauses: GroupingFilterExprT[]): GroupingFilterExprT {
    return new GroupingFilterExprT(
      GroupingFilterExprKind.GroupingFilterAnd,
      new GroupingFilterAndT(clauses),
    );
  },

  anyOf(...clauses: GroupingFilterExprT[]): GroupingFilterExprT {
    return new GroupingFilterExprT(
      GroupingFilterExprKind.GroupingFilterOr,
      new GroupingFilterOrT(clauses),
    );
  },

  not(clause: GroupingFilterExprT): GroupingFilterExprT {
    return new GroupingFilterExprT(
      GroupingFilterExprKind.GroupingFilterNot,
      new GroupingFilterNotT(clause),
    );
  },
};

export class Group {
  readonly expression: GroupingExpr;
  private limitValue?: number;
  private precisionValue?: number;
  private readonly orders: GroupingOrder[] = [];
  private readonly outputs: GroupingOutput[] = [];
  private readonly subgroups: Group[] = [];
  private readonly aliases: Array<[string, GroupingExpr]> = [];
  private filterValue?: GroupingFilterExprT;
  private keepValue?: GroupingFilterExprT;

  constructor(field: string | GroupingExpr) {
    this.expression = typeof field === "string" ? g.field(field) : field;
  }

  static by(field: string | GroupingExpr): Group {
    return new Group(field);
  }

  limit(value_: number): this {
    this.limitValue = assertUInt32(value_, "group limit");
    return this;
  }

  unlimited(): this {
    this.limitValue = undefined;
    return this;
  }

  precision(value_: number): this {
    this.precisionValue = assertUInt32(value_, "group precision");
    return this;
  }

  orderBy(...expressions: GroupingOrder[]): this {
    this.orders.push(...expressions);
    return this;
  }

  output(...expressions: Array<GroupingOutput | GroupingExpr>): this {
    this.outputs.push(
      ...expressions.map((entry) => entry instanceof GroupingOutput ? entry : entry.as("")),
    );
    return this;
  }

  subgroup(group: Group): this {
    this.subgroups.push(group);
    return this;
  }

  alias(name: string, expression: GroupingExpr): this {
    if (!name) throw new TypeError("group alias name must be non-empty");
    if (this.aliases.some(([existing]) => existing === name)) throw new TypeError(`duplicate group alias: ${name}`);
    this.aliases.push([name, expression]);
    return this;
  }

  filter(expression: GroupingFilterExprT): this {
    this.filterValue = expression;
    return this;
  }

  keep(expression: GroupingFilterExprT): this {
    this.keepValue = expression;
    return this;
  }

  private bodyOperations(): GroupingOperationT[] {
    const operations: GroupingOperationT[] = this.aliases.map(
      ([name, expression]) => new GroupingOperationT(
        GroupingOperationKind.GroupingAliasOperation,
        new GroupingAliasOperationT(name, expression.model),
      ),
    );

    if (this.limitValue !== undefined) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingMaxOperation,
          new GroupingMaxOperationT(
            GroupingMaxKind.FiniteGroupingMax,
            new FiniteGroupingMaxT(this.limitValue),
          ),
        ),
      );
    }
    if (this.precisionValue !== undefined) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingPrecisionOperation,
          new GroupingPrecisionOperationT(this.precisionValue),
        ),
      );
    }
    if (this.orders.length) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingOrderOperation,
          new GroupingOrderOperationT(
            this.orders.map((order) => new GroupingOrderExprT(order.expression.model)),
          ),
        ),
      );
    }
    if (this.filterValue) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingFilterOperation,
          new GroupingFilterOperationT(this.filterValue),
        ),
      );
    }
    if (this.keepValue) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingKeepOperation,
          new GroupingKeepOperationT(this.keepValue),
        ),
      );
    }
    if (this.outputs.length) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingOutputOperation,
          new GroupingOutputOperationT(
            this.outputs.map(
              (output) => new GroupingOutputT(output.label || null, output.expression.model),
            ),
          ),
        ),
      );
    }
    for (const subgroup of this.subgroups) {
      operations.push(
        new GroupingOperationT(
          GroupingOperationKind.GroupingAllOperation,
          new GroupingAllOperationT(subgroup.operations()),
        ),
      );
    }
    return operations;
  }

  private operations(): GroupingOperationT[] {
    return [
      new GroupingOperationT(
        GroupingOperationKind.GroupingGroupOperation,
        new GroupingGroupOperationT(this.expression.model),
      ),
      new GroupingOperationT(
        GroupingOperationKind.GroupingEachOperation,
        new GroupingEachOperationT(this.bodyOperations()),
      ),
    ];
  }

  toRequest(): GroupingRequestT {
    return new GroupingRequestT(
      GroupingSourceKind.StructuredGroupingSource,
      new StructuredGroupingSourceT(new GroupingAllOperationT(this.operations())),
    );
  }
}

export type FacetSort = "count_desc" | "count_asc" | "value_asc" | "value_desc";

export function facetRequest(field: string, limit = 10, ordering: FacetSort = "count_desc", name = field): GroupingRequestT {
  limit = assertUInt32(limit, "facet limit");
  let order: GroupingExpr;
  switch (ordering) {
    case "count_desc": order = agg.count().neg(); break;
    case "count_asc": order = agg.count(); break;
    case "value_desc": order = g.field(field).neg(); break;
    case "value_asc": order = g.field(field); break;
    default: throw new TypeError("facet sort must be count_desc, count_asc, value_asc, or value_desc");
  }

  const root = new GroupingAllOperationT([
    new GroupingOperationT(
      GroupingOperationKind.GroupingGroupOperation,
      new GroupingGroupOperationT(g.field(field).model),
    ),
    new GroupingOperationT(
      GroupingOperationKind.GroupingEachOperation,
      new GroupingEachOperationT([
        new GroupingOperationT(
          GroupingOperationKind.GroupingMaxOperation,
          new GroupingMaxOperationT(
            GroupingMaxKind.FiniteGroupingMax,
            new FiniteGroupingMaxT(limit),
          ),
        ),
        new GroupingOperationT(
          GroupingOperationKind.GroupingOrderOperation,
          new GroupingOrderOperationT([new GroupingOrderExprT(order.model)]),
        ),
        new GroupingOperationT(
          GroupingOperationKind.GroupingOutputOperation,
          new GroupingOutputOperationT([new GroupingOutputT(null, agg.count().model)]),
        ),
      ]),
    ),
  ], name);

  return new GroupingRequestT(
    GroupingSourceKind.StructuredGroupingSource,
    new StructuredGroupingSourceT(root),
  );
}

export function rootAggregationRequest(outputs: GroupingOutput[]): GroupingRequestT {
  const root = new GroupingAllOperationT([
    new GroupingOperationT(
      GroupingOperationKind.GroupingOutputOperation,
      new GroupingOutputOperationT(
        outputs.map((output) => new GroupingOutputT(output.label || null, output.expression.model)),
      ),
    ),
  ]);
  return new GroupingRequestT(
    GroupingSourceKind.StructuredGroupingSource,
    new StructuredGroupingSourceT(root),
  );
}
