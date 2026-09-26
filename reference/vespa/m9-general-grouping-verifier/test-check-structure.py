#!/usr/bin/env python3
"""Regression test for the executable-source grouping structure checker."""

from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]
CHECKER = ROOT / "reference/vespa/m9-general-grouping-verifier/check-structure.sh"
NATIVE_SOURCES = (
    ROOT / "core/native/vespa/engine.cc",
    ROOT / "core/native/vespa/grouping.cc",
)


def mask_comments_and_literals(text):
    result = list(text)
    index = 0
    state = "code"
    while index < len(text):
        if state == "code":
            if text.startswith("//", index):
                result[index:index + 2] = "  "
                index += 2
                state = "line-comment"
            elif text.startswith("/*", index):
                result[index:index + 2] = "  "
                index += 2
                state = "block-comment"
            elif text[index] in ('"', "'"):
                result[index] = " "
                state = ("string", text[index])
                index += 1
            else:
                index += 1
        elif state == "line-comment":
            if text[index] == "\n":
                state = "code"
            else:
                result[index] = " "
            index += 1
        elif state == "block-comment":
            if text.startswith("*/", index):
                result[index:index + 2] = "  "
                index += 2
                state = "code"
            else:
                if text[index] != "\n":
                    result[index] = " "
                index += 1
        else:
            quote = state[1]
            if text[index] == "\\":
                result[index] = " "
                if index + 1 < len(text):
                    if text[index + 1] != "\n":
                        result[index + 1] = " "
                    index += 2
                else:
                    index += 1
            elif text[index] == quote:
                result[index] = " "
                index += 1
                state = "code"
            else:
                if text[index] != "\n":
                    result[index] = " "
                index += 1
    return "".join(result)


def body_bounds(source, signature):
    masked = mask_comments_and_literals(source)
    matches = list(re.finditer(signature, masked, re.MULTILINE))
    for match in matches:
        opening = match.end()
        while opening < len(masked) and masked[opening].isspace():
            opening += 1
        if opening >= len(masked) or masked[opening] != "{":
            continue
        depth = 0
        for index in range(opening, len(masked)):
            if masked[index] == "{":
                depth += 1
            elif masked[index] == "}":
                depth -= 1
                if depth == 0:
                    return opening, index
    raise AssertionError("group_tree implementation body not found")


def main():
    source = "\n".join(path.read_text(encoding="utf-8") for path in NATIVE_SOURCES)
    signature = (
        r"\bsk_grouping_tree_result\s+group_tree\s*\(\s*"
        r"const\s+NativeQueryState\s*&\s*query\s*,\s*"
        r"const\s+sk_grouping_plan\s*&\s*plan\s*\)\s*const\s*"
    )
    opening, closing = body_bounds(source, signature)
    comment = """
        /* Grouping GroupingLevel Group AggregationResult
           CountAggregationResult AverageAggregationResult AttributeNode
           groupSpec groupResult getChildrenSize getChild */
    """
    mutated = source[:opening + 1] + comment + source[closing:]

    actual = subprocess.run(
        [str(CHECKER)],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if actual.returncode != 0:
        raise SystemExit(
            "the structural checker must accept the production implementation:\n"
            + actual.stdout
            + actual.stderr
        )

    with tempfile.TemporaryDirectory() as directory:
        candidate = Path(directory) / "comment-only-grouping.cc"
        candidate.write_text(mutated, encoding="utf-8")
        environment = os.environ.copy()
        environment["SK_GROUPING_NATIVE_CPP"] = str(candidate)
        result = subprocess.run(
            [str(CHECKER)],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode == 0:
            raise SystemExit(
                "check-structure.sh accepted a grouping implementation replaced "
                "by a comment containing the expected Vespa tokens"
            )

        forbidden = Path(directory) / "forbidden-grouping.cc"
        forbidden.write_text(
            source[:opening + 1]
            + "\n    GroupingRequestIr forbidden_reference;\n"
            + source[opening + 1:],
            encoding="utf-8",
        )
        environment["SK_GROUPING_NATIVE_CPP"] = str(forbidden)
        result = subprocess.run(
            [str(CHECKER)],
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode == 0:
            raise SystemExit(
                "check-structure.sh accepted a .cc grouping implementation "
                "depending on C-series IR terminology"
            )

    # Published verifier images intentionally carry pinned build artifacts and
    # provenance without a full upstream Vespa source checkout. The structure
    # checker must honor the resolved VESPA_ROOT instead of hard-coding
    # ROOT/upstream/vespa, and must not require source-trace files from a
    # provenance-validated published toolchain.
    with tempfile.TemporaryDirectory() as directory:
        sandbox = Path(directory)
        checker = (
            sandbox
            / "reference/vespa/m9-general-grouping-verifier/check-structure.sh"
        )
        checker.parent.mkdir(parents=True)
        shutil.copy2(CHECKER, checker)

        native_parent = sandbox / "core/native"
        native_parent.mkdir(parents=True)
        (native_parent / "vespa").symlink_to(
            ROOT / "core/native/vespa", target_is_directory=True
        )

        published_vespa = sandbox / "published-vespa"
        published_vespa.mkdir()
        (published_vespa / ".searchkernel-vespa-artifact.json").write_text(
            "{}\\n", encoding="utf-8"
        )

        environment = os.environ.copy()
        environment["VESPA_ROOT"] = str(published_vespa)
        environment.pop("SK_GROUPING_NATIVE_CPP", None)
        result = subprocess.run(
            [str(checker)],
            cwd=sandbox,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            raise SystemExit(
                "check-structure.sh rejected a published Vespa toolchain "
                "without a full source checkout:\\n"
                + result.stdout
                + result.stderr
            )

        unproven_vespa = sandbox / "unproven-vespa"
        unproven_vespa.mkdir()
        environment["VESPA_ROOT"] = str(unproven_vespa)
        result = subprocess.run(
            [str(checker)],
            cwd=sandbox,
            env=environment,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode == 0 or "missing pinned Vespa source-trace file" not in result.stderr:
            raise SystemExit(
                "check-structure.sh must still require source-trace files for "
                "an unproven Vespa source root"
            )

    print("check-structure source-extension, comment-only, and published-toolchain regressions passed")


if __name__ == "__main__":
    main()
