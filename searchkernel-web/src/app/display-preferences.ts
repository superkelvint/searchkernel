export type DisplayFormat = 'text' | 'number' | 'boolean' | 'date' | 'json'
export type DashboardFieldDisplay = { label: string; visible: boolean; order: number; format: DisplayFormat }
export type DashboardFieldDisplayOverride = Partial<DashboardFieldDisplay>
export type DashboardDisplayPreferences = {
  connectionId: string
  indexes: Record<string, Record<string, DashboardFieldDisplayOverride>>
}

/** Local dashboard metadata; a generated client must not carry presentation policy. */
export const dashboardDisplayPreferences: DashboardDisplayPreferences = {
  connectionId: 'local-preview',
  indexes: {},
}

export function displayPreferencesFor(indexId: string, preferences = dashboardDisplayPreferences) {
  return preferences.indexes[indexId] ?? {}
}
