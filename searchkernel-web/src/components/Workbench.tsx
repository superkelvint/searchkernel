import { useState } from 'react'
import { normalizeClientError } from '../api/searchkernel-client'

export function CopyButton({ text, label }: { text: string; label: string }) {
  const [status, setStatus] = useState('')
  return <span><button type="button" className="soft-button" onClick={async () => { try { if (!navigator.clipboard) throw new Error(); await navigator.clipboard.writeText(text); setStatus('Copied') } catch { setStatus('Clipboard unavailable') } }}>{label}</button><span className="ml-2 text-xs" role="status">{status}</span></span>
}
export function CanonicalInspector({ label, text }: { label: string; text: string }) { return <details className="mt-3"><summary className="cursor-pointer font-semibold">{label}</summary><CopyButton label={`Copy ${label}`} text={text}/><pre aria-label={`${label} inspector`} className="mt-2 max-h-96 overflow-auto rounded bg-slate-950 p-3 font-mono text-xs text-slate-100">{text}</pre></details> }
export function errorJson(error: unknown): string {
  const value = normalizeClientError(error)
  return JSON.stringify({ origin: value.origin, code: value.code, message: value.message, retryable: value.retryable, status: value.status, details: value.details }, null, 2)
}
export function ErrorNotice({ error }: { error?: unknown }) {
  if (!error) return null
  const value = normalizeClientError(error)
  return <div role="alert" className="surface my-3 border-red-200 p-4 text-red-800"><strong>{value.origin ?? 'client'} · {value.code}</strong><p>{value.message}</p><CanonicalInspector label="Error" text={errorJson(value)}/></div>
}
