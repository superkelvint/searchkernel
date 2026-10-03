import * as S from '../semantic.js';
import { value, type ValueInput } from './values.js';

export type CanonicalJson = null | boolean | number | string | CanonicalJson[] | { [key: string]: CanonicalJson };
const models = S as unknown as Record<string, any>;

/** Standard schema-aware FlatBuffers JSON, reflected from the canonical BFBS.
 * 64-bit scalars use decimal strings, which the official parser accepts losslessly.
 * This is a logical object inspector/editor; transport remains FlatBuffers.
 */
export function toCanonicalJson(input: unknown): CanonicalJson {
  if (input === null) return null;
  if (typeof input === 'bigint') return input.toString();
  if (typeof input === 'string' || typeof input === 'boolean') return input;
  if (typeof input === 'number') {
    if (!Number.isFinite(input)) throw new TypeError('Non-finite scalar requires the canonical non-finite Value union');
    return input;
  }
  if (input instanceof Uint8Array || Array.isArray(input)) return Array.from(input as Iterable<unknown>).map(toCanonicalJson);
  if (S.isSemanticTable(input)) {
    const result: Record<string, CanonicalJson> = {};
    for (const [field, spec] of Object.entries(S.SEMANTIC_JSON_FIELDS[input._schemaName]!)) {
      // Match the canonical textual-filter packer, preserving absent legacy slots.
      if (input._schemaName === 'FilterRequest' && (input as any)._textualOnly && field !== 'textual' && (field !== 'structured' || !(input as any).structured.length)) continue;
      const item = (input as any)[field];
      if (item == null) continue;
      const enumSpec = S.SEMANTIC_ENUM_FIELDS[input._schemaName]?.[field];
      result[spec.name] = enumSpec
        ? enumSpec.endsWith('[]') ? item.map((v: number) => models[enumSpec.slice(0, -2)][v]) : models[enumSpec][item]
        : toCanonicalJson(item);
    }
    return result;
  }
  throw new TypeError('Expected a canonical semantic table or scalar');
}

/** Structural decoding only. Schema-aware query meaning remains in Search Kernel IR. */
export function fromCanonicalJson(table: string, input: unknown, path = table): S.SemanticTable {
  const specs = S.SEMANTIC_JSON_FIELDS[table];
  if (!specs || !input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError(`${path}: expected ${table} object`);
  const object = input as Record<string, unknown>;
  const allowed = new Set(Object.values(specs).map(spec => spec.name));
  for (const key of Object.keys(object)) if (!allowed.has(key)) throw new TypeError(`${path}.${key}: unknown field`);
  const result = new models[`${table}T`]();
  function decode(type: string, item: unknown, at: string, field: string): unknown {
    if (type.startsWith('(') && type.endsWith(')[]')) {
      if (!Array.isArray(item)) throw new TypeError(`${at}: expected array`);
      return item.map((v, i) => decode(type.slice(1, -3), v, `${at}[${i}]`, field));
    }
    if (item === null) {
      if (type.includes('null')) return null;
      throw new TypeError(`${at}: null is not supported`);
    }
    const alternatives = type.split('|').filter(t => t !== 'null' && t !== 'Uint8Array');
    const tables = alternatives.filter(t => t.endsWith('T'));
    if (tables.length) {
      const discriminator = object[`${specs[field]!.name}_type`];
      const selected = tables.length === 1 ? tables[0]! : `${discriminator}T`;
      if (!tables.includes(selected)) throw new TypeError(`${at}: missing or mismatched union discriminator`);
      return fromCanonicalJson(selected.slice(0, -1), item, at);
    }
    const base = alternatives[0]!;
    if (base === 'bigint') {
      if ((typeof item !== 'string' || !/^-?\d+$/.test(item)) && !(typeof item === 'number' && Number.isSafeInteger(item))) throw new TypeError(`${at}: expected a lossless decimal integer`);
      const integer = BigInt(item as string | number), unsigned = specs[field]!.unsigned;
      const min = unsigned ? 0n : -(1n << 63n), max = unsigned ? (1n << 64n) - 1n : (1n << 63n) - 1n;
      if (integer < min || integer > max) throw new RangeError(`${at}: integer is outside the canonical 64-bit range`);
      return integer;
    }
    if (['string', 'number', 'boolean'].includes(base)) {
      if (typeof item !== base || (typeof item === 'number' && !Number.isFinite(item))) throw new TypeError(`${at}: expected ${base}`);
      if (typeof item === 'number') {
        const scalar = specs[field]!.scalar;
        const bounds: Record<string, [number, number]> = { Byte: [-128, 127], UByte: [0, 255], Short: [-32768, 32767], UShort: [0, 65535], Int: [-2147483648, 2147483647], UInt: [0, 4294967295] };
        const range = bounds[scalar];
        if (range && (!Number.isInteger(item) || item < range[0] || item > range[1])) throw new RangeError(`${at}: expected ${scalar} integer in [${range[0]}, ${range[1]}]`);
        if (scalar === 'Float' && Math.abs(item) > 3.4028234663852886e38) throw new RangeError(`${at}: value overflows float32`);
      }
      return item;
    }
    const enumValue = typeof item === 'string' ? models[base]?.[item] : undefined;
    if (typeof enumValue !== 'number') throw new TypeError(`${at}: unknown ${base} enum member`);
    return enumValue;
  }
  for (const [field, spec] of Object.entries(specs)) {
    if (Object.hasOwn(object, spec.name)) result[field] = decode(spec.type, object[spec.name], `${path}.${spec.name}`, field);
  }
  if (table === 'FilterRequest' && !Object.hasOwn(object, 'sources') && !Object.hasOwn(object, 'parameters')) result._textualOnly = true;
  // Union presence must match before a malformed object reaches the packer.
  for (const [field, enumName] of Object.entries(S.SEMANTIC_ENUM_FIELDS[table] ?? {})) {
    if (!field.endsWith('Type')) continue;
    const payload = field.slice(0, -4);
    const member = models[enumName]?.[result[field]];
    if (member === 'NONE') {
      if (result[payload] !== null) throw new TypeError(`${path}.${payload}: NONE union has a payload`);
    } else if (!(result[payload] instanceof models[`${member}T`])) throw new TypeError(`${path}.${payload}: missing or mismatched ${member} payload`);
  }
  return result;
}

export function parseTypedQuery(input: unknown): S.QuerySourceT {
  const query = fromCanonicalJson('QuerySource', input) as S.QuerySourceT;
  if (query.sourceType !== S.QuerySourceKind.TypedQuery) throw new TypeError('Typed editor requires source_type: TypedQuery');
  return query;
}

/** JSON scalar/container input or an explicit canonical Value (e.g. int64/tensor). */
export function jsonInput(input: unknown): ValueInput {
  if (input && typeof input === 'object' && !Array.isArray(input)) {
    if (Object.hasOwn(input, 'kind_type')) return fromCanonicalJson('Value', input) as S.ValueT;
    return Object.fromEntries(Object.entries(input).map(([key, item]) => [key, jsonInput(item)]));
  }
  if (Array.isArray(input)) return input.map(jsonInput);
  if (input === null || ['number', 'boolean', 'string'].includes(typeof input)) {
    if (typeof input === 'number' && Number.isInteger(input) && !Number.isSafeInteger(input)) throw new TypeError('Use an explicit canonical int64/uint64 Value for large integers');
    return value(input as number | boolean | string | null);
  }
  throw new TypeError('Expected a JSON Search Kernel value');
}

/** Read-only editor reference derived from the same BFBS metadata as decoding. */
export function queryEditorReference(): CanonicalJson {
  const tables: Record<string, CanonicalJson> = {}, pending = ['QuerySource'];
  while (pending.length) {
    const name = pending.pop()!;
    if (Object.hasOwn(tables, name)) continue;
    const fields: Record<string, CanonicalJson> = {};
    tables[name] = fields;
    for (const [field, spec] of Object.entries(S.SEMANTIC_JSON_FIELDS[name]!)) {
      const enumName = S.SEMANTIC_ENUM_FIELDS[name]?.[field]?.replace(/\[\]$/, '');
      fields[spec.name] = enumName
        ? { type: spec.type, members: Object.keys(models[enumName]).filter(key => !/^\d+$/.test(key)) }
        : { type: spec.type };
      for (const type of spec.type.match(/[A-Z]\w*T\b/g) ?? []) {
        const table = type.slice(0, -1);
        if (S.SEMANTIC_JSON_FIELDS[table]) pending.push(table);
      }
    }
  }
  return tables;
}
