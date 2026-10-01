#!/usr/bin/env python3
"""Qualify packaged release artifacts from a pristine consumer environment."""

from __future__ import annotations

import hashlib
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
import tomllib

DIST = Path(sys.argv[1]).resolve()
MANIFEST = json.loads((DIST / "manifest.json").read_text(encoding="utf-8"))
QUICKSTARTS = Path(__file__).resolve().parent / "quickstarts"

# The regression test compares this executable registry to release-support.json.
# Every supported surface/runtime pair must map to one packaged consumer probe.
QUALIFICATION_PROBES = {
    "rust-embedded/native": "rust_embedded",
    "searchkerneld/native": "searchkerneld_runtime",
    "python/managed-local": "python_managed_local",
    "python/connect": "python_connect",
    "typescript-node/managed-local": "typescript_node_managed_local",
    "typescript-node/connect": "typescript_node_connect",
    "ruby/managed-local": "ruby_managed_local",
    "ruby/connect": "ruby_connect",
}


def clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for key in list(environment):
        if key.startswith(("SEARCHKERNEL_", "SK_NATIVE_", "VESPA_")) or key in {
            "BUNDLE_GEMFILE",
            "CARGO_MANIFEST_DIR",
            "CARGO_TARGET_DIR",
            "GEM_HOME",
            "GEM_PATH",
            "GH_TOKEN",
            "GITHUB_TOKEN",
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "LD_AUDIT",
            "LD_ORIGIN_PATH",
            "LD_RUN_PATH",
            "LD_CONFIG_FILE",
            "DYLD_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
            "LIBRARY_PATH",
            "NODE_PATH",
            "PYTHONPATH",
            "RUBYLIB",
            "RUBYOPT",
        }:
            environment.pop(key, None)
    return environment


def run(
    *cmd: str,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    environment = clean_environment()
    if env:
        environment.update(env)
    result = subprocess.run(
        cmd, cwd=cwd, env=environment, check=False, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    if check and result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {cmd!r}\n{result.stdout}{result.stderr}"
        )
    return result


def artifact(prefix: str, suffix: str) -> Path:
    matches = [
        DIST / item["file"]
        for item in MANIFEST["artifacts"]
        if item["file"].startswith(prefix) and item["file"].endswith(suffix)
    ]
    if len(matches) != 1:
        raise SystemExit(f"expected one {prefix}*{suffix} artifact, got {matches}")
    return matches[0]


def verify_manifest() -> None:
    for item in MANIFEST["artifacts"]:
        path = DIST / item["file"]
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != item["sha256"]:
            raise SystemExit(f"checksum mismatch: {path.name}")


def install_consumers(root: Path) -> dict[str, Path]:
    wheel = artifact("searchkernel-", ".whl")
    venv = root / "python-venv"
    run(sys.executable, "-m", "venv", str(venv))
    run(str(venv / "bin/pip"), "install", str(wheel), cwd=root)

    npm = artifact("searchkernel-client-", ".tgz")
    node = root / "node-consumer"
    node.mkdir()
    (node / "package.json").write_text(
        '{"private":true,"type":"module"}', encoding="utf-8"
    )
    run(
        "npm", "install", "--ignore-scripts", "--no-audit", "--no-fund",
        "--package-lock=false", str(npm), cwd=node,
    )

    gem = artifact("searchkernel-", ".gem")
    gem_home = root / "ruby-gems"
    gem_home.mkdir()
    ruby_env = {"GEM_HOME": str(gem_home), "GEM_PATH": str(gem_home)}
    run("gem", "install", "--local", "--install-dir", str(gem_home),
        "--no-document", str(gem), cwd=root, env=ruby_env)

    return {"python": venv, "node": node, "ruby": gem_home}


def installed_runtime_descriptors(installs: dict[str, Path]) -> dict:
    python_descriptors = sorted(
        installs["python"].glob("lib/python*/site-packages/searchkernel/runtime.json")
    )
    node_descriptor = installs["node"] / "node_modules/@searchkernel/client/runtime.json"
    ruby_descriptors = sorted(
        installs["ruby"].glob("gems/searchkernel-*/lib/searchkernel/runtime.json")
    )
    rust_crates = [
        DIST / item["file"]
        for item in MANIFEST["artifacts"]
        if item["file"].startswith("searchkernel-vespa-bindings-")
        and item["file"].endswith(".crate")
    ]
    if len(rust_crates) != 1:
        raise RuntimeError(
            f"release must contain exactly one packaged Rust runtime descriptor crate: {rust_crates}"
        )
    with tarfile.open(rust_crates[0], "r:gz") as archive:
        members = [
            member
            for member in archive.getmembers()
            if member.isfile() and member.name.endswith("/runtime.json")
        ]
        if len(members) != 1:
            raise RuntimeError(
                f"Rust bindings crate must contain exactly one runtime.json: {[m.name for m in members]}"
            )
        stream = archive.extractfile(members[0])
        if stream is None:
            raise RuntimeError("could not read runtime.json from Rust bindings crate")
        rust_descriptor = json.loads(stream.read().decode("utf-8"))

    paths = [*python_descriptors, node_descriptor, *ruby_descriptors]
    if any(not path.is_file() for path in paths) or len(paths) != 3:
        raise RuntimeError(
            "consumer packages did not install exactly one runtime descriptor per contract: "
            + ", ".join(str(path) for path in paths)
        )
    descriptors = [
        *(json.loads(path.read_text(encoding="utf-8")) for path in paths),
        rust_descriptor,
    ]
    if len({json.dumps(descriptor, sort_keys=True) for descriptor in descriptors}) != 1:
        raise RuntimeError("Python, Node, Ruby, and Rust runtime descriptors differ")
    descriptor = descriptors[0]
    runtime = artifact("searchkernel-runtime-", ".tar.gz")
    expected_name = (
        f"searchkernel-runtime-{descriptor['runtime_identity']}-{descriptor['platform']}.tar.gz"
    )
    if runtime.name != expected_name:
        raise RuntimeError(
            f"runtime artifact {runtime.name!r} does not match installed identity {expected_name!r}"
        )
    return descriptor


def managed_local_process(
    language: str,
    installs: dict[str, Path],
    root: Path,
    cache: Path,
    origin: str | None,
    runtime_dir: Path | None,
    label: str,
) -> tuple[list[str], Path, dict[str, str]]:
    index = root / f"runtime-{label}-{language}-index"
    environment = {
        "SEARCHKERNEL_RUNTIME_CACHE": str(cache),
        "SK_INDEX": str(index),
    }
    if origin is not None:
        environment["SEARCHKERNEL_RUNTIME_ORIGIN"] = origin
    if runtime_dir is not None:
        environment["SEARCHKERNEL_RUNTIME_DIR"] = str(runtime_dir)
    if language == "python":
        code = (
            "from searchkernel import Index,Text; "
            "x=Index.create(__import__('os').environ['SK_INDEX'], "
            "fields={'title': Text(stored=True, searchable=True)}); x.close()"
        )
        return [str(installs["python"] / "bin/python"), "-c", code], root, environment
    if language == "node":
        script = root / f"runtime-{label}-node.mjs"
        script.write_text(
            'import { Index, Text } from "@searchkernel/client/node";\n'
            "const x=await Index.create(process.env.SK_INDEX,{fields:{title:new Text({stored:true,searchable:true})}});\n"
            "await x.close();\n",
            encoding="utf-8",
        )
        return ["node", str(script)], installs["node"], environment
    if language == "ruby":
        code = (
            'require "searchkernel"; '
            'x=SearchKernel::Index.create(ENV.fetch("SK_INDEX"), '
            'fields: {"title" => SearchKernel::Text.new(stored: true, searchable: true)}); '
            "x.close"
        )
        environment.update({"GEM_HOME": str(installs["ruby"]), "GEM_PATH": str(installs["ruby"])})
        return ["ruby", "-e", code], root, environment
    raise ValueError(f"unknown managed-local language {language!r}")

def run_managed_local_processes(
    languages: tuple[str, ...],
    installs: dict[str, Path],
    root: Path,
    cache: Path,
    origin: str | None,
    *,
    runtime_dir: Path | None = None,
    label: str,
    expect_success: bool = True,
) -> list[tuple[str, int, str, str]]:
    specs = [
        managed_local_process(language, installs, root, cache, origin, runtime_dir, label)
        for language in languages
    ]

    def execute(spec):
        command, cwd, overrides = spec
        environment = clean_environment()
        environment.update(overrides)
        process = subprocess.Popen(
            command,
            cwd=cwd,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        stdout, stderr = process.communicate()
        return process.returncode, stdout, stderr

    with ThreadPoolExecutor(max_workers=len(specs)) as executor:
        results = list(executor.map(execute, specs))
    outcomes = [
        (language, returncode, stdout, stderr)
        for language, (returncode, stdout, stderr) in zip(languages, results)
    ]
    failures = [outcome for outcome in outcomes if outcome[1] != 0]
    successes = [outcome for outcome in outcomes if outcome[1] == 0]
    if expect_success and failures:
        raise RuntimeError(
            f"managed-local {label} failed: "
            + "; ".join(f"{language}: {stderr or stdout}" for language, _, stdout, stderr in failures)
        )
    if not expect_success and successes:
        raise RuntimeError(
            f"managed-local {label} unexpectedly accepted the runtime: "
            + ", ".join(language for language, *_ in successes)
        )
    return outcomes

def assert_one_valid_runtime_object(
    cache: Path, descriptor_path: Path, venv: Path, identity: str
) -> Path:
    objects = cache / "objects"
    entries = sorted(objects.iterdir()) if objects.is_dir() else []
    expected = objects / identity
    if len(entries) != 1 or entries[0] != expected or not expected.is_dir():
        raise RuntimeError(
            f"shared runtime cache did not publish exactly one object: {entries}"
        )
    code = (
        "import sys; from pathlib import Path; "
        "from searchkernel._runtime import ensure_runtime, load_descriptor; "
        "ensure_runtime(load_descriptor(Path(sys.argv[1])), "
        "root=Path(sys.argv[2]), offline=True)"
    )
    run(
        str(venv / "bin/python"),
        "-c",
        code,
        str(descriptor_path),
        str(cache),
        cwd=venv.parent,
        env={"SEARCHKERNEL_RUNTIME_ORIGIN": "file:///offline-runtime"},
    )
    return expected

def run_python_quickstart(
    venv: Path, root: Path, runtime_env: dict[str, str] | None = None
) -> None:
    run(
        str(venv / "bin/python"),
        str(QUICKSTARTS / "python.py"),
        cwd=root,
        env={
            **(runtime_env or {}),
            "SEARCHKERNEL_QUICKSTART_PATH": str(root / "quickstart-python-index"),
        },
    )

def run_node_quickstart(node: Path, endpoint: str, target: str) -> None:
    script = node / "release-quickstart.mjs"
    shutil.copy2(QUICKSTARTS / "typescript.mjs", script)
    run(
        "node",
        str(script),
        cwd=node,
        env={"SEARCHKERNEL_ENDPOINT": endpoint, "SEARCHKERNEL_TARGET": target},
    )


def typescript_node_runtime_lock_probe(node: Path, root: Path) -> None:
    package = node / "node_modules/@searchkernel/client"
    descriptor = json.loads((package / "runtime.json").read_text(encoding="utf-8"))
    identity = descriptor["runtime_identity"]
    cache = root / "typescript-runtime-lock-cache"
    lock = cache / "locks" / f"{identity}.lock"
    lock.parent.mkdir(parents=True)
    holder = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import fcntl,sys,time; f=open(sys.argv[1], 'a+'); "
            "fcntl.flock(f, fcntl.LOCK_EX); print('LOCKED', flush=True); time.sleep(0.3)",
            str(lock),
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    workers: list[subprocess.Popen[str]] = []
    try:
        if holder.stdout is None or holder.stdout.readline().strip() != "LOCKED":
            raise RuntimeError("Python did not acquire the shared runtime file lock")
        resolver = package / "dist/node/runtime-resolver.js"
        script = (
            f"require({json.dumps(str(resolver))}).resolveRuntime()"
            ".then((path) => console.log(path), (error) => { console.error(error); process.exitCode = 1; })"
        )
        environment = clean_environment()
        environment.update({
            "SEARCHKERNEL_RUNTIME_CACHE": str(cache),
            "SEARCHKERNEL_RUNTIME_ORIGIN": DIST.as_uri(),
        })
        workers.extend([
            subprocess.Popen(
                ["node", "-e", script], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                text=True, env=environment,
            )
            for _ in range(2)
        ])
        holder.wait(timeout=10)
        results = [worker.communicate(timeout=120) for worker in workers]
        failures = [
            f"exit={worker.returncode} stdout={stdout!r} stderr={stderr!r}"
            for worker, (stdout, stderr) in zip(workers, results, strict=True)
            if worker.returncode != 0
        ]
        if failures:
            raise RuntimeError("Node runtime resolver failed under shared lock: " + "; ".join(failures))
        paths = [stdout.strip() for stdout, _ in results]
        expected = str(cache / "objects" / identity)
        if paths != [expected, expected]:
            raise RuntimeError(f"concurrent Node runtime resolvers returned inconsistent cache paths: {paths}")
        if not lock.is_file() or lock.is_dir():
            raise RuntimeError("runtime lock must remain a persistent file after release")
    finally:
        for worker in workers:
            if worker.poll() is None:
                worker.terminate()
                worker.wait(timeout=5)
        if holder.poll() is None:
            holder.terminate()
            holder.wait(timeout=5)


def run_ruby_quickstart(
    gem_home: Path, root: Path, endpoint: str, target: str,
) -> None:
    env = {
        "GEM_HOME": str(gem_home),
        "GEM_PATH": str(gem_home),
        "SEARCHKERNEL_ENDPOINT": endpoint,
        "SEARCHKERNEL_TARGET": target,
    }
    run("ruby", str(QUICKSTARTS / "ruby.rb"), cwd=root, env=env)


def crate_index_path(name: str) -> str:
    lower = name.lower()
    if len(lower) == 1:
        return f"1/{lower}"
    if len(lower) == 2:
        return f"2/{lower}"
    if len(lower) == 3:
        return f"3/{lower[0]}/{lower}"
    return f"{lower[:2]}/{lower[2:4]}/{lower}"


def cargo_index_record(crate: Path) -> tuple[str, bytes, str, str]:
    with tarfile.open(crate, "r:gz") as archive:
        manifest_member = next(
            member for member in archive.getmembers()
            if member.isfile() and member.name.endswith("/Cargo.toml")
        )
        manifest_stream = archive.extractfile(manifest_member)
        if manifest_stream is None:
            raise RuntimeError(f"missing Cargo.toml in packaged crate {crate.name}")
        manifest = tomllib.loads(manifest_stream.read().decode("utf-8"))
    package = manifest["package"]
    name, version = package["name"], package["version"]
    dependencies: list[dict[str, object]] = []
    for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
        for alias, value in manifest.get(kind, {}).items():
            dependency = {"version": value} if isinstance(value, str) else value
            dependencies.append({
                "name": dependency.get("package", alias),
                "req": dependency.get("version", "*"),
                "features": dependency.get("features", []),
                "optional": dependency.get("optional", False),
                "default_features": dependency.get("default-features", True),
                "target": None,
                "kind": {"dependencies": "normal", "build-dependencies": "build", "dev-dependencies": "dev"}[kind],
                "registry": None,
                "package": dependency.get("package"),
            })
    for target, sections in manifest.get("target", {}).items():
        for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
            for alias, value in sections.get(kind, {}).items():
                dependency = {"version": value} if isinstance(value, str) else value
                dependencies.append({
                    "name": dependency.get("package", alias),
                    "req": dependency.get("version", "*"),
                    "features": dependency.get("features", []),
                    "optional": dependency.get("optional", False),
                    "default_features": dependency.get("default-features", True),
                    "target": target,
                    "kind": {"dependencies": "normal", "build-dependencies": "build", "dev-dependencies": "dev"}[kind],
                    "registry": None,
                    "package": dependency.get("package"),
                })
    features = manifest.get("features", {})
    record: dict[str, object] = {
        "name": name,
        "vers": version,
        "deps": dependencies,
        "cksum": hashlib.sha256(crate.read_bytes()).hexdigest(),
        "features": features,
        "yanked": False,
    }
    if package.get("links"):
        record["links"] = package["links"]
    if package.get("rust-version"):
        record["rust_version"] = package["rust-version"]
    if any(any(token.startswith("dep:") for token in values) for values in features.values()):
        record["features"] = {}
        record["features2"] = features
        record["v"] = 2
    index = (json.dumps(record, separators=(",", ":")) + "\n").encode()
    return name, index, version, crate.name


class LocalCargoRegistry:
    """Expose generated .crate artifacts while proxying normal crates.io reads."""

    def __init__(self, crates: list[Path]):
        self.packages: dict[tuple[str, str], Path] = {}
        self.index: dict[str, bytes] = {}
        for crate in crates:
            name, record, version, _ = cargo_index_record(crate)
            self.packages[(name.lower(), version)] = crate
            path = crate_index_path(name)
            self.index[path] = self.index.get(path, b"") + record
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:
                path = self.path.split("?", 1)[0].lstrip("/")
                if path == "config.json":
                    body = json.dumps({"dl": f"http://127.0.0.1:{server.server_port}/crates/{{crate}}/{{version}}/download"}).encode()
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                if path.startswith("crates/") and path.endswith("/download"):
                    parts = path.split("/")
                    key = (parts[1].lower(), parts[2])
                    local = owner.packages.get(key)
                    if local is not None:
                        self._send(local.read_bytes())
                        return
                    self._proxy(f"https://static.crates.io/crates/{parts[1]}/{parts[1]}-{parts[2]}.crate")
                    return
                local_index = owner.index.get(path)
                if local_index is not None:
                    self._send(local_index)
                    return
                self._proxy(f"https://index.crates.io/{path}")

            def _send(self, body: bytes) -> None:
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def _proxy(self, url: str) -> None:
                try:
                    with urlopen(Request(url, headers={"User-Agent": "searchkernel-consumer-qualifier"}), timeout=60) as response:
                        self._send(response.read())
                except HTTPError as error:
                    self.send_error(error.code)
                except (OSError, URLError):
                    self.send_error(502, "crates.io proxy request failed")

            def log_message(self, _format: str, *_args: object) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server = server
        self.thread = threading.Thread(target=server.serve_forever, daemon=True)

    def __enter__(self) -> "LocalCargoRegistry":
        self.thread.start()
        return self

    def __exit__(self, *_args: object) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()


def run_rust_quickstart(root: Path) -> None:
    crates = [DIST / item["file"] for item in MANIFEST["artifacts"] if item["file"].endswith(".crate")]
    if not crates:
        raise RuntimeError("release does not contain publishable Cargo .crate artifacts")
    project = root / "external-rust-consumer"
    project.mkdir()
    cargo_home = root / "external-cargo-home"
    cargo_home.mkdir()
    quickstart = QUICKSTARTS / "rust"
    with LocalCargoRegistry(crates) as registry:
        port = registry.server.server_port
        (cargo_home / "config.toml").write_text(
            "[source.crates-io]\nreplace-with = \"consumer-artifacts\"\n\n"
            "[source.consumer-artifacts]\n"
            f"registry = \"sparse+http://127.0.0.1:{port}/\"\n",
            encoding="utf-8",
        )
        run("cargo", "init", "--bin", "--name", "searchkernel_release_consumer", "--vcs", "none", cwd=project)
        shutil.copy2(quickstart / "Cargo.toml", project / "Cargo.toml")
        shutil.copy2(quickstart / "src/main.rs", project / "src/main.rs")
        environment = {
            "CARGO_HOME": str(cargo_home),
            "CARGO_TARGET_DIR": str(root / "external-cargo-target"),
            "SEARCHKERNEL_QUICKSTART_PATH": str(root / "quickstart-rust-index"),
            # The test registry supplies the exact .crate files; the runtime
            # origin supplies the matching immutable native artifact.
            "SEARCHKERNEL_RUNTIME_ORIGIN": DIST.as_uri(),
            "SEARCHKERNEL_RUNTIME_CACHE": str(root / "rust-runtime-cache"),
        }
        run("cargo", "add", "searchkernel", cwd=project, env=environment)
        run("cargo", "add", "serde_json", cwd=project, env=environment)
        run("cargo", "build", cwd=project, env=environment)
        run("cargo", "run", "--quiet", cwd=project, env=environment)


def python_managed_local(
    venv: Path, index: Path, runtime_env: dict[str, str] | None = None
) -> None:
    code = r'''from searchkernel import Index,Text,Keyword,Integer,Tensor,TensorField,HnswIndexConfig,RankProfile,q
p=r"''' + str(index) + r'''"
fields={"title":Text(stored=True,searchable=True),"kind":Keyword(stored=True,filterable=True,sortable=True),"score":Integer(stored=True,filterable=True,sortable=True),"embedding":TensorField("tensor<float>(x[2])",stored=True,distance_metric="angular",hnsw=HnswIndexConfig())}
profiles=[RankProfile("semantic").first_phase("closeness(embedding)").query_input("query_embedding","tensor<float>(x[2])")]
with Index.create(p,fields=fields,rank_profiles=profiles) as x:
 x.insert({"_id":"managed-a","title":"alpha running shoe","kind":"shoe","score":2,"embedding":Tensor.dense("tensor<float>(x[2])",[1,0])})
 x.insert({"_id":"managed-b","title":"beta running shoe","kind":"shoe","score":1,"embedding":Tensor.dense("tensor<float>(x[2])",[0,1])})
 x.insert({"_id":"managed-deleted","title":"deleted running shoe","kind":"shoe","score":0,"embedding":Tensor.dense("tensor<float>(x[2])",[.5,.5])})
 assert {f.name for f in x.describe().index.fields}>={"title","kind","score","embedding"}
 assert x.get("managed-a")["score"]==2
 x.update("managed-a").set("score",3).set("title","persisted alpha running shoe").execute()
 assert x.get("managed-a")["score"]==3
 assert [h.id for h in x.query(q.contains("title","running")).filter("kind = :k",{"k":"shoe"}).sort("score","asc").execute().hits]==["managed-deleted","managed-b","managed-a"]
 result=x.query(q.nearest_neighbor("embedding","query_embedding",target_hits=3)).ranking("semantic").input("query_embedding",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(3).execute()
 assert result.hits[0].id=="managed-a"
 assert x.delete("managed-deleted") is True
 assert x.get("managed-deleted") is None
with Index.open(p) as x:
 assert x.get("managed-a")["score"]==3 and x.get("managed-a")["title"]=="persisted alpha running shoe"
 assert x.get("managed-deleted") is None
 assert [h.id for h in x.query(q.contains("title","running")).filter("kind = :k",{"k":"shoe"}).sort("score","asc").execute().hits]==["managed-b","managed-a"]
 assert [h.id for h in x.query(q.contains("title","persisted")).execute().hits]==["managed-a"]
 result=x.query(q.nearest_neighbor("embedding","query_embedding",target_hits=2)).ranking("semantic").input("query_embedding",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(2).execute()
 assert result.hits[0].id=="managed-a"
'''
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent, env=runtime_env)

def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def daemon_log(log) -> str:
    try:
        log.flush()
        log.seek(0)
        return log.read()
    except (OSError, ValueError):
        return ""


def start_multi_daemon(
    binary: Path, data_root: Path, socket_path: Path, port: int,
):
    log = tempfile.TemporaryFile(mode="w+t", encoding="utf-8")
    proc = subprocess.Popen(
        [
            str(binary),
            "--data-root",
            str(data_root),
            "--listen",
            f"unix://{socket_path}",
            "--web-listen",
            f"127.0.0.1:{port}",
        ],
        env=clean_environment(),
        stdout=log,
        stderr=subprocess.STDOUT,
        text=True,
    )
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            output = daemon_log(log)
            log.close()
            raise RuntimeError(
                "packaged multi-index daemon exited"
                + (f":\n{output}" if output else "")
            )
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return proc, log
        except OSError:
            time.sleep(0.1)
    stop_daemon(proc, log)
    raise RuntimeError("packaged multi-index daemon did not become ready")


def stop_daemon(proc, log) -> None:
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
    log.close()


def extract_daemon(root: Path) -> Path:
    bundle = artifact("searchkernel-runtime-", ".tar.gz")
    daemon_root = root / "daemon"
    daemon_root.mkdir()
    with tarfile.open(bundle) as archive:
        archive.extractall(daemon_root, filter="data")
    daemon = daemon_root / "bin/searchkerneld"
    daemon.chmod(0o755)
    return daemon


def packaged_daemon_negative_probe(root: Path, packaged_root: Path) -> None:
    broken_root = root / "incomplete-daemon"
    shutil.copytree(packaged_root, broken_root, symlinks=True)
    broken_daemon = broken_root / "bin/searchkerneld"
    adapter = broken_root / "lib/libsearchkernel_vespa.so.2"
    if not adapter.is_file():
        raise RuntimeError(f"packaged daemon is missing its required adapter: {adapter}")
    adapter.unlink()
    socket_path = root / "broken-daemon.sock"
    port = free_port()
    environment = clean_environment()
    environment["PATH"] = "/usr/bin:/bin"
    forbidden = (
        "LD_LIBRARY_PATH",
        "LIBRARY_PATH",
        "LD_PRELOAD",
        "LD_AUDIT",
        "CARGO_MANIFEST_DIR",
        "CARGO_TARGET_DIR",
    )
    if any(name in environment for name in forbidden):
        raise RuntimeError("runtime negative probe inherited a developer library/build override")

    log = tempfile.TemporaryFile(mode="w+t", encoding="utf-8")
    proc = subprocess.Popen(
        [
            str(broken_daemon),
            "--data-root",
            str(root / "broken-data"),
            "--listen",
            f"unix://{socket_path}",
            "--web-listen",
            f"127.0.0.1:{port}",
        ],
        cwd=broken_root,
        env=environment,
        stdout=log,
        stderr=subprocess.STDOUT,
        text=True,
    )
    ready = False
    deadline = time.monotonic() + 10
    try:
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                break
            try:
                with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                    ready = True
                    break
            except OSError:
                time.sleep(0.05)
        if proc.poll() is None and not ready:
            proc.terminate()
            proc.wait(timeout=5)
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)

    diagnostic = daemon_log(log)
    log.close()
    if ready or proc.returncode == 0 or "libsearchkernel_vespa.so.2" not in diagnostic:
        raise RuntimeError(
            "incomplete packaged daemon runtime did not fail before readiness at the missing "
            f"native adapter: ready={ready}, exit={proc.returncode}, output={diagnostic!r}"
        )


CREATE_TARGET_RUBY = r'''require "searchkernel"
target, endpoint = ARGV
fields = {
  "title" => SearchKernel::Text.new(stored: true, searchable: true),
  "kind" => SearchKernel::Keyword.new(stored: true, filterable: true, sortable: true),
  "score" => SearchKernel::IntegerField.new(stored: true, filterable: true, sortable: true),
  "embedding" => SearchKernel::TensorField.new(
    "tensor<float>(x[2])", stored: true, distance_metric: "angular",
    hnsw: SearchKernel::HnswIndexConfig.new
  )
}
profile = SearchKernel::RankProfile.new("semantic")
  .first_phase("closeness(embedding)")
  .query_input("query_embedding", "tensor<float>(x[2])")
index = SearchKernel::Index.create(endpoint, fields: fields, rank_profiles: [profile], target: target)
index.close
'''


def create_index_target(gem_home: Path, target: str, endpoint: str, root: Path) -> None:
    env = {"GEM_HOME": str(gem_home), "GEM_PATH": str(gem_home)}
    run(
        "ruby", "-e", CREATE_TARGET_RUBY, target, endpoint,
        cwd=root, env=env,
    )


def with_restarted_daemon(
    root: Path, daemon: Path, index_name: str, gem_home: Path, probe,
) -> None:
    data_root = root / f"{index_name}-data-root"
    socket_path = root / f"{index_name}.sock"
    target = index_name
    port = free_port()
    endpoint = f"unix://{socket_path}"
    http = f"http://127.0.0.1:{port}"
    proc, log = start_multi_daemon(daemon, data_root, socket_path, port)
    try:
        create_index_target(gem_home, target, http, root)
        probe("initial", endpoint, http, target)
    finally:
        stop_daemon(proc, log)

    port = free_port()
    proc, log = start_multi_daemon(daemon, data_root, socket_path, port)
    try:
        open_target = r'''require "searchkernel"
target, endpoint = ARGV
index = SearchKernel::Index.open(endpoint, target: target)
index.close
'''
        run("ruby", "-e", open_target, target, f"http://127.0.0.1:{port}",
            cwd=root, env={"GEM_HOME": str(gem_home), "GEM_PATH": str(gem_home)})
        probe("reopened", endpoint, f"http://127.0.0.1:{port}", target)
    finally:
        stop_daemon(proc, log)


def python_connect_probe(venv: Path, phase: str, endpoint: str, target: str) -> None:
    code = f'''from searchkernel import Index,Tensor,q
x=Index.connect({endpoint!r}, target={target!r})
try:
 fields={{f.name for f in x.describe().index.fields}}
 assert {{"title","kind","score","embedding"}} <= fields
 if {phase!r} == "initial":
  x.insert({{"_id":"python-kept","title":"alpha running shoe","kind":"shoe","score":4,"embedding":Tensor.dense("tensor<float>(x[2])",[1,0])}})
  x.insert({{"_id":"python-sort","title":"beta running shoe","kind":"shoe","score":2,"embedding":Tensor.dense("tensor<float>(x[2])",[0,1])}})
  x.insert({{"_id":"python-deleted","title":"gamma running shoe","kind":"shoe","score":1,"embedding":Tensor.dense("tensor<float>(x[2])",[.5,.5])}})
  x.update("python-kept").set("score",8).set("title","persisted python alpha running shoe").execute()
  assert x.get("python-kept")["score"] == 8
  assert x.delete("python-deleted") is True and x.get("python-deleted") is None
 expected=["python-sort","python-kept"]
 hits=x.query(q.contains("title","running")).filter("kind = :k",{{"k":"shoe"}}).sort("score","asc").execute().hits
 assert [h.id for h in hits] == expected
 assert x.get("python-kept")["title"] == "persisted python alpha running shoe"
 assert x.get("python-deleted") is None
 assert [h.id for h in x.query(q.contains("title","persisted")).execute().hits] == ["python-kept"]
 nearest=x.query(q.nearest_neighbor("embedding","query_embedding",target_hits=2)).ranking("semantic").input("query_embedding",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(2).execute()
 assert nearest.hits[0].id == "python-kept"
finally:
 x.close()
'''
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent)


def typescript_connect_probe(node: Path, phase: str, endpoint: str, target: str) -> None:
    code = f'''import {{Index,Tensor,q}} from "@searchkernel/client/node";
const x=Index.connect({json.dumps(endpoint)}, {json.dumps(target)});
try {{
 const schema=(await x.describe()).index;
 const fields=new Set(schema.fields.map(f=>f.name));
 for(const f of ["title","kind","score","embedding"]) if(!fields.has(f)) throw new Error("schema missing "+f);
 if({json.dumps(phase)}==="initial") {{
  await x.insert({{_id:"typescript-kept",title:"delta running shoe",kind:"shoe",score:4n,embedding:Tensor.dense("tensor<float>(x[2])",[1,0])}});
  await x.insert({{_id:"typescript-sort",title:"epsilon running shoe",kind:"shoe",score:2n,embedding:Tensor.dense("tensor<float>(x[2])",[0,1])}});
  await x.insert({{_id:"typescript-deleted",title:"zeta running shoe",kind:"shoe",score:1n,embedding:Tensor.dense("tensor<float>(x[2])",[.5,.5])}});
  await x.update("typescript-kept").set("score",8n).set("title","persisted typescript delta running shoe").execute();
  if((await x.get("typescript-kept")).score!==8n) throw new Error("updated get failed");
  if(!await x.delete("typescript-deleted") || await x.get("typescript-deleted")!==null) throw new Error("delete failed");
 }}
 const hits=(await x.query(q.contains("title","running")).filter("kind = :k",{{k:"shoe"}}).sort("score","asc").execute()).hits;
 if(hits.map(h=>h.id).join(",")!=="typescript-sort,typescript-kept") throw new Error("text/filter/sort failed: "+hits.map(h=>h.id));
 if((await x.get("typescript-kept")).title!=="persisted typescript delta running shoe") throw new Error("persisted update failed");
 if(await x.get("typescript-deleted")!==null) throw new Error("deleted document returned after reopen");
 if((await x.query(q.contains("title","persisted")).execute()).hits.map(h=>h.id).join(",")!=="typescript-kept") throw new Error("updated document is not searchable");
 const nearest=await x.query(q.nearestNeighbor("embedding","query_embedding")).ranking("semantic").input("query_embedding",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(2).execute();
 if(nearest.hits[0].id!=="typescript-kept") throw new Error("nearest-neighbor failed");
}} finally {{ await x.close(); }}
'''
    run("node", "--input-type=module", "-e", code, cwd=node)


RUBY_PROBE = r'''require "json"
require "rubygems"
gem "searchkernel"
require "searchkernel"

spec = Gem.loaded_specs.fetch("searchkernel")
entry = $LOADED_FEATURES.find { |path| File.basename(path) == "searchkernel.rb" }
raise "installed gem entry point was not loaded" unless entry
raise "Ruby client did not load from installed gem" unless File.expand_path(entry) == File.join(File.expand_path(spec.full_gem_path), "lib", "searchkernel.rb")

phase, endpoint, target = ARGV
index = phase == "reopened" ? SearchKernel::Index.open(endpoint, target: target) : SearchKernel::Index.connect(endpoint, target: target)
begin
  names = index.describe.fetch("fields").map { |field| field.fetch("name") }
  raise "schema missing release fields: #{names.inspect}" unless %w[title kind score embedding].all? { |name| names.include?(name) }
  if phase == "initial"
    index.insert("ruby-kept", {"title" => "eta running shoe", "kind" => "shoe", "score" => 4, "embedding" => SearchKernel::Tensor.dense("tensor<float>(x[2])", [1, 0])})
    index.insert("ruby-sort", {"title" => "theta running shoe", "kind" => "shoe", "score" => 2, "embedding" => SearchKernel::Tensor.dense("tensor<float>(x[2])", [0, 1])})
    index.insert("ruby-deleted", {"title" => "iota running shoe", "kind" => "shoe", "score" => 1, "embedding" => SearchKernel::Tensor.dense("tensor<float>(x[2])", [0.5, 0.5])})
    index.patch("ruby-kept").set("score", 8).set("title", "persisted ruby eta running shoe").execute
    raise "updated get failed" unless index.get("ruby-kept").fetch("score") == 8
    raise "delete failed" unless index.delete("ruby-deleted") && index.get("ruby-deleted").nil?
  end
  hits = index.query(SearchKernel::Query.contains("title", "running")).filter("kind = :k", "k" => "shoe").sort("score", "asc").execute.hits.map(&:id)
  raise "text/filter/sort failed: #{hits.inspect}" unless hits == %w[ruby-sort ruby-kept]
  raise "updated field did not persist" unless index.get("ruby-kept").fetch("title") == "persisted ruby eta running shoe"
  raise "deleted document returned after reopen" unless index.get("ruby-deleted").nil?
  raise "updated document is not searchable" unless index.query(SearchKernel::Query.contains("title", "persisted")).execute.hits.map(&:id) == ["ruby-kept"]
  query = SearchKernel::Query.nearest_neighbor("embedding", "query_embedding", target_hits: 2)
  nearest = index.query(query).ranking("semantic").input("query_embedding", SearchKernel::Tensor.dense("tensor<float>(x[2])", [1, 0])).limit(2).execute
  raise "nearest-neighbor failed" unless nearest.hits.first.id == "ruby-kept"
ensure
  index.close
end
'''


def ruby_connect_probe(
    gem_home: Path, root: Path, phase: str, http: str, target: str,
) -> None:
    script = root / "ruby-connect-probe.rb"
    script.write_text(RUBY_PROBE, encoding="utf-8")
    env = {"GEM_HOME": str(gem_home), "GEM_PATH": str(gem_home)}
    run("ruby", str(script), phase, http, target, cwd=root, env=env)


MULTI_INDEX_RUBY_PROBE = r'''require "rubygems"
gem "searchkernel"
require "searchkernel"

phase, endpoint = ARGV
fields = {
  "title" => SearchKernel::Text.new(stored: true, searchable: true),
  "kind" => SearchKernel::Keyword.new(stored: true, filterable: true),
  "score" => SearchKernel::IntegerField.new(stored: true, sortable: true)
}

if phase == "initial"
  a = SearchKernel::Index.create(endpoint, fields: fields, target: "release-a")
  b = SearchKernel::Index.create(endpoint, fields: fields, target: "release-b")
  begin
    a.insert("_id" => "a-only", "title" => "alpha registry", "kind" => "a", "score" => 1)
    b.insert("_id" => "b-only", "title" => "beta registry", "kind" => "b", "score" => 2)
    raise "index A cannot read its own document" unless a.get("a-only")
    raise "index B cannot read its own document" unless b.get("b-only")
    raise "index A leaked index B data" unless a.get("b-only").nil?
    raise "index B leaked index A data" unless b.get("a-only").nil?
    raise "index A search failed" unless a.search("alpha").execute.hits.map(&:id) == ["a-only"]
    a.close
    raise "closing A disrupted B" unless b.search("beta").execute.hits.map(&:id) == ["b-only"]
  ensure
    a.close rescue nil
    b.close rescue nil
  end
else
  a = SearchKernel::Index.open(endpoint, target: "release-a")
  b = SearchKernel::Index.open(endpoint, target: "release-b")
  begin
    raise "index A did not persist across daemon restart" unless a.get("a-only")
    raise "index B did not persist across daemon restart" unless b.get("b-only")
    raise "index A leaked index B data after restart" unless a.get("b-only").nil?
    raise "index B leaked index A data after restart" unless b.get("a-only").nil?
  ensure
    a.close
    b.close
  end
end
'''


def multi_index_registry_probe(gem_home: Path, root: Path, daemon: Path) -> None:
    data_root = root / "multi-index-data"
    socket_path = root / "multi-index.sock"
    script = root / "multi-index-probe.rb"
    script.write_text(MULTI_INDEX_RUBY_PROBE, encoding="utf-8")
    env = {"GEM_HOME": str(gem_home), "GEM_PATH": str(gem_home)}

    port = free_port()
    proc, log = start_multi_daemon(daemon, data_root, socket_path, port)
    try:
        run(
            "ruby",
            str(script),
            "initial",
            f"http://127.0.0.1:{port}",
            cwd=root,
            env=env,
        )
    finally:
        stop_daemon(proc, log)

    port = free_port()
    proc, log = start_multi_daemon(daemon, data_root, socket_path, port)
    try:
        run(
            "ruby",
            str(script),
            "reopened",
            f"http://127.0.0.1:{port}",
            cwd=root,
            env=env,
        )
    finally:
        stop_daemon(proc, log)


def shared_runtime_contract_probe(
    root: Path, installs: dict[str, Path]
) -> None:
    descriptor = installed_runtime_descriptors(installs)
    identity = descriptor["runtime_identity"]
    python_descriptor = next(
        installs["python"].glob("lib/python*/site-packages/searchkernel/runtime.json")
    )
    runtime_origin = DIST.as_uri()
    offline_origin = "file:///searchkernel-runtime-offline"

    # The first-use race is deliberately cross-language and uses one cache.
    # Each process starts a managed-local host, so the check exercises the
    # packaged clients' actual lazy runtime acquisition path.
    shared_cache = root / "runtime-cache"
    run_managed_local_processes(
        ("python", "node", "ruby"),
        installs,
        root,
        shared_cache,
        runtime_origin,
        label="cold-concurrent",
    )
    assert_one_valid_runtime_object(shared_cache, python_descriptor, installs["python"], identity)

    # Check both asymmetric warm-cache directions. The offline origin makes a
    # warm success observable: a resolver that downloads again cannot pass.
    for seed, warm in (("python", ("node", "ruby")), ("node", ("python", "ruby"))):
        cache = root / f"runtime-cache-seed-{seed}"
        run_managed_local_processes(
            (seed,), installs, root, cache, runtime_origin, label=f"{seed}-cold"
        )
        assert_one_valid_runtime_object(cache, python_descriptor, installs["python"], identity)
        run_managed_local_processes(
            warm, installs, root, cache, offline_origin, label=f"{seed}-warm"
        )
        assert_one_valid_runtime_object(cache, python_descriptor, installs["python"], identity)

    # Exercise the explicit prefetch/prewarm command, then require all three
    # managed-local clients to consume that exact object without an origin.
    prefetch_cache = root / "runtime-cache-prefetch"
    runtime_module = next(
        installs["python"].glob("lib/python*/site-packages/searchkernel/_runtime.py")
    )
    run(
        str(installs["python"] / "bin/python"),
        str(runtime_module),
        "prefetch",
        str(python_descriptor),
        "--cache",
        str(prefetch_cache),
        cwd=installs["python"].parent,
        env={"SEARCHKERNEL_RUNTIME_ORIGIN": runtime_origin},
    )
    prefetched = assert_one_valid_runtime_object(
        prefetch_cache, python_descriptor, installs["python"], identity
    )
    run_managed_local_processes(
        ("python", "node", "ruby"),
        installs,
        root,
        prefetch_cache,
        offline_origin,
        label="prefetched-warm",
    )
    assert prefetched == prefetch_cache / "objects" / identity

    # A mismatched object must be rejected by every client, rather than being
    # treated as a usable cache hit or silently replaced from an untrusted path.
    mismatch_manifest = json.loads(
        (prefetched / "manifest.json").read_text(encoding="utf-8")
    )
    mismatch_manifest["runtime_identity"] = "skrt-v1-" + "0" * 64
    (prefetched / "manifest.json").write_text(
        json.dumps(mismatch_manifest), encoding="utf-8"
    )
    outcomes = run_managed_local_processes(
        ("python", "node", "ruby"),
        installs,
        root,
        prefetch_cache,
        None,
        runtime_dir=prefetched,
        label="mismatch-rejected",
        expect_success=False,
    )
    if any("identity" not in (stdout + stderr).lower() and "mismatch" not in (stdout + stderr).lower()
           for _, _, stdout, stderr in outcomes):
        raise RuntimeError("runtime mismatch rejection did not identify the identity failure")

def main() -> None:
    verify_manifest()
    with tempfile.TemporaryDirectory(prefix="searchkernel-release-qualification-") as raw:
        root = Path(raw)
        installs = install_consumers(root)
        context = {"daemon": None}

        def rust_embedded_probe() -> None:
            # Build from Cargo's real .crate artifacts in a clean external
            # project. This is the consumer distribution contract: no checkout
            # or workspace path dependency reaches Cargo.
            run_rust_quickstart(root)

        def searchkerneld_runtime_probe() -> None:
            context["daemon"] = extract_daemon(root)
            packaged_daemon_negative_probe(root, context["daemon"].parent.parent)
            multi_index_registry_probe(installs["ruby"], root, context["daemon"])

        def python_managed_local_probe() -> None:
            runtime_env = {
                "SEARCHKERNEL_RUNTIME_ORIGIN": DIST.as_uri(),
                "SEARCHKERNEL_RUNTIME_CACHE": str(root / "runtime-cache"),
            }
            run_python_quickstart(installs["python"], root, runtime_env)
            python_managed_local(
                installs["python"], root / "managed-local-index", runtime_env
            )

        def typescript_node_managed_local_probe() -> None:
            run_managed_local_processes(
                ("node",),
                installs,
                root,
                root / "runtime-cache",
                "file:///searchkernel-runtime-offline",
                label="node-managed-local",
            )

        def ruby_managed_local_probe() -> None:
            run_managed_local_processes(
                ("ruby",),
                installs,
                root,
                root / "runtime-cache",
                "file:///searchkernel-runtime-offline",
                label="ruby-managed-local",
            )

        def python_connect_probe_runner() -> None:
            with_restarted_daemon(
                root, context["daemon"], "python-connect-index", installs["ruby"],
                lambda phase, endpoint, _http, target: python_connect_probe(
                    installs["python"], phase, endpoint, target
                ),
            )

        def typescript_node_connect_probe_runner() -> None:
            def probe(phase, endpoint, _http, target):
                if phase == "initial":
                    typescript_node_runtime_lock_probe(installs["node"], root)
                    run_node_quickstart(installs["node"], endpoint, target)
                typescript_connect_probe(installs["node"], phase, endpoint, target)

            with_restarted_daemon(
                root, context["daemon"], "typescript-node-index", installs["ruby"], probe,
            )

        def ruby_connect_probe_runner() -> None:
            def probe(phase, _endpoint, http, target):
                if phase == "initial":
                    run_ruby_quickstart(installs["ruby"], root, http, target)
                ruby_connect_probe(installs["ruby"], root, phase, http, target)

            with_restarted_daemon(
                root, context["daemon"], "ruby-connect-index", installs["ruby"], probe,
            )

        runners = {
            "rust_embedded": rust_embedded_probe,
            "searchkerneld_runtime": searchkerneld_runtime_probe,
            "python_managed_local": python_managed_local_probe,
            "python_connect": python_connect_probe_runner,
            "typescript_node_managed_local": typescript_node_managed_local_probe,
            "typescript_node_connect": typescript_node_connect_probe_runner,
            "ruby_managed_local": ruby_managed_local_probe,
            "ruby_connect": ruby_connect_probe_runner,
        }
        failures = []
        try:
            shared_runtime_contract_probe(root, installs)
            print("packaged qualification PASS: shared consumer runtime contract")
        except Exception as exc:
            failures.append(f"shared consumer runtime contract: {exc}")
        for row, probe_name in QUALIFICATION_PROBES.items():
            try:
                runners[probe_name]()
                print(f"packaged qualification PASS: {row}")
            except Exception as exc:
                failures.append(f"{row} ({probe_name}): {exc}")
        if failures:
            raise SystemExit(
                "clean-machine consumer release qualification failed:\n  "
                + "\n  ".join(failures)
            )

    print("clean-machine consumer release qualification PASS")


if __name__ == "__main__":
    main()
