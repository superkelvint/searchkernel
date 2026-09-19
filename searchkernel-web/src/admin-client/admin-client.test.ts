import { describe, expect, it } from 'vitest'
import { createFixtureSearchKernelClient } from '../api/fixture-searchkernel-client'
import { createFixtureAdminClient } from './fixture-admin-client'

describe('AdminClient boundary', () => {
  it('keeps operational data behind the admin client', async () => {
    const admin = createFixtureAdminClient(createFixtureSearchKernelClient())
    expect(await admin.getOverview()).toMatchObject({ indexCount: 3, daemon: 'Disconnected' })
    expect(await admin.getHealth('products')).toMatchObject({ status: 'Fixture state', latency: '3.7 ms' })
    expect(await admin.getDiagnostics('missing')).toBeUndefined()
  })
})
