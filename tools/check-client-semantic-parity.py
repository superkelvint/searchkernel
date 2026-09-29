#!/usr/bin/env python3
"""SK-23-016: execute the frozen cross-language canonical-IR parity corpus."""
from pathlib import Path
import json, os, shutil, subprocess, sys, tempfile

ROOT=Path(__file__).resolve().parents[1]
if os.environ.get("SEARCHKERNEL_IN_CI_IMAGE") != "1" and os.environ.get("SEARCHKERNEL_DEV_USE_CI_IMAGE", "1") != "0":
 os.execv(str(ROOT/"dev"), [str(ROOT/"dev"), "verify", "client-semantic-parity"])

CORPUS=ROOT/'reference/client-semantic-parity/scenarios.json'

def run(cmd, **kw):
 print('+',' '.join(map(str,cmd)), flush=True)
 subprocess.run(cmd,cwd=ROOT,check=True,**kw)

def output(cmd, **kw):
 print('+',' '.join(map(str,cmd)), flush=True)
 return subprocess.check_output(cmd,cwd=ROOT,text=True,**kw)

def require_tool(name):
 path=shutil.which(name)
 if path is None:
  raise SystemExit(f"required parity tool is unavailable: {name}")
 return path

def semantic_snapshot():
 run(['cargo','build','--quiet','--no-default-features','-p','searchkernel-testkit','--bin','searchkernel-semantic-snapshot'])
 metadata=json.loads(output(['cargo','metadata','--no-deps','--format-version','1']))
 target_dir=Path(os.environ.get('CARGO_TARGET_DIR', metadata['target_directory']))
 if not target_dir.is_absolute():
  target_dir=ROOT/target_dir
 binary=target_dir/'debug'/'searchkernel-semantic-snapshot'
 if not binary.is_file():
  raise SystemExit(f"semantic snapshot binary was not produced at {binary}")
 return binary

def main():
 for tool in ('cargo','npm','php','ruby'):
  require_tool(tool)
 corpus=json.loads(CORPUS.read_text())
 ids=[x['id'] for x in corpus['scenarios']]
 if len(ids)!=len(set(ids)) or not ids:
  raise SystemExit('parity corpus scenario IDs must be unique and non-empty')

 snapshot=semantic_snapshot()
 fixture=ROOT/corpus['index']
 if not fixture.is_file():
  raise SystemExit(f"parity index fixture is missing: {fixture}")

 with tempfile.TemporaryDirectory(prefix='searchkernel-parity-') as d:
  out=Path(d)
  run(['cargo','run','--quiet','--no-default-features','-p','searchkernel-testkit','--bin','searchkernel-semantic-parity','--','--corpus',str(CORPUS),'--output',str(out/'rust.json')])

  env=os.environ.copy()
  env['PYTHONPATH']=str(ROOT/'clients/python')
  env['SEARCHKERNEL_PARITY_CORPUS']=str(CORPUS)
  env['SEARCHKERNEL_SEMANTIC_SNAPSHOT']=str(snapshot)
  env['SEARCHKERNEL_PARITY_INDEX']=str(fixture)

  env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'python.json')
  run([sys.executable,'-m','unittest','clients.python.tests.test_semantic_parity'],env=env)

  env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'typescript.json')
  run(['npm','--prefix','clients/typescript','test','--','tests/semantic-parity.test.ts'],env=env)

  env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'php.json')
  run(['php','clients/php/tests/semantic_parity.php'],env=env)

  env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'ruby.json')
  run(['ruby','clients/ruby/test/semantic_parity.rb'],env=env)

  traces={p.stem:json.loads(p.read_text()) for p in out.glob('*.json')}
  for language in ('rust','python','typescript','php','ruby'):
   if language not in traces:
    raise SystemExit(f'{language} parity trace missing')
   if set(traces[language])!=set(ids):
    raise SystemExit(f'{language} scenario coverage differs from frozen corpus')

  MAP_KEYS={'inputs','properties'}
  def norm(v, key=None):
   # Only normalize the representation mismatch explicitly allowed by the
   # canonical snapshot contract. Numeric kinds remain exact: coercing ints
   # to floats can hide signedness, precision, and large-integer regressions.
   if key in MAP_KEYS and v == []:
    return {}
   if isinstance(v,dict):
    return {k:norm(x,k) for k,x in v.items()}
   if isinstance(v,list):
    return [norm(x) for x in v]
   return v

  for sid in ids:
   values={lang:traces[lang][sid] for lang in traces}
   first=norm(values['rust'])
   if any(norm(v)!=first for v in values.values()):
    print(json.dumps({'scenario':sid,'traces':values},indent=2,sort_keys=True))
    raise SystemExit(f'canonical IR parity mismatch: {sid}')

 print(f'cross-language canonical IR parity PASS ({len(ids)} scenarios)')

if __name__=='__main__':
 main()
