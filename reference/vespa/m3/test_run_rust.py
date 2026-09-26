#!/usr/bin/env python3
import json
import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[3]
M3 = ROOT / "reference" / "vespa" / "m3"
RUN_RUST = M3 / "run-rust.sh"


def main() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        temp = Path(temp_dir)
        fake_bin = temp / "bin"
        fake_bin.mkdir()
        cargo_log = temp / "cargo.log"
        output = temp / "output.jsonl"
        target_dir = temp / "cargo-target"

        fake_cargo = fake_bin / "cargo"
        fake_cargo.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, sys\n"
            "from pathlib import Path\n"
            "Path(os.environ['FAKE_CARGO_LOG']).write_text(' '.join(sys.argv[1:]) + '\\n', encoding='utf-8')\n"
            "for raw in sys.stdin:\n"
            "    case_id = raw.split('\\t', 1)[0]\n"
            "    if case_id:\n"
            "        print(json.dumps({'id': case_id, 'terms': []}, separators=(',', ':')))\n",
            encoding="utf-8",
        )
        fake_cargo.chmod(0o755)

        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{fake_bin}:{env['PATH']}",
                "FAKE_CARGO_LOG": str(cargo_log),
                "CARGO_TARGET_DIR": str(target_dir),
            }
        )
        completed = subprocess.run(
            ["bash", str(RUN_RUST), "document", str(output)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=env,
            check=False,
        )
        if completed.returncode != 0:
            raise SystemExit(completed.stdout)

        cargo_args = cargo_log.read_text(encoding="utf-8").strip()
        expected_manifest = M3 / "rust-driver" / "Cargo.toml"
        checks = (
            (cargo_args.startswith("run --quiet "), cargo_args),
            (f"--target-dir {target_dir}" in cargo_args, cargo_args),
            (f"--manifest-path {expected_manifest}" in cargo_args, cargo_args),
            ("--bin searchkernel-m3-conformance-driver" in cargo_args, cargo_args),
            (cargo_args.endswith("-- document"), cargo_args),
            (" build " not in f" {cargo_args} ", cargo_args),
            (bool(output.read_text(encoding="utf-8").strip()), "empty run-rust output"),
        )
        for ok, message in checks:
            if not ok:
                raise SystemExit(f"M3 run-rust regression failed: {message}")

    print("M3 Cargo runtime launcher regression PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
