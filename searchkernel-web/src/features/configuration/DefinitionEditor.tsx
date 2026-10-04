/** Shared canonical definition editor; validation belongs to the daemon resolver. */
export function DefinitionEditor({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return <label className="block">Canonical IndexDefinition<textarea aria-label="Canonical IndexDefinition" className="input mt-2 min-h-72 font-mono text-xs" value={value} onChange={event => onChange(event.target.value)}/></label>
}
