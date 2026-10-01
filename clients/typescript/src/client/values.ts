import { BoolValueT } from "../semantic.js";
import { BytesValueT } from "../semantic.js";
import { DenseTensorValuesT } from "../semantic.js";
import { Float64ValueT } from "../semantic.js";
import { Int64ValueT } from "../semantic.js";
import { MixedTensorBlocksT } from "../semantic.js";
import { NonFiniteFloatKind } from "../semantic.js";
import { NonFiniteFloatValueT } from "../semantic.js";
import { NullValue } from "../semantic.js";
import { NullValueValueT } from "../semantic.js";
import { SparseTensorCellsT } from "../semantic.js";
import { StringValueT } from "../semantic.js";
import { TensorAddressEntryT } from "../semantic.js";
import { TensorPayloadKind } from "../semantic.js";
import { TensorValueT } from "../semantic.js";
import { UInt64ValueT } from "../semantic.js";
import { ValueEntryT } from "../semantic.js";
import { ValueKind } from "../semantic.js";
import { ValueListT } from "../semantic.js";
import { ValueObjectT } from "../semantic.js";
import { ValueT } from "../semantic.js";
import { comparePortableKeys } from "../internal/scalars.js";

export type Int64 = bigint | string | number;

export class ValueDecodeError extends Error {
  override name = "ValueDecodeError";
}

function portableText(input: string | Uint8Array | null | undefined): string | null {
  if (input == null) return null;
  if (typeof input === "string") return input;
  return new TextDecoder().decode(input);
}

export class Tensor {
  private constructor(readonly typeSpec: string, readonly values: readonly number[]) {}

  static dense(typeSpec: string, values: readonly number[]): Tensor {
    if (!typeSpec.trim()) throw new TypeError("tensor type spec must be a non-empty string");
    for (const cell of values) {
      if (!Number.isFinite(cell)) throw new TypeError("dense tensor cells must be finite");
    }
    return new Tensor(typeSpec, [...values]);
  }
}

export interface DecodedTensorAddressEntry {
  dimension: string;
  label: string;
}

export interface DecodedSparseTensorCell {
  address: DecodedTensorAddressEntry[];
  value: number;
}

export interface DecodedMixedTensorBlock {
  address: DecodedTensorAddressEntry[];
  values: number[];
}

export type DecodedTensor =
  | { $searchkernel_tensor: { typeSpec: string | null; dense: number[] } }
  | { $searchkernel_tensor: { typeSpec: string | null; sparse: DecodedSparseTensorCell[] } }
  | { $searchkernel_tensor: { typeSpec: string | null; mixed: DecodedMixedTensorBlock[] } };

/**
 * Values accepted by ergonomic document/query APIs.
 *
 * `ValueT` is accepted deliberately so explicit helpers such as `int64()`,
 * `uint64()` and `float64()` can be embedded inside documents, lists and
 * objects without being reinterpreted as ordinary JavaScript objects.
 */
export type ValueInput =
  | ValueT
  | null
  | boolean
  | string
  | number
  | bigint
  | Uint8Array
  | Tensor
  | readonly ValueInput[]
  | { readonly [key: string]: ValueInput };

function parseInt64(input: Int64, signed: boolean): bigint {
  let parsed: bigint;

  if (typeof input === "number") {
    if (!Number.isSafeInteger(input)) {
      throw new RangeError(
        "64-bit numbers outside Number.MAX_SAFE_INTEGER must be bigint or decimal strings",
      );
    }
    parsed = BigInt(input);
  } else if (typeof input === "string") {
    const decimal = signed ? /^-?[0-9]+$/ : /^[0-9]+$/;
    if (!decimal.test(input)) {
      throw new TypeError("64-bit integer strings must be decimal integers");
    }
    parsed = BigInt(input);
  } else {
    parsed = input;
  }

  const min = signed ? -(1n << 63n) : 0n;
  const max = signed ? (1n << 63n) - 1n : (1n << 64n) - 1n;
  if (parsed < min || parsed > max) {
    throw new RangeError(signed ? "int64 value is out of range" : "uint64 value is out of range");
  }
  return parsed;
}

/** Explicit signed 64-bit value, including lossless decimal-string input. */
export function int64(input: Int64): ValueT {
  return new ValueT(ValueKind.Int64Value, new Int64ValueT(parseInt64(input, true)));
}

/** Explicit unsigned 64-bit value, including lossless decimal-string input. */
export function uint64(input: Int64): ValueT {
  return new ValueT(ValueKind.UInt64Value, new UInt64ValueT(parseInt64(input, false)));
}

/**
 * Explicit Float64 value.
 *
 * JavaScript cannot distinguish `129` from `129.0`, so use this helper when
 * an integer-valued number must retain Float64 wire semantics.
 */
export function float64(input: number): ValueT {
  if (typeof input !== "number" || !Number.isFinite(input)) {
    throw new TypeError("float64 values must be finite JavaScript numbers");
  }
  return new ValueT(ValueKind.Float64Value, new Float64ValueT(input));
}

/** Converts ergonomic values to the private wire value model. */
export function value(input: ValueInput): ValueT {
  if (input instanceof ValueT) return input;
  if (input === null) {
    return new ValueT(ValueKind.NullValueValue, new NullValueValueT(NullValue.Null));
  }
  if (typeof input === "boolean") {
    return new ValueT(ValueKind.BoolValue, new BoolValueT(input));
  }
  if (typeof input === "string") {
    return new ValueT(ValueKind.StringValue, new StringValueT(input));
  }
  if (typeof input === "number") {
    if (!Number.isFinite(input)) throw new TypeError("non-finite floats are not portable");
    return Number.isInteger(input) ? int64(input) : float64(input);
  }
  if (typeof input === "bigint") return int64(input);
  if (input instanceof Uint8Array) {
    return new ValueT(ValueKind.BytesValue, new BytesValueT([...input]));
  }
  if (input instanceof Tensor) {
    return new ValueT(
      ValueKind.TensorValue,
      new TensorValueT(
        input.typeSpec,
        TensorPayloadKind.DenseTensorValues,
        new DenseTensorValuesT([...input.values]),
      ),
    );
  }
  if (Array.isArray(input)) {
    return new ValueT(ValueKind.ValueList, new ValueListT(input.map(value)));
  }
  const object = input as { readonly [key: string]: ValueInput };
  return new ValueT(
    ValueKind.ValueObject,
    new ValueObjectT(
      Object.keys(object)
        .sort(comparePortableKeys)
        .map((key) => new ValueEntryT(key, value(object[key]!))),
    ),
  );
}

function finiteTensorCells(values: readonly number[], label: string): number[] {
  const decoded = [...values];
  if (decoded.some((cell) => !Number.isFinite(cell))) {
    throw new ValueDecodeError(`${label} tensor cells must be finite`);
  }
  return decoded;
}

function decodeAddress(entries: readonly TensorAddressEntryT[]): DecodedTensorAddressEntry[] {
  return entries.map((entry) => {
    const dimension = portableText(entry.dimension);
    const label = portableText(entry.label);
    if (dimension === null || label === null) {
      throw new ValueDecodeError("tensor address entry has a missing dimension or label");
    }
    return { dimension, label };
  });
}

/** Returns a union payload only when it matches the discriminator exactly. */
function requirePayload<T>(
  input: ValueT,
  ctor: abstract new (...args: never[]) => T,
  label: string,
): T {
  if (!(input.kind instanceof ctor)) {
    throw new ValueDecodeError(`${label} value has a missing or mismatched payload`);
  }
  return input.kind;
}

export function decodeValueEntries(
  fields: readonly (ValueEntryT | null)[] | null | undefined,
  label = "value object",
): Record<string, unknown> {
  const decoded: Record<string, unknown> = {};
  const seen = new Set<string>();
  for (const entry of fields ?? []) {
    if (entry === null) throw new ValueDecodeError(`malformed ${label} entry`);
    const key = portableText(entry.key);
    if (key === null) throw new ValueDecodeError(`${label} entry has no key`);
    if (seen.has(key)) throw new ValueDecodeError(`duplicate ${label} key: ${key}`);
    if (entry.value === null) throw new ValueDecodeError(`${label} entry has no Value payload: ${key}`);
    seen.add(key);
    decoded[key] = fromValue(entry.value);
  }
  return decoded;
}

/** Losslessly converts decoded values into JavaScript values. */
export function fromValue(input: ValueT | null): unknown {
  if (!input) return null;

  switch (input.kindType) {
    case ValueKind.NullValueValue: {
      const payload = requirePayload(input, NullValueValueT, "null");
      if (payload.value !== NullValue.Null) {
        throw new ValueDecodeError(`unsupported null value kind: ${payload.value}`);
      }
      return null;
    }
    case ValueKind.BoolValue:
      return requirePayload(input, BoolValueT, "bool").value;
    case ValueKind.Int64Value:
      return requirePayload(input, Int64ValueT, "int64").value;
    case ValueKind.UInt64Value:
      return requirePayload(input, UInt64ValueT, "uint64").value;
    case ValueKind.Float64Value:
      return requirePayload(input, Float64ValueT, "float64").value;
    case ValueKind.NonFiniteFloatValue: {
      const kind = requirePayload(input, NonFiniteFloatValueT, "non-finite float").kind;
      switch (kind) {
        case NonFiniteFloatKind.NaN:
          return Number.NaN;
        case NonFiniteFloatKind.PositiveInfinity:
          return Number.POSITIVE_INFINITY;
        case NonFiniteFloatKind.NegativeInfinity:
          return Number.NEGATIVE_INFINITY;
        default:
          throw new ValueDecodeError(`unsupported non-finite float kind: ${String(kind)}`);
      }
    }
    case ValueKind.StringValue: {
      const decoded = portableText(requirePayload(input, StringValueT, "string").value);
      if (decoded === null) throw new ValueDecodeError("string value has no string payload");
      return decoded;
    }
    case ValueKind.BytesValue:
      return new Uint8Array(requirePayload(input, BytesValueT, "bytes").value);
    case ValueKind.ValueList:
      return requirePayload(input, ValueListT, "list").values.map(fromValue);
    case ValueKind.ValueObject:
      return decodeValueEntries(requirePayload(input, ValueObjectT, "object").fields);
    case ValueKind.TensorValue: {
      const tensor = requirePayload(input, TensorValueT, "tensor");
      const typeSpec = portableText(tensor.typeSpec);
      if (typeSpec === null) throw new ValueDecodeError("tensor value has no type spec");
      switch (tensor.payloadType) {
        case TensorPayloadKind.DenseTensorValues: {
          if (!(tensor.payload instanceof DenseTensorValuesT)) {
            throw new ValueDecodeError("dense tensor has a missing or mismatched payload");
          }
          return { $searchkernel_tensor: { typeSpec, dense: finiteTensorCells(tensor.payload.values, "dense") } } satisfies DecodedTensor;
        }
        case TensorPayloadKind.SparseTensorCells: {
          if (!(tensor.payload instanceof SparseTensorCellsT)) {
            throw new ValueDecodeError("sparse tensor has a missing or mismatched payload");
          }
          return {
            $searchkernel_tensor: {
              typeSpec,
              sparse: tensor.payload.cells.map((cell) => ({
                address: decodeAddress(cell.address),
                value: finiteTensorCells([cell.value], "sparse")[0]!,
              })),
            },
          } satisfies DecodedTensor;
        }
        case TensorPayloadKind.MixedTensorBlocks: {
          if (!(tensor.payload instanceof MixedTensorBlocksT)) {
            throw new ValueDecodeError("mixed tensor has a missing or mismatched payload");
          }
          return {
            $searchkernel_tensor: {
              typeSpec,
              mixed: tensor.payload.blocks.map((block) => ({
                address: decodeAddress(block.address),
                values: finiteTensorCells(block.values, "mixed"),
              })),
            },
          } satisfies DecodedTensor;
        }
        default:
          throw new ValueDecodeError(`unsupported tensor payload kind: ${tensor.payloadType}`);
      }
    }
    case ValueKind.NONE:
    default:
      throw new ValueDecodeError(`unsupported value kind: ${input.kindType}`);
  }
}
