#!/usr/bin/env python3
"""Qualify packaged release artifacts from a pristine consumer environment."""

from __future__ import annotations

import hashlib
import json
import os
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

DIST = Path(sys.argv[1]).resolve()
MANIFEST = json.loads((DIST / "manifest.json").read_text())


def clean_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for key in list(environment):
        if key.startswith(("SEARCHKERNEL_", "SK_NATIVE_", "VESPA_")) or key in {
            "CARGO_MANIFEST_DIR",
            "CARGO_TARGET_DIR",
            "LD_LIBRARY_PATH",
            "LIBRARY_PATH",
            "NODE_PATH",
            "PYTHONPATH",
        }:
            environment.pop(key, None)
    return environment


def run(*cmd: str, cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    environment = clean_environment()
    if env:
        environment.update(env)
    subprocess.run(cmd, cwd=cwd, env=environment, check=True)


def artifact(prefix: str, suffix: str) -> Path:
    matches = [
        DIST / item["file"]
        for item in MANIFEST["artifacts"]
        if item["file"].startswith(prefix) and item["file"].endswith(suffix)
    ]
    if len(matches) != 1:
        raise SystemExit(
            f"expected one {prefix}*{suffix} artifact, got {matches}"
        )
    return matches[0]


def verify_manifest() -> None:
    for item in MANIFEST["artifacts"]:
        path = DIST / item["file"]
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != item["sha256"]:
            raise SystemExit(f"checksum mismatch: {path.name}")


def python_probe(venv: Path, index: Path) -> None:
    code = r'''from searchkernel import Index,Text,Keyword,Integer,Tensor,TensorField,q
p=r"''' + str(index) + r'''"
with Index.create(p, fields={"title":Text(stored=True,searchable=True),"kind":Keyword(stored=True,filterable=True,sortable=True),"score":Integer(stored=True,filterable=True,sortable=True),"embedding":TensorField("tensor<float>(x[2])",stored=True)}) as x:
 x.insert({"_id":"a","title":"alpha running shoe","kind":"shoe","score":2,"embedding":Tensor.dense("tensor<float>(x[2])",[1,0])})
 x.insert({"_id":"b","title":"beta running shoe","kind":"shoe","score":1,"embedding":Tensor.dense("tensor<float>(x[2])",[0,1])})
 assert x.get("a")["score"]==2
 x.update("a").set("score",3).execute(); assert x.get("a")["score"]==3
 assert [h.id for h in x.search("running").filter("kind = :k",{"k":"shoe"}).sort("score","asc").execute().hits]==["b","a"]
 r=x.query(q.nearest_neighbor("embedding","qt",target_hits=2)).input("qt",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(2).execute(); assert r.hits[0].id=="a"
 assert x.delete("b") is True; assert x.get("b") is None
with Index.open(p) as x:
 assert x.get("a")["score"]==3 and x.get("b") is None
 assert any(h.id=="a" for h in x.search("running").execute().hits)
'''
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent)


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def daemon_log(log: tempfile._TemporaryFileWrapper | object) -> str:
    try:
        log.flush()
        log.seek(0)
        return log.read()
    except (OSError, ValueError):
        return ""


def start_daemon(
    binary: Path, index: Path, port: int
) -> tuple[subprocess.Popen[str], object]:
    log = tempfile.TemporaryFile(mode="w+t", encoding="utf-8")
    proc = subprocess.Popen(
        [
            str(binary),
            "--index",
            str(index),
            "--listen",
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
                "packaged daemon exited"
                + (f":\n{output}" if output else f" with {proc.returncode}")
            )
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return proc, log
        except OSError:
            time.sleep(0.1)
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=10)
    output = daemon_log(log)
    log.close()
    raise RuntimeError(
        "packaged daemon did not become ready"
        + (f":\n{output}" if output else "")
    )


def stop_daemon(proc: subprocess.Popen[str], log: object) -> None:
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=10)
    log.close()


def node_probe(node: Path, endpoint: str) -> None:
    code = '''import {Index,Tensor,q} from "@searchkernel/client/node";
const x=Index.connect(process.argv[1]);
await x.insert({_id:"c",title:"gamma running shoe",kind:"shoe",score:4n,embedding:Tensor.dense("tensor<float>(x[2])",[.9,.1])});
if((await x.get("c")).score!==4n) throw new Error("get failed");
await x.update("c").set("score",5n).execute();
let r=await x.search("running").filter("kind = :k",{k:"shoe"}).sort("score","asc").execute();
if(!r.hits.some(h=>h.id==="c")) throw new Error("text/filter/sort failed");
r=await x.query(q.nearestNeighbor("embedding","qt")).input("qt",Tensor.dense("tensor<float>(x[2])",[1,0])).limit(3).execute();
if(!r.hits.some(h=>h.id==="c")) throw new Error("nearest-neighbor failed");
if(!await x.delete("c") || await x.get("c")!==null) throw new Error("delete failed");
await x.close();'''
    run("node", "--input-type=module", "-e", code, endpoint, cwd=node)


def python_connect_probe(venv: Path, endpoint: str) -> None:
    code = f'''from searchkernel import Index
x=Index.connect({endpoint!r})
assert x.get("a")["score"] == 3
assert any(h.id == "a" for h in x.search("running").execute().hits)
x.close()
'''
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent)


def main() -> None:
    verify_manifest()
    with tempfile.TemporaryDirectory(
        prefix="searchkernel-release-qualification-"
    ) as raw:
        root = Path(raw)

        rust = artifact("searchkernel-rust-", ".tar.gz")
        rust_root = root / "rust"
        rust_root.mkdir()
        with tarfile.open(rust) as archive:
            archive.extractall(rust_root, filter="data")
        run(
            "cargo",
            "run",
            "--quiet",
            "-p",
            "searchkernel",
            "--features",
            "native",
            "--example",
            "try_searchkernel_client_api_e2e",
            cwd=rust_root,
        )

        wheel = artifact("searchkernel-", ".whl")
        venv = root / "py"
        run(sys.executable, "-m", "venv", str(venv))
        run(str(venv / "bin/pip"), "install", str(wheel))
        python_probe(venv, root / "managed-index")

        npm = artifact("searchkernel-client-", ".tgz")
        node = root / "node"
        node.mkdir()
        (node / "package.json").write_text(
            '{"private":true,"type":"module"}',
            encoding="utf-8",
        )
        run(
            "npm",
            "install",
            "--ignore-scripts",
            "--no-audit",
            "--no-fund",
            "--package-lock=false",
            str(npm),
            cwd=node,
        )

        negative = (
            'import("@searchkernel/client/node").then(async m=>{'
            'process.env.SEARCHKERNEL_LOCAL_HOST="/definitely/missing-searchkerneld";'
            'try{await m.Index.open("missing");process.exit(2)}'
            'catch(e){if(!String(e).match(/ENOENT|not found|spawn/))throw e}})'
        )
        run("node", "-e", negative, cwd=node)

        daemon_bundle = artifact("searchkerneld-", ".tar.gz")
        daemon_root = root / "daemon"
        daemon_root.mkdir()
        with tarfile.open(daemon_bundle) as archive:
            archive.extractall(daemon_root, filter="data")
        daemon = daemon_root / "bin/searchkerneld"
        daemon.chmod(0o755)

        port = free_port()
        proc, log = start_daemon(daemon, root / "managed-index", port)
        endpoint = f"http://127.0.0.1:{port}"
        try:
            python_connect_probe(venv, endpoint)
            node_probe(node, endpoint)
        finally:
            stop_daemon(proc, log)

        port = free_port()
        proc, log = start_daemon(daemon, root / "managed-index", port)
        try:
            python_connect_probe(venv, f"http://127.0.0.1:{port}")
        finally:
            stop_daemon(proc, log)

    print("clean-machine consumer release qualification PASS")


if __name__ == "__main__":
    main()
