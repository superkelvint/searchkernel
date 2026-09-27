#!/usr/bin/env python3
"""Build the supported Linux x86-64 consumer artifact set plus provenance/checksums."""
import argparse, hashlib, json, os, shutil, subprocess, sys, tarfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
VERSION='0.1.0'
def run(*cmd,cwd=ROOT): subprocess.run(cmd,cwd=cwd,check=True)
def sha(p):
 h=hashlib.sha256();
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''): h.update(b)
 return h.hexdigest()
def daemon_binary():
 target=Path(os.environ.get('CARGO_TARGET_DIR',ROOT/'target'))
 if not target.is_absolute(): target=ROOT/target
 return target/'release'/'searchkerneld'
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--output',default='dist/release'); a=ap.parse_args(); out=(ROOT/a.output).resolve(); shutil.rmtree(out,ignore_errors=True); out.mkdir(parents=True)
 # Rust embedded distribution is a versioned source bundle: internal path crates are
 # included so a consumer never depends on unpublished workspace crates or this checkout.
 rust_bundle=out/f'searchkernel-rust-{VERSION}.tar.gz'
 rust_paths=['Cargo.toml','Cargo.lock','core/crates/searchkernel','core/crates/searchkernel-ir','core/crates/searchkernel-filter','core/crates/searchkernel-protocol','core/crates/searchkernel-ir-adapter','core/crates/searchkernel-vespa-engine','core/crates/searchkernel-vespa-bindings','api','tools/resolve-native-sdk.sh','tools/install-native-base-artifact.sh','tools/native-base-release-selection.py','core/native/vespa/manifest.json']
 with tarfile.open(rust_bundle,'w:gz') as tf:
  for rel in rust_paths:
   path=ROOT/rel
   if not path.exists(): raise FileNotFoundError(f'required consumer release path is missing: {rel}')
   tf.add(path,arcname=rel)
 # Daemon executable distribution.
 run('cargo','build','-p','searchkerneld','--release','--features','native')
 daemon=daemon_binary(); bundle=out/f'searchkerneld-{VERSION}-linux-x86_64-glibc.tar.gz'
 with tarfile.open(bundle,'w:gz') as tf: tf.add(daemon,arcname='bin/searchkerneld')
 # Supported managed-local packages carry the exact same versioned daemon.
 for package_dir in (ROOT/'clients/python/searchkernel', ROOT/'clients/typescript'):
  bindir=package_dir/'bin'; bindir.mkdir(exist_ok=True); shutil.copy2(daemon,bindir/'searchkerneld')
 run(sys.executable,'-m','pip','wheel','--no-deps','--no-build-isolation','--wheel-dir',str(out),str(ROOT/'clients/python'))
 run('npm','pack','--pack-destination',str(out),cwd=ROOT/'clients/typescript')
 for bundled in (ROOT/'clients/python/searchkernel/bin/searchkerneld', ROOT/'clients/typescript/bin/searchkerneld'):
  bundled.unlink(missing_ok=True)
  try: bundled.parent.rmdir()
  except OSError: pass
 commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 artifacts=[]
 for p in sorted(out.iterdir()):
  if p.is_file() and p.name!='manifest.json': artifacts.append({'file':p.name,'sha256':sha(p),'bytes':p.stat().st_size})
 (out/'manifest.json').write_text(json.dumps({'version':VERSION,'commit':commit,'platform':'linux-x86_64-glibc','artifacts':artifacts},indent=2)+'\n')
 print(out)
if __name__=='__main__': main()
