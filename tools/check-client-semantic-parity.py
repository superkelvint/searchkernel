#!/usr/bin/env python3
"""SK-23-016: execute the frozen Rust/Python/TypeScript canonical-IR parity corpus."""
from pathlib import Path
import json, os, subprocess, sys, tempfile
ROOT=Path(__file__).resolve().parents[1]
CORPUS=ROOT/'reference/client-semantic-parity/scenarios.json'
def run(cmd, **kw):
 print('+',' '.join(map(str,cmd)), flush=True); subprocess.run(cmd,cwd=ROOT,check=True,**kw)
def main():
 corpus=json.loads(CORPUS.read_text()); ids=[x['id'] for x in corpus['scenarios']]
 if len(ids)!=len(set(ids)) or not ids: raise SystemExit('parity corpus scenario IDs must be unique and non-empty')
 with tempfile.TemporaryDirectory(prefix='searchkernel-parity-') as d:
  out=Path(d)
  run(['cargo','run','--quiet','--no-default-features','-p','searchkernel-testkit','--bin','searchkernel-semantic-parity','--','--corpus',str(CORPUS),'--output',str(out/'rust.json')])
  env=os.environ.copy(); env['PYTHONPATH']=str(ROOT/'clients/python'); env['SEARCHKERNEL_PARITY_CORPUS']=str(CORPUS); env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'python.json')
  run([sys.executable,'-m','unittest','clients.python.tests.test_semantic_parity'],env=env)
  env['SEARCHKERNEL_PARITY_OUTPUT']=str(out/'typescript.json')
  run(['npm','--prefix','clients/typescript','test','--','tests/semantic-parity.test.ts'],env=env)
  traces={p.stem:json.loads(p.read_text()) for p in out.glob('*.json')}
  for language in ('rust','python','typescript'):
   if language not in traces: raise SystemExit(f'{language} parity trace missing')
   if set(traces[language])!=set(ids): raise SystemExit(f'{language} scenario coverage differs from frozen corpus')
  for sid in ids:
   values={lang:traces[lang][sid] for lang in traces}
   first=values['rust']
   if any(v!=first for v in values.values()):
    print(json.dumps({'scenario':sid,'traces':values},indent=2,sort_keys=True)); raise SystemExit(f'canonical IR parity mismatch: {sid}')
 print(f'cross-language canonical IR parity PASS ({len(ids)} scenarios)')
if __name__=='__main__': main()
