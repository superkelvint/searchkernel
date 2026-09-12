# Portable value model

The conformance transport is JSON, but the public clients need the full Search Kernel value domain.

## i64

Signed 64-bit integers outside JavaScript's exact integer range use the conformance-only tagged form:

```json
{"$i64":"9223372036854775807"}
```

Language clients should expose their idiomatic exact integer type (`i64`, `int`, `BigInt`, etc.). The tagged representation is not application syntax.

## f64

NaN and infinities are not portable Search Kernel values and are rejected. `-0.0` canonicalizes to `0.0`. Tagged `$f64` forms exist only so the conformance harness can express invalid non-finite inputs.

## strings

Keyword/string values are preserved exactly. Search Kernel performs no implicit Unicode normalization on exact values. Text analysis is controlled by the selected analyzer.

## null and missing

For document input, JSON/null on an optional field normalizes to missing. Null on a required field is an error. Empty strings and empty arrays remain real values.
