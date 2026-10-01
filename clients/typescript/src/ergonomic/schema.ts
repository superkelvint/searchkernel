import { comparePortableKeys } from "../internal/scalars.js";
import { DataTypeKind } from "../semantic.js";
import { DataTypeT } from "../semantic.js";
import { FieldDefinitionT } from "../semantic.js";
import { MatchingConfigT } from "../semantic.js";
import { MatchingMode } from "../semantic.js";
import { OptionalBoolT } from "../semantic.js";
import { OptionalUInt32T } from "../semantic.js";
import { PrimitiveDataTypeKind } from "../semantic.js";
import { PrimitiveDataTypeT } from "../semantic.js";
import { RankPhaseDefinitionT } from "../semantic.js";
import { RankProfileDefinitionT } from "../semantic.js";
import { RankProfileInputT } from "../semantic.js";
import { TensorDataTypeT } from "../semantic.js";
import { ArrayDataTypeT } from "../semantic.js";
import { StructDataTypeT } from "../semantic.js";
import { StructFieldDefinitionT } from "../semantic.js";
import { PositionDataTypeT } from "../semantic.js";
import { Tensor, value } from "../client/values.js";

export interface FieldOptions {
  stored?: boolean;
  searchable?: boolean;
  filterable?: boolean;
  sortable?: boolean;
  facetable?: boolean;
}

function optionalBool(setting: boolean | undefined): OptionalBoolT | null {
  return setting === undefined ? null : new OptionalBoolT(setting);
}

abstract class FieldSpec {
  protected abstract readonly primitive: PrimitiveDataTypeKind;
  protected readonly matching: MatchingMode = MatchingMode.Unspecified;

  constructor(protected readonly options: FieldOptions = {}) {}

  definition(name: string): FieldDefinitionT {
    if (!name) throw new TypeError("field name must not be empty");
    return new FieldDefinitionT(
      name,
      new DataTypeT(DataTypeKind.PrimitiveDataType, new PrimitiveDataTypeT(this.primitive)),
      optionalBool(this.options.stored),
      optionalBool(this.options.searchable),
      optionalBool(this.options.filterable),
      optionalBool(this.options.sortable),
      optionalBool(this.options.facetable),
      null,
      this.primitive === PrimitiveDataTypeKind.String
        ? new MatchingConfigT(this.matching)
        : null,
    );
  }
}

export class Text extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.String;
  protected override readonly matching = MatchingMode.Text;
}

export class Keyword extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.String;
  protected override readonly matching = MatchingMode.Exact;
}

export class Integer extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.Int64;
}

export class Float extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.Float64;
}

export class Boolean extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.Bool;
}

export class PositionField extends FieldSpec {
  protected readonly primitive = PrimitiveDataTypeKind.String;

  override definition(name: string): FieldDefinitionT {
    if (!name) throw new TypeError("field name must not be empty");
    return new FieldDefinitionT(name, new DataTypeT(DataTypeKind.PositionDataType, new PositionDataTypeT()),
      optionalBool(this.options.stored), optionalBool(this.options.searchable),
      optionalBool(this.options.filterable), optionalBool(this.options.sortable), optionalBool(this.options.facetable));
  }
}

export class ArrayOfStructField {
  constructor(readonly members: Record<string, FieldSpec>, readonly options: FieldOptions = {}) {}

  definition(name: string): FieldDefinitionT {
    if (!name || Object.keys(this.members).length === 0) throw new TypeError("array struct field requires a name and members");
    const fields = Object.keys(this.members).sort().map((member) => {
      const type = this.members[member]!.definition(member).dataType;
      return new StructFieldDefinitionT(member, type);
    });
    const struct = new DataTypeT(DataTypeKind.StructDataType, new StructDataTypeT(fields));
    const array = new DataTypeT(DataTypeKind.ArrayDataType, new ArrayDataTypeT(struct));
    return new FieldDefinitionT(name, array, optionalBool(this.options.stored), optionalBool(this.options.searchable),
      optionalBool(this.options.filterable), optionalBool(this.options.sortable), optionalBool(this.options.facetable));
  }
}

export class TensorField {
  constructor(readonly typeSpec: string, readonly options: FieldOptions = {}) {
    if (!typeSpec.trim()) throw new TypeError("tensor type spec must be a non-empty string");
  }

  definition(name: string): FieldDefinitionT {
    if (!name) throw new TypeError("field name must not be empty");
    return new FieldDefinitionT(
      name,
      new DataTypeT(DataTypeKind.TensorDataType, new TensorDataTypeT(this.typeSpec)),
      optionalBool(this.options.stored),
      optionalBool(this.options.searchable),
      optionalBool(this.options.filterable),
      optionalBool(this.options.sortable),
      optionalBool(this.options.facetable),
    );
  }
}

export class RankProfile {
  private firstPhaseExpression?: string;
  private secondPhaseExpression?: string;
  private rerankCountValue?: number;
  private readonly matchFeatures: string[] = [];
  private readonly queryInputs = new Map<string, string>();

  constructor(readonly name: string) {
    if (!name) throw new TypeError("rank profile name must not be empty");
  }

  firstPhase(expression: string): this {
    this.firstPhaseExpression = expression;
    return this;
  }

  secondPhase(expression: string): this {
    if (!expression.trim()) throw new TypeError("second-phase expression must not be empty");
    this.secondPhaseExpression = expression;
    return this;
  }

  rerankCount(count: number): this {
    if (!Number.isInteger(count) || count < 0 || count > 0xffff_ffff) {
      throw new RangeError("rerank count must be a uint32");
    }
    this.rerankCountValue = count;
    return this;
  }

  matchFeature(expression: string): this {
    this.matchFeatures.push(expression);
    return this;
  }

  queryInput(name: string, typeSpec: string): this {
    if (!name) throw new TypeError("query input name must not be empty");
    if (!typeSpec.trim()) throw new TypeError("query input tensor type spec must be non-empty");
    this.queryInputs.set(name, typeSpec);
    return this;
  }

  definition(): RankProfileDefinitionT {
    const inputs = [...this.queryInputs]
      .sort(([a], [b]) => comparePortableKeys(a, b))
      .map(([name, typeSpec]) => new RankProfileInputT(
        name,
        new DataTypeT(DataTypeKind.TensorDataType, new TensorDataTypeT(typeSpec)),
        value(Tensor.dense(typeSpec, [])),
      ));

    const secondPhase = this.secondPhaseExpression === undefined && this.rerankCountValue === undefined
      ? null
      : new RankPhaseDefinitionT(
        this.secondPhaseExpression ?? null,
        this.rerankCountValue === undefined ? null : new OptionalUInt32T(this.rerankCountValue),
      );

    return new RankProfileDefinitionT(
      this.name,
      [],
      inputs,
      [],
      this.firstPhaseExpression === undefined
        ? null
        : new RankPhaseDefinitionT(this.firstPhaseExpression),
      secondPhase,
      null,
      [],
      [...this.matchFeatures],
    );
  }
}
