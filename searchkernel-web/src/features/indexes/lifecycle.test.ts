import { describe, expect, it } from 'vitest'
import { IndexTargetT, SearchKernelClientError, targetKey, type IndexStatus } from '../../api/searchkernel-client'
import { canDelete, failureAction, reconcileTarget } from './lifecycle'

describe('incarnation safety', () => {
  const old = new IndexTargetT('alpha', 'G1'), current = new IndexTargetT('alpha', 'G2')
  const inventory: IndexStatus[] = [{ name: 'alpha', generation: 'G2', persisted: true, state: 'open', schema_name: 'schema', warning: null }]
  it('keys the canonical target by name and generation', () => {
    expect(targetKey(old)).not.toBe(targetKey(current))
    expect(targetKey(old)).toBe(targetKey(new IndexTargetT('alpha', 'G1')))
    expect(targetKey(old)).not.toBe(targetKey(new IndexTargetT('beta', 'G1')))
  })
  it('reconciles deleted, recreated, restarted and transitioning targets without retargeting', () => {
    expect(reconcileTarget(old, inventory)).toBe('stale')
    expect(reconcileTarget(current, inventory)).toBe('current')
    expect(reconcileTarget(old, [])).toBe('missing')
    expect(reconcileTarget(old, [{ ...inventory[0], generation: null, state: 'persisted' }])).toBe('stale')
    expect(reconcileTarget(old, [{ ...inventory[0], generation: null, state: 'transitioning' }])).toBe('transitioning')
    expect(old.generation).toBe('G1')
    expect(reconcileTarget(new IndexTargetT('beta', 'B1'), [...inventory, { ...inventory[0], name: 'beta', generation: 'B1' }])).toBe('current')
  })
  it('requires a named confirmation and an exact incarnation, and prevents duplicates', () => {
    expect(canDelete(old, '', false)).toBe(false)
    expect(canDelete(old, 'beta', false)).toBe(false)
    expect(canDelete(old, 'alpha', true)).toBe(false)
    expect(canDelete(new IndexTargetT('alpha'), 'alpha', false)).toBe(false)
    expect(canDelete(old, 'alpha', false)).toBe(true)
  })
  it('maps lifecycle failure details to safe actions', () => {
    const stale = new SearchKernelClientError('stale', { code: 'failed_precondition', details: [{ code: 'stale_index_handle' }] })
    expect(failureAction(stale)).toContain('Continuing with this handle is unsafe')
    expect(failureAction(new SearchKernelClientError('offline', { origin: 'transport' }))).toContain('unknown outcome')
    expect(failureAction(new SearchKernelClientError('invalid', { code: 'invalid_argument' }))).toContain('definition editor')
    expect(failureAction(new SearchKernelClientError('exists', { code: 'already_exists' }))).toContain('existing index')
    expect(failureAction(new SearchKernelClientError('absent', { code: 'not_found' }))).toContain('open it')
  })
})
