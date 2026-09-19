import { Component, type ErrorInfo, type ReactNode } from 'react'

type Props = { children: ReactNode }
type State = { error?: Error }

export class AppErrorBoundary extends Component<Props, State> {
  state: State = {}

  static getDerivedStateFromError(error: Error): State { return { error } }

  componentDidCatch(error: Error, info: ErrorInfo) { console.error('Search Kernel dashboard crashed.', error, info) }

  render() {
    if (!this.state.error) return this.props.children
    return <div className="grid min-h-screen place-items-center bg-slate-50 p-6"><main className="surface max-w-lg p-7" role="alert"><p className="kicker">Application error</p><h1 className="mt-1 text-xl font-semibold">The dashboard could not render</h1><p className="mt-2 text-sm text-slate-600">{this.state.error.message}</p><button className="primary-button mt-5" onClick={() => window.location.reload()}>Reload dashboard</button></main></div>
  }
}
