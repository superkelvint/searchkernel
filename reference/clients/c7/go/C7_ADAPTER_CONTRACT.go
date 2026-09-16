package c7adapter

// This file documents the verifier adapter shape expected at
// clients/go/c7adapter. The real adapter MUST delegate to the production
// NativeTransport and must not implement C FFI itself.

type Transport interface {
	Call([]byte) ([]byte, error)
	Close() error
}

func ExpectedABIVersion() uint32                                     { panic("contract only") }
func Open(libraryPath string, openPayload []byte) (Transport, error) { panic("contract only") }

// RealC5Smoke runs representative generated-client operations through the real
// C5 library and C3 recording/scripted backend fixture. Minimum coverage: one
// mutation-like request, one search request, and one describe/inspect request.
func RealC5Smoke(libraryPath string) error { panic("contract only") }
