import { describe, it, expect } from 'vitest';
import { SearchRequestBuilder, q, toCanonicalJson, fromCanonicalJson, parseTypedQuery, jsonInput, int64, Tensor } from '../src/index.js';
import { packRequest } from '../src/_wire/v1/codec.js';
import { QuerySourceT, ValueT } from '../src/semantic.js';

describe('canonical BFBS JSON editor and independent request builder', () => {
  it('round trips nested typed nodes, annotations, enums and lossless input Values', () => {
    const source = q.allOf(q.term('title', 'orchid'), q.anyOf(q.equals('quantity', 7), q.not(q.equals('quantity', 11))));
    expect(toCanonicalJson(parseTypedQuery(toCanonicalJson(source)))).toEqual(toCanonicalJson(source));
    expect(fromCanonicalJson('QuerySource', toCanonicalJson(source))).toBeInstanceOf(QuerySourceT);
    for (const v of [int64('9007199254740993'), Tensor.dense('tensor<float>(x[2])', [1, 2])]) {
      const canonical = toCanonicalJson(new SearchRequestBuilder(null).input('x', v).build());
      expect(canonical).toBeTruthy();
    }
    const large = toCanonicalJson(int64('9007199254740993'));
    expect(toCanonicalJson(jsonInput(large))).toEqual(large);
    expect(jsonInput(large)).toBeInstanceOf(ValueT);
  });
  it('constructs ordered canonical controls and structured/textual filters without deprecated slots', () => {
    const request = new SearchRequestBuilder('orchid').filterField('quantity', 'gte', 7).filterField('title', 'exists')
      .filter('quantity < $max', { max: 30 }).sort('quantity', 'desc').sort('title', 'asc').offset(2).limit(2).ranking('boosted').input('boost', 3).summary('compact').build();
    const json = toCanonicalJson(request) as any;
    expect(json.filters.structured).toHaveLength(2);
    expect(json.filters.sources).toBeUndefined();
    expect(json.filters.parameters).toBeUndefined();
    expect(json.sort.map((s: any) => s.direction)).toEqual(['Desc', 'Asc']);
    expect(json.offset).toEqual({ value: 2 });
    expect(json.ranking.profile).toBe('boosted');
    expect(json.presentation.summary).toBe('compact');
    expect(packRequest('search', request, 'alpha', 'generation').length).toBeGreaterThan(0);
  });
  it('rejects unknown fields, malformed unions, invalid scalar shape and unsafe integers', () => {
    expect(() => parseTypedQuery({ source_type: 'TypedQuery', source: { query: { expr_type: 'TermQuery', expr: { bogus: true } } } })).toThrow('unknown field');
    expect(() => parseTypedQuery({ source_type: 'TypedQuery' })).toThrow('missing');
    expect(() => parseTypedQuery({ source_type: 'BadNode', source: {} })).toThrow();
    expect(() => fromCanonicalJson('OptionalUInt32', { value: 1.5 })).toThrow()
    expect(() => fromCanonicalJson('OptionalUInt32', { value: 4294967296 })).toThrow()
    expect(() => jsonInput(9007199254740992)).toThrow('explicit');
    expect(() => jsonInput({ kind_type: 'Int64Value', kind: { value: '9223372036854775808' } })).toThrow('64-bit range');
    expect(() => jsonInput({ kind_type: 'Int64Value', kind: { value: 9007199254740992 } })).toThrow('lossless');
  });
});
