#!/usr/bin/env python3
"""Run the frozen M3 Java oracle for arbitrary M4 strings.

This is harness-authoring/verification infrastructure only. Production M4 code
must use the Rust M3 implementation.
"""
import json
import subprocess
import sys
from pathlib import Path


def root() -> Path:
    return Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())


def java_command(mode: str):
    r = root()
    m3 = r / "reference/vespa/m3"
    subprocess.run([str(m3 / "oracle/build-java-oracle.sh")], check=True)
    build = m3 / "oracle/target"
    cp = f"{build / 'classes'}:{(build / 'runtime-classpath.txt').read_text().strip()}"
    return ["java", "-cp", cp, "searchkernel.m3.LinguisticsOracle", mode]


def analyze(mode: str, values, stemming="default", remove_accents=False):
    proc = subprocess.Popen(java_command(mode), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=sys.stderr, text=True)
    assert proc.stdin is not None and proc.stdout is not None
    for ident, text in values:
        line = f"{ident}\t{stemming}\t{1 if remove_accents else 0}\t{text.encode('utf-8').hex()}\n"
        proc.stdin.write(line)
    proc.stdin.close()
    out = {}
    for line in proc.stdout:
        if line.strip():
            obj = json.loads(line)
            out[obj["id"]] = obj
    rc = proc.wait()
    if rc != 0:
        raise SystemExit(rc)
    return out


if __name__ == "__main__":
    # Small CLI useful when authoring/debugging the C++ oracle.
    if len(sys.argv) != 3 or sys.argv[1] not in ("document", "query"):
        raise SystemExit(f"usage: {sys.argv[0]} document|query TEXT")
    print(json.dumps(analyze(sys.argv[1], [("x", sys.argv[2])])["x"], ensure_ascii=False))
