#!/usr/bin/env python3
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
        runtime_log = temp / "runtime.log"
        output = temp / "output.jsonl"
        target_dir = temp / "cargo-target"
        existing_runtime = temp / "existing-runtime"
        existing_runtime.mkdir()

        fake_cargo = fake_bin / "cargo"
        fake_cargo.write_text(
            "#!/usr/bin/env python3\n"
            "import json, os, sys\n"
            "from pathlib import Path\n"
            "args = sys.argv[1:]\n"
            "Path(os.environ['FAKE_CARGO_LOG']).write_text(' '.join(args) + '\\n', encoding='utf-8')\n"
            "target = Path(os.environ['CARGO_TARGET_DIR']).resolve()\n"
            "binary = target / 'debug/examples/searchkernel-m3-conformance-driver'\n"
            "binary.parent.mkdir(parents=True, exist_ok=True)\n"
            "binary.write_text(\n"
            "    '#!/usr/bin/env python3\\n'\n"
            "    'import json, os, sys\\n'\n"
            "    'from pathlib import Path\\n'\n"
            "    \"Path(os.environ['FAKE_RUNTIME_LOG']).write_text(os.environ.get('LD_LIBRARY_PATH', '') + '\\\\n', encoding='utf-8')\\n\"\n"
            "    \"for raw in sys.stdin:\\n\"\n"
            "    \"    case_id = raw.split('\\\\t', 1)[0]\\n\"\n"
            "    \"    if case_id:\\n\"\n"
            "    \"        print(json.dumps({'id': case_id, 'terms': []}, separators=(',', ':')))\\n\",\n"
            "    encoding='utf-8',\n"
            ")\n"
            "binary.chmod(0o755)\n"
            "print('normal example build')\n",
            encoding="utf-8",
        )
        fake_cargo.chmod(0o755)

        env = os.environ.copy()
        env.update(
            {
                "PATH": f"{fake_bin}:{env['PATH']}",
                "FAKE_CARGO_LOG": str(cargo_log),
                "FAKE_RUNTIME_LOG": str(runtime_log),
                "CARGO_TARGET_DIR": str(target_dir),
                "LD_LIBRARY_PATH": str(existing_runtime),
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
        runtime_path = runtime_log.read_text(encoding="utf-8").strip()

        checks = (
            (cargo_args.startswith("build --quiet "), cargo_args),
            ("-p searchkernel-vespa-engine --example searchkernel-m3-conformance-driver" in cargo_args, cargo_args),
            ("--manifest-path" not in cargo_args and "--target-dir" not in cargo_args, cargo_args),
            (runtime_path == str(existing_runtime), runtime_path),
            (bool(output.read_text(encoding="utf-8").strip()), "empty run-rust output"),
        )
        for ok, message in checks:
            if not ok:
                raise SystemExit(f"M3 run-rust regression failed: {message}")

    print("M3 normal-workspace Cargo example regression PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
