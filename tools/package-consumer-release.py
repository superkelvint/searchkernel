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
RUST_BUNDLE_PATHS = [
 'Cargo.toml', 'Cargo.lock', 'LICENSE', 'README.md', 'core/README.md',
 'core/crates', 'searchkerneld', 'api',
]
def build_rust_bundle(rust_bundle):
 missing=[rel for rel in RUST_BUNDLE_PATHS if not (ROOT/rel).exists()]
 if missing:
  raise FileNotFoundError(f"missing Rust consumer bundle inputs: {', '.join(missing)}")
 with tarfile.open(rust_bundle,'w:gz') as tf:
  for rel in RUST_BUNDLE_PATHS: tf.add(ROOT/rel,arcname=rel)
def main():
 ap=argparse.ArgumentParser(); ap.add_argument('--output',default='dist/release'); a=ap.parse_args(); out=(ROOT/a.output).resolve(); shutil.rmtree(out,ignore_errors=True); out.mkdir(parents=True)
 # Rust embedded distribution is a versioned source bundle. Include the complete
 # workspace/path-dependency closure, but omit repository-native helper scripts so
 # searchkernel-vespa-bindings uses its generated consumer support bundle.
 rust_bundle=out/f'searchkernel-rust-{VERSION}.tar.gz'
 build_rust_bundle(rust_bundle)
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
