export function paritySafe(value: unknown): unknown {
  if (typeof value === "bigint") {
    if (value >= BigInt(Number.MIN_SAFE_INTEGER) && value <= BigInt(Number.MAX_SAFE_INTEGER)) {
      return Number(value);
    }
    return value.toString();
  }
  if (Array.isArray(value)) return value.map(paritySafe);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, item]) => [key, paritySafe(item)]),
    );
  }
  return value;
}
