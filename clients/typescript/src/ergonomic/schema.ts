import { comparePortableKeys } from "../internal/scalars.js";
import { DataTypeKind } from "../generated/flatbuffers/search-kernel/v2/data-type-kind.js";
import { DataTypeT } from "../generated/flatbuffers/search-kernel/v2/data-type.js";
import { FieldDefinitionT } from "../generated/flatbuffers/search-kernel/v2/field-definition.js";
import { MatchingConfigT } from "../generated/flatbuffers/search-kernel/v2/matching-config.js";
import { MatchingMode } from "../generated/flatbuffers/search-kernel/v2/matching-mode.js";
import { OptionalBoolT } from "../generated/flatbuffers/search-kernel/v2/optional-bool.js";
import { PrimitiveDataTypeKind } from "../generated/flatbuffers/search-kernel/v2/primitive-data-type-kind.js";
import { PrimitiveDataTypeT } from "../generated/flatbuffers/search-kernel/v2/primitive-data-type.js";
import { RankPhaseDefinitionT } from "../generated/flatbuffers/search-kernel/v2/rank-phase-definition.js";
import { RankProfileDefinitionT } from "../generated/flatbuffers/search-kernel/v2/rank-profile-definition.js";
import { RankProfileInputT } from "../generated/flatbuffers/search-kernel/v2/rank-profile-input.js";
import { TensorDataTypeT } from "../generated/flatbuffers/search-kernel/v2/tensor-data-type.js";
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
  private readonly matchFeatures: string[] = [];
  private readonly queryInputs = new Map<string, string>();

  constructor(readonly name: string) {
    if (!name) throw new TypeError("rank profile name must not be empty");
  }

  firstPhase(expression: string): this {
    this.firstPhaseExpression = expression;
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

    return new RankProfileDefinitionT(
      this.name,
      [],
      inputs,
      [],
      this.firstPhaseExpression === undefined
        ? null
        : new RankPhaseDefinitionT(this.firstPhaseExpression),
      null,
      null,
      [],
      [...this.matchFeatures],
    );
  }
}
