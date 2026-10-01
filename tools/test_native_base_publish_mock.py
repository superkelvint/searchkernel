#!/usr/bin/env python3
"""Exercise publication ordering with a local fake GitHub CLI boundary."""

from __future__ import annotations

import hashlib
import os
import pathlib
import shutil
import subprocess
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
VESPA_COMMIT = "2300ef0412bdb76f227ae3b497c8ca5e73c5da4e"


def executable(path: pathlib.Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")
    path.chmod(0o755)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="searchkernel-publish-mock.") as raw:
        temp = pathlib.Path(raw)
        repo = temp / "repo"
        tools = repo / "tools"
        (repo / "core/native/vespa").mkdir(parents=True)
        tools.mkdir()
        fake_bin = temp / "bin"
        fake_bin.mkdir()
        remote = temp / "remote"
        remote.mkdir()
        events = temp / "events"
        (repo / "core/native/vespa/manifest.json").write_text(
            f'{{"vespa_commit":"{VESPA_COMMIT}"}}\n', encoding="utf-8"
        )
        shutil.copy2(ROOT / "tools/publish-native-base.sh", tools)
        shutil.copy2(ROOT / "tools/remove-readonly-tree.sh", tools)
        executable(tools / "resolve-python.sh", "#!/usr/bin/env bash\ncommand -v python3\n")
        executable(tools / "resolve-rust.sh", "#!/usr/bin/env bash\ncommand -v rustc\n")
        executable(
            repo / "dev",
            f'''#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" != native-base ]]; then
    echo "unexpected dev command: $*" >&2
    exit 2
fi
case "$2" in
    build)
        build_env=$3
        out=$4
        generation=$5
        target=${{SK_NATIVE_TARGET:-x86_64-unknown-linux-gnu}}
        commit=$(git rev-parse HEAD)
        pin={VESPA_COMMIT}
        asset="searchkernel-vespa-base-v${{generation}}-${{build_env}}-${{target}}-${{pin}}.tar.gz"
        mkdir -p "$out"
        printf 'verified bytes %s %s' "$build_env" "$commit" >"$out/$asset"
        sha256sum "$out/$asset" >"$out/$asset.sha256"
        printf 'build:%s:%s\\n' "$build_env" "$(sha256sum "$out/$asset" | awk '{{print $1}}')" >>"$PUBLISH_EVENTS"
        ;;
    verify)
        hash=$(sha256sum "$3" | awk '{{print $1}}')
        printf 'verify:%s\\n' "$hash" >>"$PUBLISH_EVENTS"
        ;;
    *)
        echo "unexpected native-base dev command: $*" >&2
        exit 2
        ;;
esac
''',
        )
        executable(
            fake_bin / "gh",
            '''#!/usr/bin/env python3
import json, os, pathlib, shutil, sys
a = sys.argv[1:]
remote = pathlib.Path(os.environ["FAKE_REMOTE"])
events = pathlib.Path(os.environ["PUBLISH_EVENTS"])
head = os.environ["PUBLISH_HEAD"]
if a[0] == "api":
    if any("git/ref/heads/main" in x for x in a):
        print(head if "--jq" in a else json.dumps({"object": {"sha": head}}))
    elif any("releases?per_page=100" in x for x in a):
        pass
    else:
        print("{}")
elif a[:2] == ["release", "view"]:
    path = remote / a[2]
    if not path.is_dir():
        raise SystemExit(1)
    if "--json" in a:
        print("true" if (path / "draft").read_text().strip() == "true" else "false")
    else:
        raise SystemExit(1)
elif a[:2] == ["release", "create"]:
    tag, archive, checksum = a[2:5]
    path = remote / tag
    path.mkdir()
    shutil.copy2(archive, path / pathlib.Path(archive).name)
    shutil.copy2(checksum, path / pathlib.Path(checksum).name)
    (path / "draft").write_text("true\\n")
    events.write_text(events.read_text() + f"create:{tag}\\n" if events.exists() else f"create:{tag}\\n")
elif a[:2] == ["release", "download"]:
    tag = a[2]
    dest = pathlib.Path(a[a.index("--dir") + 1])
    dest.mkdir(parents=True, exist_ok=True)
    for path in (remote / tag).glob("*.tar.gz*"):
        shutil.copy2(path, dest / path.name)
    with events.open("a") as stream:
        stream.write(f"download:{tag}\\n")
elif a[:2] == ["release", "edit"]:
    tag = a[2]
    (remote / tag / "draft").write_text("false\\n")
    with events.open("a") as stream:
        stream.write(f"publish:{tag}\\n")
else:
    raise SystemExit("unexpected fake gh command: " + " ".join(a))
''',
        )

        subprocess.run(["git", "init", "-q", "-b", "main", str(repo)], check=True)
        subprocess.run(["git", "-C", str(repo), "config", "user.email", "test@example.com"], check=True)
        subprocess.run(["git", "-C", str(repo), "config", "user.name", "Publication Test"], check=True)
        subprocess.run(["git", "-C", str(repo), "add", "."], check=True)
        subprocess.run(["git", "-C", str(repo), "commit", "-qm", "publication fixture"], check=True)
        head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        env = os.environ.copy()
        env.update(
            PATH=str(fake_bin) + os.pathsep + env["PATH"],
            FAKE_REMOTE=str(remote),
            PUBLISH_EVENTS=str(events),
            PUBLISH_HEAD=head,
            SEARCHKERNEL_GITHUB_REPO="fixture/searchkernel",
            SK_NATIVE_TARGET="x86_64-unknown-linux-gnu",
        )
        result = subprocess.run(
            ["bash", str(tools / "publish-native-base.sh"), "host"],
            cwd=repo,
            env=env,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0:
            raise SystemExit(f"mock publisher failed:\n{result.stdout}\n{result.stderr}")
        lines = events.read_text(encoding="utf-8").splitlines()
        prefixes = ("build:", "create:", "download:", "verify:", "publish:")
        positions = [next(i for i, line in enumerate(lines) if line.startswith(prefix)) for prefix in prefixes]
        if positions != sorted(positions):
            raise SystemExit(f"publication order is unsafe: {lines}")
        built_hash = lines[positions[0]].split(":", 2)[2]
        verified_hash = lines[positions[3]].split(":", 1)[1]
        uploaded = next(remote.rglob("*.tar.gz"))
        if built_hash != verified_hash or hashlib.sha256(uploaded.read_bytes()).hexdigest() != built_hash:
            raise SystemExit("publisher did not upload and verify the canonical builder bytes")

    print("native-base publication GitHub-boundary mock: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
