#!/usr/bin/env python3
"""Drive first-party decoders over the canonical production result bytes."""

from __future__ import annotations

import importlib.util
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
FACET_FIXTURES = ROOT / "core/crates/searchkernel-dispatch/tests/fixtures/portable/responses"
SCHEMA_CHECK = ROOT / "tools/check-result-response-schema.py"

_spec = importlib.util.spec_from_file_location("result_response_schema", SCHEMA_CHECK)
assert _spec is not None and _spec.loader is not None
_schema = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_schema)


def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(command), flush=True)
    return subprocess.run(command, cwd=ROOT, check=True, text=True, **kwargs)


def require_outputs(
    outputs: dict[str, dict[str, object]],
    cases: list[dict[str, str]],
    languages: tuple[str, ...],
) -> None:
    required = {case["id"] for case in cases}
    for language in languages:
        actual = set(outputs.get(language, {})) - {"malformed_outcomes"}
        if actual != required:
            raise SystemExit(
                f"{language} result trace coverage mismatch: "
                f"missing={sorted(required - actual)}, extra={sorted(actual - required)}"
            )


def compare(
    outputs: dict[str, dict[str, object]],
    cases: list[dict[str, str]],
    *,
    validate_shapes: bool = True,
) -> None:
    languages = ("rust", "python", "typescript", "php", "ruby")
    require_outputs(outputs, cases, languages)

    for case in cases:
        scenario = case["id"]
        baseline = outputs["rust"][scenario]
        if validate_shapes:
            for language in languages:
                errors = _schema.trace_shape_errors(outputs[language][scenario])
                if errors:
                    raise SystemExit(
                        f"{language} trace shape mismatch for {scenario}: " + "; ".join(errors)
                    )
        for language in languages[1:]:
            client_trace = outputs[language][scenario]
            if client_trace != baseline:
                print(
                    json.dumps(
                        {
                            "scenario": scenario,
                            "language": language,
                            "rust": baseline,
                            "client": client_trace,
                        },
                        indent=2,
                        sort_keys=True,
                    ),
                    file=sys.stderr,
                )
                raise SystemExit(f"normalized result trace mismatch: {scenario} ({language})")


def verify_malformed(
    outputs: dict[str, dict[str, object]],
    malformed: list[dict[str, object]],
) -> None:
    expected_ids = {str(case["id"]) for case in malformed}
    bridge_rejected = {
        str(case["id"]): bool(case["bridge_rejected"]) for case in malformed
    }
    for language in ("rust", "python", "typescript", "php", "ruby"):
        outcomes = outputs[language].get("malformed_outcomes")
        if not isinstance(outcomes, dict):
            raise SystemExit(f"{language} malformed outcome map is missing")
        if set(outcomes) != expected_ids:
            raise SystemExit(
                f"{language} malformed coverage mismatch: "
                f"missing={sorted(expected_ids - set(outcomes))}, "
                f"extra={sorted(set(outcomes) - expected_ids)}"
            )
        for scenario, stage in outcomes.items():
            expected_stage = (
                "bridge"
                if language in {"php", "ruby"} and bridge_rejected[scenario]
                else "client"
            )
            if stage != expected_stage:
                raise SystemExit(
                    f"{language} malformed outcome mismatch for {scenario}: "
                    f"expected {expected_stage}, got {stage!r}"
                )


def main() -> int:
    corpus = json.loads(CORPUS.read_text())
    valid = corpus["valid"]
    malformed = corpus["malformed"]
    ids = [case["id"] for case in valid + malformed]
    if len(ids) != len(set(ids)) or not ids:
        raise SystemExit("result response scenario IDs must be unique and non-empty")

    flatc = shutil.which("flatc")
    if flatc is None:
        flatc = subprocess.check_output(
            ["bash", str(ROOT / "tools/resolve-flatbuffers.sh"), "flatc"], text=True
        ).strip()

    with tempfile.TemporaryDirectory(prefix="searchkernel-result-response-") as temp:
        temporary = Path(temp)
        case_dir = temporary / "cases"
        case_dir.mkdir()

        cases = []
        official_envelopes: dict[str, dict[str, object]] = {}
        for case in valid:
            source = ROOT / case.get(
                "source",
                f"reference/result-response-conformance/fixtures/{case['file']}",
            )
            if not source.is_file():
                raise SystemExit(f"canonical result fixture missing: {source}")
            target = case_dir / case["file"]
            shutil.copyfile(source, target)
            run(
                [
                    flatc,
                    "--json",
                    "--strict-json",
                    "-I",
                    str(ROOT / "api"),
                    "-o",
                    str(case_dir),
                    str(ROOT / "api/searchkernel_response.fbs"),
                    "--",
                    str(target),
                ]
            )
            json_name = target.with_suffix(".json").name
            envelope = json.loads((case_dir / json_name).read_text())
            if not isinstance(envelope, dict):
                raise SystemExit(f"official JSON bridge produced non-object: {case['id']}")
            official_envelopes[case["id"]] = envelope
            cases.append({"id": case["id"], "file": case["file"], "json": json_name})

        coverage_errors = _schema.verify_fixture_coverage(official_envelopes)
        coverage_errors.extend(
            _schema.verify_partitions(json.loads(_schema.MANIFEST.read_text()), official_envelopes)
        )
        if coverage_errors:
            raise SystemExit("result fixture field coverage failed: " + "; ".join(coverage_errors))

        malformed_cases = []
        for case in malformed:
            source = FIXTURES / case["file"]
            if not source.is_file():
                raise SystemExit(f"canonical malformed result fixture missing: {source}")
            target = case_dir / case["file"]
            shutil.copyfile(source, target)
            result = subprocess.run(
                [
                    flatc,
                    "--json",
                    "--strict-json",
                    "-I",
                    str(ROOT / "api"),
                    "-o",
                    str(case_dir),
                    str(ROOT / "api/searchkernel_response.fbs"),
                    "--",
                    str(target),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            malformed_cases.append(
                {
                    "id": case["id"],
                    "file": case["file"],
                    "json": target.with_suffix(".json").name
                    if result.returncode == 0
                    else None,
                    "bridge_rejected": result.returncode != 0,
                }
            )

        facet_cases = []
        for source in sorted(FACET_FIXTURES.glob("result-facet-*.skrs")):
            target = case_dir / source.name
            shutil.copyfile(source, target)
            run(
                [
                    flatc,
                    "--json",
                    "--strict-json",
                    "-I",
                    str(ROOT / "api"),
                    "-o",
                    str(case_dir),
                    str(ROOT / "api/searchkernel_response.fbs"),
                    "--",
                    str(target),
                ]
            )
            facet_cases.append(
                {
                    "id": source.stem.removeprefix("result-"),
                    "file": source.name,
                    "json": target.with_suffix(".json").name,
                }
            )

        case_manifest = temporary / "cases.json"
        case_manifest.write_text(
            json.dumps(
                {
                    "valid": cases,
                    "malformed": malformed_cases,
                    "facet": facet_cases,
                }
            )
        )
        traces_dir = temporary / "traces"
        traces_dir.mkdir()
        env = os.environ.copy()
        env["SEARCHKERNEL_RESULT_CASES"] = str(case_manifest)
        env["SEARCHKERNEL_RESULT_CASE_DIR"] = str(case_dir)
        env["SEARCHKERNEL_RESULT_FIXTURE_DIR"] = str(FIXTURES)
        env["SEARCHKERNEL_RESULT_TRACE_DIR"] = str(traces_dir)
        env["PYTHONPATH"] = str(ROOT / "clients/python")

        run(
            [
                "cargo",
                "test",
                "-p",
                "searchkernel-testkit",
                "result_response_wire_fixtures_reject_malformed_ir_inputs",
                "--",
                "--nocapture",
            ],
            env=env,
        )
        run(
            [
                "cargo",
                "test",
                "-p",
                "searchkernel",
                "--no-default-features",
                "response_byte_fixtures_reach_the_public_rust_result_projection",
                "--",
                "--nocapture",
            ],
            env=env,
        )
        run(
            [
                sys.executable,
                "-m",
                "unittest",
                "clients.python.tests.test_result_response_conformance",
            ],
            env=env,
        )
        run(
            [
                "npm",
                "--prefix",
                "clients/typescript",
                "test",
                "--",
                "tests/result-response-conformance.test.ts",
            ],
            env=env,
        )
        run(["php", "clients/php/tests/result_response_conformance.php"], env=env)
        run(["ruby", "clients/ruby/test/result_response_conformance.rb"], env=env)

        outputs: dict[str, dict[str, object]] = {}
        for language in ("rust", "python", "typescript", "php", "ruby"):
            path = traces_dir / f"{language}.json"
            if not path.is_file():
                raise SystemExit(f"{language} result trace output missing")
            value = json.loads(path.read_text())
            if not isinstance(value, dict):
                raise SystemExit(f"{language} result trace file must contain an object")
            outputs[language] = value

        compare(outputs, valid)
        verify_malformed(outputs, malformed_cases)

    print(
        "cross-language canonical SearchResult response conformance PASS "
        f"({len(valid)} scenarios; {len(malformed)} malformed cases)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
