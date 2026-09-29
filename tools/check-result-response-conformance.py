#!/usr/bin/env python3
"""Drive first-party decoders over the canonical production result bytes."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "reference/result-response-conformance/corpus.json"
FIXTURES = ROOT / "reference/result-response-conformance/fixtures"


def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(command), flush=True)
    return subprocess.run(command, cwd=ROOT, check=True, text=True, **kwargs)


def require_outputs(outputs: dict[str, dict[str, object]], cases: list[dict[str, str]], languages: tuple[str, ...]) -> None:
    required = {case["id"] for case in cases}
    for language in languages:
        actual = set(outputs.get(language, {})) - {"malformed_rejected"}
        if actual != required:
            raise SystemExit(
                f"{language} result trace coverage mismatch: "
                f"missing={sorted(required - actual)}, extra={sorted(actual - required)}"
            )


def compare(outputs: dict[str, dict[str, object]], cases: list[dict[str, str]]) -> None:
    languages = ("rust", "python", "typescript", "php", "ruby")
    require_outputs(outputs, cases, languages)

    def normalized_trace(value: object) -> object:
        if not isinstance(value, str):
            raise SystemExit("client result trace must be canonical BFBS JSON text")
        envelope = json.loads(value)
        if envelope.get("payload_type") != "SearchKernel_V2_SearchResult":
            raise SystemExit("client result trace has the wrong response union")
        if not isinstance(envelope.get("payload"), dict):
            raise SystemExit("client result trace is missing SearchResult payload")
        return envelope

    for case in cases:
        scenario = case["id"]
        baseline = normalized_trace(outputs["rust"][scenario])
        for language in languages[1:]:
            client_trace = normalized_trace(outputs[language][scenario])
            if client_trace != baseline:
                print(json.dumps({"scenario": scenario, "language": language,
                                  "rust": baseline, "client": client_trace},
                                 indent=2, sort_keys=True), file=sys.stderr)
                raise SystemExit(f"normalized result trace mismatch: {scenario} ({language})")


def main() -> int:
    corpus = json.loads(CORPUS.read_text())
    valid = corpus["valid"]
    malformed = corpus["malformed"]
    all_cases = valid + malformed
    ids = [case["id"] for case in all_cases]
    if len(ids) != len(set(ids)) or not ids:
        raise SystemExit("result response scenario IDs must be unique and non-empty")

    flatc = shutil.which("flatc")
    if flatc is None:
        resolver = subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "flatc"], text=True
        ).strip()
        flatc = resolver
    with tempfile.TemporaryDirectory(prefix="searchkernel-result-response-") as temp:
        temporary = Path(temp)
        case_dir = temporary / "cases"
        case_dir.mkdir()
        cases = []
        for case in valid:
            source = ROOT / case.get("source", f"reference/result-response-conformance/fixtures/{case['file']}")
            if not source.is_file():
                raise SystemExit(f"canonical result fixture missing: {source}")
            target = case_dir / case["file"]
            shutil.copyfile(source, target)
            run([flatc, "--json", "--strict-json", "-I", str(ROOT / "api"), "-o", str(case_dir),
                 str(ROOT / "api/searchkernel_response.fbs"), "--", str(target)])
            cases.append({"id": case["id"], "file": case["file"], "json": target.with_suffix(".json").name})

        malformed_cases = []
        for case in malformed:
            source = FIXTURES / case["file"]
            if not source.is_file():
                raise SystemExit(f"canonical malformed result fixture missing: {source}")
            target = case_dir / case["file"]
            shutil.copyfile(source, target)
            result = subprocess.run(
                [flatc, "--json", "--strict-json", "-I", str(ROOT / "api"), "-o", str(case_dir),
                 str(ROOT / "api/searchkernel_response.fbs"), "--", str(target)],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            malformed_cases.append({
                "id": case["id"],
                "file": case["file"],
                "json": target.with_suffix(".json").name if result.returncode == 0 else None,
                "bridge_rejected": result.returncode != 0,
            })

        case_manifest = temporary / "cases.json"
        case_manifest.write_text(json.dumps({"valid": cases, "malformed": malformed_cases}))
        traces_dir = temporary / "traces"
        traces_dir.mkdir()
        env = os.environ.copy()
        env["SEARCHKERNEL_RESULT_CASES"] = str(case_manifest)
        env["SEARCHKERNEL_RESULT_CASE_DIR"] = str(case_dir)
        env["SEARCHKERNEL_RESULT_FIXTURE_DIR"] = str(FIXTURES)
        env["SEARCHKERNEL_RESULT_TRACE_DIR"] = str(traces_dir)
        env["PYTHONPATH"] = str(ROOT / "clients/python")

        # This unit test runs the IR response decoder against every malformed
        # wire case generated by searchkernel-testkit.
        run(["cargo", "test", "-p", "searchkernel-testkit", "result_response_wire_fixtures", "--", "--nocapture"], env=env)
        run(["cargo", "test", "-p", "searchkernel", "--no-default-features", "response_byte_fixtures_reach"], env=env)
        run([sys.executable, "-m", "unittest", "clients.python.tests.test_result_response_conformance"], env=env)
        run(["npm", "--prefix", "clients/typescript", "test", "--", "tests/result-response-conformance.test.ts"], env=env)
        run(["php", "clients/php/tests/result_response_conformance.php"], env=env)
        run(["ruby", "clients/ruby/test/result_response_conformance.rb"], env=env)

        outputs = {}
        for language in ("rust", "python", "typescript", "php", "ruby"):
            path = traces_dir / f"{language}.json"
            if not path.is_file():
                raise SystemExit(f"{language} result trace output missing")
            outputs[language] = json.loads(path.read_text())
        compare(outputs, valid)
        expected_malformed = {case["id"] for case in malformed}
        for language in ("rust", "python", "typescript", "php", "ruby"):
            rejected = set(outputs[language].get("malformed_rejected", []))
            if rejected != expected_malformed:
                raise SystemExit(f"{language} malformed-result behavior mismatch: {sorted(expected_malformed - rejected)}")

    print(f"cross-language canonical SearchResult response conformance PASS ({len(valid)} scenarios; {len(malformed)} malformed cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
