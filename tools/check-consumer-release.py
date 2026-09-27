#!/usr/bin/env python3
import hashlib,json,sys,tempfile,subprocess,tarfile,zipfile
from pathlib import Path
out=Path(sys.argv[1]); m=json.loads((out/'manifest.json').read_text()); assert m['platform']=='linux-x86_64-glibc'
for a in m['artifacts']:
 p=out/a['file']; assert hashlib.sha256(p.read_bytes()).hexdigest()==a['sha256']
# Assert artifacts are independently consumable and contain their runtime entry points.
rust=next(out.glob('searchkernel-rust-*.tar.gz'))
with tarfile.open(rust) as t:
 names=t.getnames(); assert 'core/crates/searchkernel/Cargo.toml' in names and 'native-sdk.lock.json' in names
wheel=next(out.glob('searchkernel-*.whl'))
with zipfile.ZipFile(wheel) as z: assert any(n.endswith('searchkernel/__init__.py') for n in z.namelist())
npm=next(out.glob('searchkernel-client-*.tgz'))
with tarfile.open(npm) as t: assert 'package/dist/node.js' in t.getnames()
daemon=next(out.glob('searchkerneld-*.tar.gz'))
with tarfile.open(daemon) as t: assert 'bin/searchkerneld' in t.getnames()
print('consumer release artifacts PASS')
