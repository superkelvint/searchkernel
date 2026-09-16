#!/usr/bin/env python3
"""Frozen Python C7 transport probe.

The production repository supplies `clients/python/tests/c7_adapter.py`.
That adapter must import and delegate to production NativeTransport; it may not
implement FFI itself.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path


def load_adapter(repo: Path):
    path = repo / "clients/python/tests/c7_adapter.py"
    if not path.exists():
        raise RuntimeError(f"missing C7 Python adapter: {path}")
    spec = importlib.util.spec_from_file_location("searchkernel_c7_adapter", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    for name in ("open_transport", "expected_abi_version", "real_c5_smoke"):
        if not hasattr(mod, name):
            raise RuntimeError(f"{path} must define {name}()")
    return mod


def make_bytes(case: dict) -> bytes:
    if "request_hex" in case:
        return bytes.fromhex(case["request_hex"])
    if case.get("request_pattern") == "byte_ramp":
        n = int(case["request_len"])
        return bytes(i & 0xFF for i in range(n))
    raise RuntimeError(f"unknown case: {case}")


def expect_reverse(request: bytes, response: bytes, case_id: str):
    expected = request[::-1]
    if response != expected:
        raise AssertionError(
            f"{case_id}: response byte mismatch: got {len(response)} bytes, "
            f"expected {len(expected)}"
        )


def run_plumbing(repo: Path, library: str, scenarios: Path):
    adapter = load_adapter(repo)
    cases = json.loads(scenarios.read_text())["cases"]

    # 1 open, 5 calls. Tests ordinary byte fidelity and explicit close.
    transport = adapter.open_transport(library, b"")
    try:
        for case in cases:
            request = make_bytes(case)
            response = transport.call(request)
            if type(response) is not bytes:
                raise AssertionError(
                    f"{case['id']}: call() must return Python-owned bytes, "
                    f"got {type(response)!r}"
                )
            expect_reverse(request, response, case["id"])
            # NativeTransport.call() must have completed native free before
            # returning. Reading/copying here must therefore be independent of
            # the native allocation.
            if bytes(response) != request[::-1]:
                raise AssertionError(f"{case['id']}: response invalid after native free")
    finally:
        transport.close()
        transport.close()  # must not enter native close twice

    # Must be rejected locally; exact trace counts detect accidental native entry.
    try:
        transport.call(b"after-close")
    except Exception:
        pass
    else:
        raise AssertionError("call after close unexpectedly succeeded")

    # 2 opens, 3 calls. Independent native handles.
    a = adapter.open_transport(library, b"a")
    b = adapter.open_transport(library, b"b")
    try:
        expect_reverse(b"one", a.call(b"one"), "handle-a")
        expect_reverse(b"two", b.call(b"two"), "handle-b")
        a.close()
        expect_reverse(b"still-live", b.call(b"still-live"), "handle-b-after-a-close")
    finally:
        a.close()  # idempotent
        b.close()

    # 1 open, 1 native call. FAIL is reserved by the verifier shim and returns
    # a native transport status failure with no canonical response allocation.
    t = adapter.open_transport(library, b"")
    try:
        try:
            t.call(b"FAIL")
        except Exception:
            pass
        else:
            raise AssertionError("native call failure did not raise")
    finally:
        t.close()

    # 1 open, 1 call. Context-manager exit must deterministically close.
    ctx = adapter.open_transport(library, b"context")
    with ctx as entered:
        if entered is not ctx:
            raise AssertionError("__enter__ must return the transport instance")
        expect_reverse(b"context-call", entered.call(b"context-call"), "context-manager")
    ctx.close()  # must remain idempotent after __exit__
    try:
        ctx.call(b"context-after-close")
    except Exception:
        pass
    else:
        raise AssertionError("context-manager did not close transport")

    # 1 open, exactly 5000 calls. Ownership/leak stress.
    t = adapter.open_transport(library, b"")
    try:
        for i in range(5000):
            req = i.to_bytes(8, "little") + b"c7"
            expect_reverse(req, t.call(req), f"stress-{i}")
    finally:
        t.close()

    print(json.dumps({"language": "python", "mode": "plumbing", "status": "PASS"}))


def run_abi_mismatch(repo: Path, library: str):
    adapter = load_adapter(repo)
    try:
        adapter.open_transport(library, b"")
    except Exception:
        print(json.dumps({"language": "python", "mode": "abi-mismatch", "status": "PASS"}))
        return
    raise AssertionError("ABI mismatch was not rejected")


def run_missing_library(repo: Path, library: str):
    adapter = load_adapter(repo)
    try:
        adapter.open_transport(library, b"")
    except Exception:
        print(json.dumps({"language": "python", "mode": "missing-library", "status": "PASS"}))
        return
    raise AssertionError("missing/unloadable native library was not rejected")


def run_real_c5_smoke(repo: Path, library: str):
    adapter = load_adapter(repo)
    adapter.real_c5_smoke(library)
    print(json.dumps({"language": "python", "mode": "real-c5-smoke", "status": "PASS"}))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", type=Path, required=True)
    ap.add_argument("--library", required=True)
    ap.add_argument(
        "--mode",
        choices=("plumbing", "abi-mismatch", "missing-library", "real-c5-smoke"),
        required=True,
    )
    ap.add_argument("--scenarios", type=Path)
    args = ap.parse_args()

    repo = args.repo.resolve()
    if args.mode == "plumbing":
        if not args.scenarios:
            raise SystemExit("--scenarios is required for plumbing")
        run_plumbing(repo, args.library, args.scenarios)
    elif args.mode == "abi-mismatch":
        run_abi_mismatch(repo, args.library)
    elif args.mode == "missing-library":
        run_missing_library(repo, args.library)
    else:
        run_real_c5_smoke(repo, args.library)


if __name__ == "__main__":
    main()
