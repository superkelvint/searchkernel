#!/usr/bin/env python3
"""Qualify packaged release artifacts from pristine consumer directories."""
import hashlib, json, os, shutil, socket, subprocess, sys, tarfile, tempfile, time, zipfile
from pathlib import Path

DIST = Path(sys.argv[1]).resolve()
ROOT = Path(__file__).resolve().parents[1]
MANIFEST = json.loads((DIST / "manifest.json").read_text())

def run(*cmd, cwd=None, env=None):
    clean = os.environ.copy()
    for key in list(clean):
        if key.startswith("SEARCHKERNEL_") or key in {"CARGO_MANIFEST_DIR", "PYTHONPATH", "NODE_PATH"}:
            clean.pop(key, None)
    if env: clean.update(env)
    subprocess.run(cmd, cwd=cwd, env=clean, check=True)

def artifact(prefix, suffix):
    matches=[DIST/a["file"] for a in MANIFEST["artifacts"] if a["file"].startswith(prefix) and a["file"].endswith(suffix)]
    if len(matches)!=1: raise SystemExit(f"expected one {prefix}*{suffix} artifact, got {matches}")
    return matches[0]

def verify_manifest():
    for item in MANIFEST["artifacts"]:
        p=DIST/item["file"]
        if hashlib.sha256(p.read_bytes()).hexdigest()!=item["sha256"]: raise SystemExit(f"checksum mismatch: {p.name}")

def python_probe(venv, index):
    code=r'''from searchkernel import Index,Text,Keyword,Integer,Tensor,TensorField,q
p=r"'''+str(index)+r'''"
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
    run(str(venv/"bin/python"), "-c", code, cwd=venv.parent)

def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]

def start_daemon(binary, index, port):
    env = os.environ.copy()
    for key in list(env):
        if key.startswith("SEARCHKERNEL_"):
            env.pop(key, None)
    proc = subprocess.Popen([str(binary), "--index", str(index), "--listen", f"127.0.0.1:{port}"], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    deadline = time.time() + 20
    while time.time() < deadline:
        if proc.poll() is not None:
            raise RuntimeError("packaged daemon exited: " + proc.stderr.read())
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=.2):
                return proc
        except OSError:
            time.sleep(.1)
    proc.terminate()
    raise RuntimeError("packaged daemon did not become ready")

def node_probe(node, endpoint):
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

def python_connect_probe(venv, endpoint):
    code = f'''from searchkernel import Index
x=Index.connect({endpoint!r})
assert x.get("a")["score"] == 3
assert any(h.id == "a" for h in x.search("running").execute().hits)
x.close()
'''
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent)

def main():
    verify_manifest()
    with tempfile.TemporaryDirectory(prefix="searchkernel-release-qualification-") as td:
        root=Path(td)
        rust=artifact("searchkernel-rust-", ".tar.gz")
        with tarfile.open(rust) as tf: tf.extractall(root/"rust", filter="data")
        # Public Rust quickstart example, compiled only from the extracted source artifact.
        run("cargo","run","--quiet","-p","searchkernel","--features","native","--example","try_searchkernel_client_api_e2e",cwd=root/"rust")
        wheel=artifact("searchkernel-", ".whl")
        venv=root/"py"; run(sys.executable,"-m","venv",str(venv)); run(str(venv/"bin/pip"),"install","--no-index",str(wheel))
        python_probe(venv, root/"managed-index")
        npm=artifact("searchkernel-client-", ".tgz"); node=root/"node"; node.mkdir(); (node/"package.json").write_text('{"private":true,"type":"module"}')
        run("npm","install","--ignore-scripts","--no-audit","--no-fund","--package-lock=false",str(npm),cwd=node)
        # Negative package/runtime case: explicit incompatible runtime override must fail.
        negative='import("@searchkernel/client/node").then(async m=>{process.env.SEARCHKERNEL_LOCAL_HOST="/definitely/missing-searchkerneld";try{await m.Index.open("missing");process.exit(2)}catch(e){if(!String(e).match(/ENOENT|not found|spawn/))throw e}})'
        run("node","-e",negative,cwd=node)
        daemon_bundle=artifact("searchkerneld-", ".tar.gz"); daemon_root=root/"daemon"
        with tarfile.open(daemon_bundle) as tf: tf.extractall(daemon_root, filter="data")
        daemon=daemon_root/"bin/searchkerneld"; daemon.chmod(0o755); port=free_port(); proc=start_daemon(daemon,root/"managed-index",port)
        endpoint=f"http://127.0.0.1:{port}"
        try:
            python_connect_probe(venv,endpoint); node_probe(node,endpoint)
        finally:
            proc.terminate(); proc.wait(timeout=10)
        port=free_port(); proc=start_daemon(daemon,root/"managed-index",port)
        try: python_connect_probe(venv,f"http://127.0.0.1:{port}")
        finally: proc.terminate(); proc.wait(timeout=10)
    print("clean-machine consumer release qualification PASS")
if __name__ == "__main__": main()
