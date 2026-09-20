export const UINT32_MAX = 0xffff_ffff;

export function assertUInt32(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > UINT32_MAX) {
    throw new RangeError(`${label} must be an integer between 0 and ${UINT32_MAX}`);
  }
  return value;
}

/**
 * Canonical portable-map key ordering.
 *
 * JavaScript's native string comparison uses UTF-16 code units, which differs
 * from Rust/Python Unicode scalar-value ordering for non-BMP characters.
 * Compare code points explicitly so independently implemented clients emit the
 * same semantic-map ordering.
 */
export function comparePortableKeys(left: string, right: string): number {
  const leftPoints = Array.from(left, (part) => part.codePointAt(0)!);
  const rightPoints = Array.from(right, (part) => part.codePointAt(0)!);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const delta = leftPoints[index]! - rightPoints[index]!;
    if (delta !== 0) return delta < 0 ? -1 : 1;
  }
  return leftPoints.length < rightPoints.length ? -1 : leftPoints.length > rightPoints.length ? 1 : 0;
}
