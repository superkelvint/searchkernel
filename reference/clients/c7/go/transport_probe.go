package main

// Frozen Go C7 transport probe.
//
// The repository supplies clients/go/c7adapter. That package must delegate to
// the production NativeTransport and must not implement FFI itself.

import (
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"os"

	c7adapter "searchkernel/clients/go/c7adapter"
)

type transportCase struct {
	ID             string `json:"id"`
	RequestHex     string `json:"request_hex"`
	RequestPattern string `json:"request_pattern"`
	RequestLen     int    `json:"request_len"`
	ResponseMode   string `json:"response_mode"`
}

type transportScenarios struct {
	Cases []transportCase `json:"cases"`
}

func caseBytes(c transportCase) ([]byte, error) {
	if c.RequestHex != "" || (c.RequestHex == "" && c.RequestPattern == "") {
		return hex.DecodeString(c.RequestHex)
	}
	if c.RequestPattern == "byte_ramp" {
		b := make([]byte, c.RequestLen)
		for i := range b {
			b[i] = byte(i)
		}
		return b, nil
	}
	return nil, fmt.Errorf("unknown case %q", c.ID)
}

func reverse(b []byte) []byte {
	out := make([]byte, len(b))
	for i := range b {
		out[i] = b[len(b)-1-i]
	}
	return out
}

func equal(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func plumbing(library, scenarioPath string) error {
	raw, err := os.ReadFile(scenarioPath)
	if err != nil {
		return err
	}
	var scenarios transportScenarios
	if err := json.Unmarshal(raw, &scenarios); err != nil {
		return err
	}

	t, err := c7adapter.Open(library, nil)
	if err != nil {
		return err
	}
	for _, c := range scenarios.Cases {
		req, err := caseBytes(c)
		if err != nil {
			return err
		}
		got, err := t.Call(req)
		if err != nil {
			return fmt.Errorf("%s: %w", c.ID, err)
		}
		if !equal(got, reverse(req)) {
			return fmt.Errorf("%s: response mismatch", c.ID)
		}
		copied := append([]byte(nil), got...)
		if !equal(copied, reverse(req)) {
			return fmt.Errorf("%s: response invalid after native free", c.ID)
		}
	}
	if err := t.Close(); err != nil {
		return err
	}
	if err := t.Close(); err != nil {
		return fmt.Errorf("idempotent close failed: %w", err)
	}
	if _, err := t.Call([]byte("after-close")); err == nil {
		return fmt.Errorf("call after close unexpectedly succeeded")
	}

	a, err := c7adapter.Open(library, []byte("a"))
	if err != nil {
		return err
	}
	b, err := c7adapter.Open(library, []byte("b"))
	if err != nil {
		return err
	}
	if got, err := a.Call([]byte("one")); err != nil || !equal(got, []byte("eno")) {
		return fmt.Errorf("handle a failed")
	}
	if got, err := b.Call([]byte("two")); err != nil || !equal(got, []byte("owt")) {
		return fmt.Errorf("handle b failed")
	}
	if err := a.Close(); err != nil {
		return err
	}
	if got, err := b.Call([]byte("still-live")); err != nil || !equal(got, reverse([]byte("still-live"))) {
		return fmt.Errorf("handle independence failed")
	}
	_ = a.Close()
	_ = b.Close()

	f, err := c7adapter.Open(library, nil)
	if err != nil {
		return err
	}
	if _, err := f.Call([]byte("FAIL")); err == nil {
		return fmt.Errorf("native failure did not surface")
	}
	_ = f.Close()

	s, err := c7adapter.Open(library, nil)
	if err != nil {
		return err
	}
	for i := 0; i < 5000; i++ {
		req := []byte{byte(i), byte(i >> 8), byte(i >> 16), byte(i >> 24), 'c', '7'}
		got, err := s.Call(req)
		if err != nil {
			return fmt.Errorf("stress %d: %w", i, err)
		}
		if !equal(got, reverse(req)) {
			return fmt.Errorf("stress %d: mismatch", i)
		}
	}
	return s.Close()
}

func main() {
	mode := flag.String("mode", "", "plumbing|abi-mismatch|real-c5-smoke")
	library := flag.String("library", "", "native library path")
	scenarios := flag.String("scenarios", "", "scenario JSON")
	flag.Parse()

	var err error
	switch *mode {
	case "plumbing":
		err = plumbing(*library, *scenarios)
	case "abi-mismatch":
		var t c7adapter.Transport
		t, err = c7adapter.Open(*library, nil)
		if err == nil {
			_ = t.Close()
			err = fmt.Errorf("ABI mismatch was not rejected")
		} else {
			err = nil
		}
	case "real-c5-smoke":
		err = c7adapter.RealC5Smoke(*library)
	default:
		err = fmt.Errorf("unknown --mode %q", *mode)
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	out, _ := json.Marshal(map[string]string{"language": "go", "mode": *mode, "status": "PASS"})
	fmt.Println(string(out))
}
