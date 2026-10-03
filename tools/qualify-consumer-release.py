#!/usr/bin/env python3
"""Qualify packaged release artifacts from a pristine consumer environment."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

DIST = Path(sys.argv[1]).resolve()
MANIFEST = json.loads((DIST / "manifest.json").read_text(encoding="utf-8"))
QUALIFICATION_ROOT = Path(__file__).resolve().parent
QUICKSTARTS = QUALIFICATION_ROOT / "quickstarts"
LIFECYCLE_TIMEOUT_SECONDS = 120.0

# The regression test compares this executable registry to release-support.json.
# Every supported surface/runtime pair must map to one packaged consumer probe.
QUALIFICATION_PROBES = {
    "rust-embedded/native": "rust_embedded",
    "searchkerneld/native": "searchkerneld_runtime",
    "python/managed-local": "python_managed_local",
    "python/connect": "python_connect",
    "typescript-node/connect": "typescript_node_connect",
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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def native_base_fixture() -> tuple[dict[str, object], Path]:
    fixture_root = QUALIFICATION_ROOT / "native-base"
    manifest_path = fixture_root / "manifest.json"
    try:
        metadata = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeError(f"qualification native-base metadata is unavailable: {exc}") from exc

    artifact_name = metadata.get("artifact")
    if not isinstance(artifact_name, str) or not artifact_name:
        raise RuntimeError("qualification native-base metadata has no artifact")
    archive = fixture_root / artifact_name
    if not archive.is_file():
        raise RuntimeError(f"qualification native-base artifact is missing: {archive}")

    expected = metadata.get("sha256")
    actual = sha256_file(archive)
    if not isinstance(expected, str) or actual != expected:
        raise RuntimeError("qualification native-base artifact checksum mismatch")

    generation = metadata.get("artifact_generation")
    build_env = metadata.get("build_environment")
    target = metadata.get("native_target")
    if not isinstance(generation, int) or generation < 1:
        raise RuntimeError("qualification native-base generation is invalid")
    if build_env not in {"host", "ci"}:
        raise RuntimeError("qualification native-base build environment is invalid")
    if not isinstance(target, str) or not target:
        raise RuntimeError("qualification native-base target is invalid")
    return metadata, archive


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


def run_python_quickstart(venv: Path, root: Path) -> None:
    run(
        str(venv / "bin/python"),
        str(QUICKSTARTS / "python.py"),
        cwd=root,
        env={"SEARCHKERNEL_QUICKSTART_PATH": str(root / "quickstart-python-index")},
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


def run_rust_quickstart(rust_root: Path, root: Path) -> dict[str, str]:
    quickstart = rust_root / "release-quickstart"
    shutil.copytree(QUICKSTARTS / "rust", quickstart)
    native_base, native_base_archive = native_base_fixture()
    environment = {
        "SEARCHKERNEL_QUICKSTART_PATH": str(root / "quickstart-rust-index"),
        "SEARCHKERNEL_BUILD_ENV": str(native_base["build_environment"]),
        "SK_NATIVE_TARGET": str(native_base["native_target"]),
        "SK_NATIVE_BASE_ARTIFACT_URL": native_base_archive.as_uri(),
        "SK_NATIVE_BASE_ARTIFACT_GENERATION": str(native_base["artifact_generation"]),
        "SK_NATIVE_CACHE_DIR": str(root / "rust-native-cache"),
    }
    # Qualification consumes the exact native-base sidecar staged by this
    # release run. It never publishes to, discovers from, or downloads from
    # GitHub; the tag workflow owns publication separately.
    run(
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(quickstart / "Cargo.toml"),
        cwd=quickstart,
        env=environment,
    )
    return environment


def python_managed_local(venv: Path, index: Path) -> None:
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
    run(str(venv / "bin/python"), "-c", code, cwd=venv.parent)


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
index = SearchKernel::Index.create(endpoint, fields: fields, rank_profiles: [profile], target: target, timeout: 120.0)
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
index = SearchKernel::Index.open(endpoint, target: target, timeout: 120.0)
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
  a = SearchKernel::Index.create(endpoint, fields: fields, target: "release-a", timeout: 120.0)
  b = SearchKernel::Index.create(endpoint, fields: fields, target: "release-b", timeout: 120.0)
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
  a = SearchKernel::Index.open(endpoint, target: "release-a", timeout: 120.0)
  b = SearchKernel::Index.open(endpoint, target: "release-b", timeout: 120.0)
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


def main() -> None:
    verify_manifest()
    with tempfile.TemporaryDirectory(prefix="searchkernel-release-qualification-") as raw:
        root = Path(raw)
        installs = install_consumers(root)
        context = {"daemon": None}

        def rust_embedded_probe() -> None:
            rust = artifact("searchkernel-rust-", ".tar.gz")
            rust_root = root / "rust"
            rust_root.mkdir()
            with tarfile.open(rust) as archive:
                archive.extractall(rust_root, filter="data")
            # Build exactly like a clean Rust consumer, but provision the
            # native prerequisite from the qualification-only local sidecar.
            # GitHub publication/discovery is intentionally outside this test.
            native_environment = run_rust_quickstart(rust_root, root)
            run(
                "cargo", "run", "--quiet", "-p", "searchkernel", "--features", "native",
                "--example", "try_searchkernel_client_api_e2e", cwd=rust_root,
                env=native_environment,
            )

        def searchkerneld_runtime_probe() -> None:
            context["daemon"] = extract_daemon(root)
            packaged_daemon_negative_probe(root, context["daemon"].parent.parent)
            multi_index_registry_probe(installs["ruby"], root, context["daemon"])

        def python_managed_local_probe() -> None:
            run_python_quickstart(installs["python"], root)
            python_managed_local(installs["python"], root / "managed-local-index")

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
            "typescript_node_connect": typescript_node_connect_probe_runner,
            "ruby_connect": ruby_connect_probe_runner,
        }
        failures = []
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
