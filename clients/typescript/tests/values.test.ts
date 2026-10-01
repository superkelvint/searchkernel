import { describe, expect, it } from "vitest";
import { Tensor, ValueDecodeError, float64, fromValue, int64, uint64, value } from "../src/client/values.js";
import { DenseTensorValuesT } from "../src/semantic.js";
import { MixedTensorBlocksT } from "../src/semantic.js";
import { NonFiniteFloatKind } from "../src/semantic.js";
import { NonFiniteFloatValueT } from "../src/semantic.js";
import { SparseTensorCellsT } from "../src/semantic.js";
import { TensorAddressEntryT } from "../src/semantic.js";
import { TensorBlockT } from "../src/semantic.js";
import { TensorCellT } from "../src/semantic.js";
import { TensorPayloadKind } from "../src/semantic.js";
import { TensorValueT } from "../src/semantic.js";
import { ValueEntryT } from "../src/semantic.js";
import { ValueKind } from "../src/semantic.js";
import { ValueObjectT } from "../src/semantic.js";
import { ValueT } from "../src/semantic.js";

describe("portable values", () => {
  it("encodes and decodes scalar values losslessly", () => {
    expect(fromValue(value(null))).toBeNull();
    expect(fromValue(value(true))).toBe(true);
    expect(fromValue(value("hello"))).toBe("hello");
    expect(fromValue(value(1.5))).toBe(1.5);
    expect(fromValue(value(42))).toBe(42n);
  });

  it("provides an explicit Float64 representation for integer-valued JavaScript numbers", () => {
    expect(value(129).kindType).toBe(ValueKind.Int64Value);
    expect(float64(129).kindType).toBe(ValueKind.Float64Value);
    expect(fromValue(float64(129))).toBe(129);
    expect(Object.is(fromValue(float64(-0)), -0)).toBe(true);
  });

  it("preserves bytes, recursive values and dense tensors", () => {
    expect(fromValue(value(new Uint8Array([1, 2, 255])))).toEqual(new Uint8Array([1, 2, 255]));
    expect(fromValue(value({ b: [true, "x"], a: null }))).toEqual({ a: null, b: [true, "x"] });
    expect(fromValue(value(Tensor.dense("tensor<float>(x[2])", [1, 2])))).toEqual({
      $searchkernel_tensor: { typeSpec: "tensor<float>(x[2])", dense: [1, 2] },
    });
  });

  it("decodes sparse and mixed tensors without losing addresses or values", () => {
    const address = [new TensorAddressEntryT("category", "books")];
    const sparse = new ValueT(
      ValueKind.TensorValue,
      new TensorValueT(
        "tensor<float>(category{})",
        TensorPayloadKind.SparseTensorCells,
        new SparseTensorCellsT([new TensorCellT(address, 3.5)]),
      ),
    );
    expect(fromValue(sparse)).toEqual({
      $searchkernel_tensor: {
        typeSpec: "tensor<float>(category{})",
        sparse: [{ address: [{ dimension: "category", label: "books" }], value: 3.5 }],
      },
    });

    const mixed = new ValueT(
      ValueKind.TensorValue,
      new TensorValueT(
        "tensor<float>(category{},x[2])",
        TensorPayloadKind.MixedTensorBlocks,
        new MixedTensorBlocksT([new TensorBlockT(address, [1, 2])]),
      ),
    );
    expect(fromValue(mixed)).toEqual({
      $searchkernel_tensor: {
        typeSpec: "tensor<float>(category{},x[2])",
        mixed: [{ address: [{ dimension: "category", label: "books" }], values: [1, 2] }],
      },
    });
  });

  it("supports full-range signed and unsigned 64-bit values inside documents", () => {
    const signed = int64("-9223372036854775808");
    const unsigned = uint64("18446744073709551615");
    expect(value({ signed, unsigned }).kindType).toBe(ValueKind.ValueObject);
    expect(fromValue(signed)).toBe(-9223372036854775808n);
    expect(fromValue(unsigned)).toBe(18446744073709551615n);
  });

  it("does not reinterpret ordinary decimal strings as integers", () => {
    expect(value("9223372036854775807").kindType).toBe(ValueKind.StringValue);
  });

  it("accepts decimal integer strings only", () => {
    expect(() => int64("0x10")).toThrow(/decimal/);
    expect(() => int64(" ")).toThrow(/decimal/);
    expect(() => int64("+1")).toThrow(/decimal/);
    expect(() => uint64("-1")).toThrow(/decimal/);
    expect(fromValue(int64("01"))).toBe(1n);
  });

  it("rejects unsafe integers, non-finite inputs and out-of-range 64-bit values", () => {
    expect(() => value(Number.MAX_SAFE_INTEGER + 1)).toThrow(/64-bit/);
    expect(() => value(Infinity)).toThrow(/non-finite/);
    expect(() => float64(Infinity)).toThrow(/finite/);
    expect(() => int64("9223372036854775808")).toThrow(/out of range/);
    expect(() => uint64("18446744073709551616")).toThrow(/out of range/);
  });

  it("decodes explicit non-finite response values", () => {
    expect(fromValue(new ValueT(
      ValueKind.NonFiniteFloatValue,
      new NonFiniteFloatValueT(NonFiniteFloatKind.PositiveInfinity),
    ))).toBe(Infinity);
    expect(Number.isNaN(fromValue(new ValueT(
      ValueKind.NonFiniteFloatValue,
      new NonFiniteFloatValueT(NonFiniteFloatKind.NaN),
    )) as number)).toBe(true);
    expect(fromValue(new ValueT(
      ValueKind.NonFiniteFloatValue,
      new NonFiniteFloatValueT(NonFiniteFloatKind.NegativeInfinity),
    ))).toBe(Number.NEGATIVE_INFINITY);
  });

  it("fails closed for missing or unknown value/tensor discriminators", () => {
    expect(() => fromValue(new ValueT(ValueKind.NONE, null))).toThrow(ValueDecodeError);
    expect(() => fromValue(new ValueT(255 as ValueKind, null))).toThrow(ValueDecodeError);
    expect(() => fromValue(new ValueT(
      ValueKind.TensorValue,
      new TensorValueT("tensor<float>(x[1])", TensorPayloadKind.NONE, null),
    ))).toThrow(ValueDecodeError);
  });
});

it("fails closed for known value discriminators with missing or mismatched payloads", () => {
  expect(() => fromValue(new ValueT(ValueKind.BoolValue, null))).toThrow(ValueDecodeError);
  expect(() => fromValue(new ValueT(ValueKind.Int64Value, null))).toThrow(ValueDecodeError);
  expect(() => fromValue(new ValueT(ValueKind.StringValue, null))).toThrow(ValueDecodeError);
  expect(() => fromValue(new ValueT(ValueKind.BoolValue, new NonFiniteFloatValueT(NonFiniteFloatKind.NaN)))).toThrow(ValueDecodeError);
});


it("rejects duplicate semantic-map keys instead of applying last-wins", () => {
  const duplicate = new ValueT(
    ValueKind.ValueObject,
    new ValueObjectT([
      new ValueEntryT("same", value(1)),
      new ValueEntryT("same", value(2)),
    ]),
  );
  expect(() => fromValue(duplicate)).toThrow(/duplicate value object key/);
});

it("sorts semantic-map keys by Unicode scalar value, not UTF-16 code units", () => {
  const bmp = "\uE000";
  const nonBmp = "\u{10000}";
  const encoded = value({ [nonBmp]: 1, [bmp]: 2 });
  const object = encoded.kind as ValueObjectT;
  expect(object.fields.map((entry) => entry.key)).toEqual([bmp, nonBmp]);
});


it("rejects missing required ValueObject payloads", () => {
  const malformed = new ValueT(
    ValueKind.ValueObject,
    new ValueObjectT([new ValueEntryT("missing", null)]),
  );
  expect(() => fromValue(malformed)).toThrow(/Value payload/);
});

it("rejects malformed tensor addresses and non-finite decoded cells", () => {
  const missingLabel = new ValueT(
    ValueKind.TensorValue,
    new TensorValueT(
      "tensor<float>(category{})",
      TensorPayloadKind.SparseTensorCells,
      new SparseTensorCellsT([new TensorCellT([new TensorAddressEntryT("category", null)], 1)]),
    ),
  );
  expect(() => fromValue(missingLabel)).toThrow(/dimension or label/);

  const dense = new ValueT(
    ValueKind.TensorValue,
    new TensorValueT("tensor<float>(x[1])", TensorPayloadKind.DenseTensorValues, new DenseTensorValuesT([Number.NaN])),
  );
  expect(() => fromValue(dense)).toThrow(/finite/);

  const sparse = new ValueT(
    ValueKind.TensorValue,
    new TensorValueT(
      "tensor<float>(category{})",
      TensorPayloadKind.SparseTensorCells,
      new SparseTensorCellsT([new TensorCellT([new TensorAddressEntryT("category", "books")], Number.POSITIVE_INFINITY)]),
    ),
  );
  expect(() => fromValue(sparse)).toThrow(/finite/);

  const mixed = new ValueT(
    ValueKind.TensorValue,
    new TensorValueT(
      "tensor<float>(category{},x[1])",
      TensorPayloadKind.MixedTensorBlocks,
      new MixedTensorBlocksT([new TensorBlockT([new TensorAddressEntryT("category", "books")], [Number.NEGATIVE_INFINITY])]),
    ),
  );
  expect(() => fromValue(mixed)).toThrow(/finite/);
});
