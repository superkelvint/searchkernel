#!/usr/bin/env python3
"""Run canonical SearchKernel Clippy profiles and emit agent-friendly diagnostics."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

PROFILES = {
    "rust-clippy-non-native": [
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--no-default-features",
        "--message-format=json-diagnostic-rendered-ansi",
        "--",
        "-D",
        "warnings",
    ],
    "rust-clippy-all-features": [
        "cargo",
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--message-format=json-diagnostic-rendered-ansi",
        "--",
        "-D",
        "warnings",
    ],
}


def analyze_compiler_message(message: dict[str, Any]) -> tuple[int, int, int]:
    diagnostic = message.get("message")
    if not isinstance(diagnostic, dict):
        return (0, 0, 0)

    level = diagnostic.get("level")
    actionable = int(level in {"warning", "error"})
    code_obj = diagnostic.get("code")
    code = code_obj.get("code") if isinstance(code_obj, dict) else None
    clippy = int(actionable and isinstance(code, str) and code.startswith("clippy::"))

    suggestions = 0
    for span in diagnostic.get("spans") or []:
        if (
            isinstance(span, dict)
            and span.get("suggested_replacement") is not None
            and span.get("suggestion_applicability") == "MachineApplicable"
        ):
            suggestions += 1
    return (actionable, clippy, suggestions)


def classify(returncode: int, clippy_diagnostics: int) -> str:
    if returncode == 0:
        return "PASS"
    if clippy_diagnostics:
        return "CLIPPY_LINT_FAILURE"
    return "BUILD_OR_COMPILE_FAILURE"


def render_summary(
    *,
    profile: str,
    result: str,
    diagnostics: int,
    clippy_diagnostics: int,
    suggestions: int,
) -> str:
    target = os.environ.get("CARGO_TARGET_DIR", "<Cargo default>")
    build_env = os.environ.get("SEARCHKERNEL_BUILD_ENV", "host")
    lines = [
        "## SearchKernel Clippy",
        "",
        f"- Result: \`{result}\`",
        f"- Profile: \`{profile}\`",
        f"- Diagnostics: {diagnostics}",
        f"- Clippy diagnostics: {clippy_diagnostics}",
        f"- Machine-applicable suggestions: {suggestions}",
        f"- Reproduce: \`./dev lint --profile {profile}\`",
        f"- CARGO_TARGET_DIR: \`{target}\`",
        f"- SEARCHKERNEL_BUILD_ENV: \`{build_env}\`",
    ]
    if result == "BUILD_OR_COMPILE_FAILURE":
        lines += [
            "",
            "No Clippy lint diagnostic was observed before failure.",
            "Run \`./dev doctor\` before changing product code, then rerun the reproduction command.",
        ]
    return "\n".join(lines) + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(PROFILES), required=True)
    parser.add_argument(
        "--diagnostics-dir",
        type=Path,
        help="Write clippy.log, clippy-diagnostics.jsonl, and summary.md here.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    command = PROFILES[args.profile]

    diagnostics_dir: Path | None = args.diagnostics_dir
    human_log = None
    json_log = None
    if diagnostics_dir is not None:
        diagnostics_dir.mkdir(parents=True, exist_ok=True)
        human_log = (diagnostics_dir / "clippy.log").open("w", encoding="utf-8")
        json_log = (diagnostics_dir / "clippy-diagnostics.jsonl").open(
            "w", encoding="utf-8"
        )

    diagnostics = 0
    clippy_diagnostics = 0
    suggestions = 0

    def emit(text: str) -> None:
        sys.stdout.write(text)
        sys.stdout.flush()
        if human_log is not None:
            human_log.write(text)
            human_log.flush()

    emit(f"SearchKernel Clippy profile: {args.profile}\n")
    emit(f"Reproduce: ./dev lint --profile {args.profile}\n")

    try:
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        assert process.stdout is not None
        for line in process.stdout:
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                emit(line)
                continue

            if json_log is not None:
                json_log.write(json.dumps(message, separators=(",", ":")) + "\n")

            if message.get("reason") != "compiler-message":
                continue

            diagnostic = message.get("message")
            if isinstance(diagnostic, dict):
                rendered = diagnostic.get("rendered")
                if isinstance(rendered, str):
                    emit(rendered)
                else:
                    emit(str(diagnostic.get("message", "compiler diagnostic")) + "\n")

            add_diagnostics, add_clippy, add_suggestions = analyze_compiler_message(message)
            diagnostics += add_diagnostics
            clippy_diagnostics += add_clippy
            suggestions += add_suggestions

        returncode = process.wait()
    finally:
        if human_log is not None:
            human_log.close()
        if json_log is not None:
            json_log.close()

    result = classify(returncode, clippy_diagnostics)
    summary = render_summary(
        profile=args.profile,
        result=result,
        diagnostics=diagnostics,
        clippy_diagnostics=clippy_diagnostics,
        suggestions=suggestions,
    )
    sys.stdout.write("\n" + summary)
    if diagnostics_dir is not None:
        (diagnostics_dir / "summary.md").write_text(summary, encoding="utf-8")

    github_summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if github_summary:
        with open(github_summary, "a", encoding="utf-8") as handle:
            handle.write(summary)

    return returncode


if __name__ == "__main__":
    raise SystemExit(main())
