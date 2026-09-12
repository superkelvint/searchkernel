#!/usr/bin/env python3
from __future__ import annotations
import json, re, sys
from collections import defaultdict
from pathlib import Path

root=Path(__file__).resolve().parents[1]
errors=[]

def load(p: Path):
    try: return json.loads(p.read_text())
    except Exception as e:
        errors.append(f'{p.relative_to(root)}: invalid JSON: {e}')
        return None

def rel(p): return str(p.relative_to(root))

manifest=load(root/'manifest.json') or {}
index=load(root/'cases.json') or {'cases':[]}
model=load(root/'model/canonical-model.json') or {}
case_files=sorted((root/'cases').glob('*/*.json'))
golden_files=sorted((root/'golden').glob('*/*.json'))
seen=set(); counts={}; expected_index=[]; referenced=set()

errdoc=(root/'docs/ERROR_CODES.md').read_text()
error_codes={m.group(1) for m in re.finditer(r'^([a-z][a-z0-9_]*)$',errdoc,re.M)}
error_detail_shapes=defaultdict(set)

allowed_ops={
 'resolve_index_definition','resolve_search','resolve_browse','resolve_write','resolve_get','resolve_bulk_write',
 'resolve_facet_values','resolve_preset','resolve_synonym','resolve_curation','describe_field','describe_index','inspect_search',
 'map_search_response','map_bulk_response','normalize_value','resolve_analyze','map_analyze_response','analyze_text','map_write_response',
 'map_get_response','map_delete_response','map_grouped_response','run_stateful_scenario'
}

# Runtime/generated artifacts are never constitutional.
for p in root.rglob('*'):
    if p.is_file() and (p.suffix=='.pyc' or '__pycache__' in p.parts):
        errors.append(f'generated artifact present: {rel(p)}')

def field_map(idx): return {x['name']:x for x in idx.get('fields',[])}
def field_cap(fd,key):
    if not fd: return None
    if key=='searchable': return bool(fd.get('searchable') or fd.get('kind')=='text')
    if key=='prefix': return bool(fd.get('prefix', True if fd.get('kind')=='text' else False))
    if key=='infix': return bool(fd.get('infix',False))
    if key=='typo_tolerance':
        return fd.get('typo_tolerance','standard/v1' if fd.get('kind')=='text' else 'disabled')
    return fd.get(key)

def scan_fixture_refs(v):
    if isinstance(v,dict):
        for k,x in v.items():
            if k=='backend_response_fixture': referenced.add(('backend-responses',str(x)))
            elif k=='preset_fixture': referenced.add(('presets',str(x)))
            scan_fixture_refs(x)
    elif isinstance(v,list):
        for x in v: scan_fixture_refs(x)

def scan_errors(v,path):
    if isinstance(v,dict):
        # Any object containing an "error" member with a canonical error is checked recursively.
        e=v.get('error')
        if isinstance(e,dict) and 'code' in e:
            code=e.get('code'); details=e.get('details')
            if code not in error_codes: errors.append(f'{path}: error code {code} not in ERROR_CODES.md')
            if not isinstance(details,dict): errors.append(f'{path}: error.details must be object for {code}')
            else: error_detail_shapes[code].add(tuple(sorted(details.keys())))
        for x in v.values(): scan_errors(x,path)
    elif isinstance(v,list):
        for x in v: scan_errors(x,path)

def validate_analyze_result(ar,path):
    req={'field','input','analyzer','tokens'}
    if set(ar)!=req: errors.append(f'{path}: AnalyzeResult shape must be {sorted(req)}')
    for i,t in enumerate(ar.get('tokens',[])):
        tr={'original','normalized','indexed','position'}
        if set(t)!=tr: errors.append(f'{path}: token {i} shape must be {sorted(tr)}')
        if t.get('position')!=i: errors.append(f'{path}: token positions must be contiguous from zero')

def validate_bulk_request(br,path):
    if set(br) != {'kind','ordered','atomic','operations'}:
        errors.append(f'{path}: canonical bulk request shape mismatch')
    if br.get('kind')!='batch' or br.get('ordered') is not True or br.get('atomic') is not False:
        errors.append(f'{path}: bulk must be kind=batch ordered=true atomic=false')
    for i,op in enumerate(br.get('operations',[])):
        if op.get('kind')=='upsert' and op.get('mode')!='replace': errors.append(f'{path}: bulk upsert {i} missing mode=replace')
        if op.get('kind')=='insert' and op.get('mode')!='create_only': errors.append(f'{path}: bulk insert {i} missing mode=create_only')

def validate_bulk_result(br,path):
    if set(br) != {'items','succeeded','failed'}: errors.append(f'{path}: BulkWriteResult shape mismatch')
    items=br.get('items',[])
    if br.get('succeeded') != sum(1 for x in items if x.get('ok') is True): errors.append(f'{path}: succeeded count mismatch')
    if br.get('failed') != sum(1 for x in items if x.get('ok') is False): errors.append(f'{path}: failed count mismatch')
    for i,it in enumerate(items):
        if set(it) != {'index','id','ok','error'}: errors.append(f'{path}: bulk item {i} shape mismatch')
        if it.get('index')!=i: errors.append(f'{path}: bulk item index/order mismatch at {i}')
        if it.get('ok') is True and it.get('error') is not None: errors.append(f'{path}: successful bulk item must have error=null')
        if it.get('ok') is False and not isinstance(it.get('error'),dict): errors.append(f'{path}: failed bulk item must contain error')

def validate_resolved_search(ro, inp, idx, path):
    req=set(model.get('types',{}).get('ResolvedSearchRequest',{}).get('required',[]))
    if set(ro) != req: errors.append(f'{path}: ResolvedSearchRequest keys differ; missing={sorted(req-set(ro))}, extra={sorted(set(ro)-req)}')
    q=ro.get('query',{})
    if set(q)!={'original_text','text','syntax','parsed'}: errors.append(f'{path}: query shape must be original_text,text,syntax,parsed')
    if q.get('original_text') != inp.get('query'):
        errors.append(f'{path}: query.original_text differs from caller input')
    # Effective text may differ only when curation explicitly records the replacement.
    replacement=ro.get('curation',{}).get('query_replacement')
    if replacement is None and q.get('text') != q.get('original_text'): errors.append(f'{path}: effective query changed without curation replacement')
    if replacement is not None and q.get('text') != replacement: errors.append(f'{path}: curation replacement != effective query text')
    cur=ro.get('curation',{})
    if set(cur)!={'applied','pins','hidden_ids','added_filters','query_replacement'}: errors.append(f'{path}: curation effect shape mismatch')
    # Explicit request options survive resolution.
    if 'limit' in inp and ro.get('pagination',{}).get('limit') != inp['limit']: errors.append(f'{path}: explicit limit lost')
    if 'offset' in inp and ro.get('pagination',{}).get('offset') != inp['offset']: errors.append(f'{path}: explicit offset lost')
    if 'prefix' in inp and ro.get('prefix') != inp['prefix']: errors.append(f'{path}: explicit prefix lost')
    if 'term_match' in inp and ro.get('term_match') != inp['term_match']: errors.append(f'{path}: explicit term_match lost')
    if 'typo_tolerance' in inp and ro.get('typo_tolerance') != inp['typo_tolerance']: errors.append(f'{path}: explicit typo policy lost')
    if 'synonyms' in inp and ro.get('synonyms') != inp['synonyms']: errors.append(f'{path}: explicit synonyms flag lost')
    if 'page' in inp:
        pg=inp['page']; exp=(pg['number']-1)*pg['size']
        if ro.get('pagination') != {'limit':pg['size'],'offset':exp}: errors.append(f'{path}: page normalization incorrect')
    # Search field canonical/effective shape and capability ceiling.
    fmp=field_map(idx or {})
    names=[]
    for sf in ro.get('search_fields',[]):
        if set(sf)!={'field','weight','prefix','infix','typo_tolerance'}: errors.append(f'{path}: noncanonical ResolvedSearchField')
        name=sf.get('field'); names.append(name); fd=fmp.get(name) if idx else None
        if idx:
            if not fd: errors.append(f'{path}: resolved unknown search field {name}')
            elif not field_cap(fd,'searchable'): errors.append(f'{path}: nonsearchable field resolved {name}')
            if sf.get('prefix') and not field_cap(fd,'prefix'): errors.append(f'{path}: prefix exceeds field capability {name}')
            if sf.get('infix') and not field_cap(fd,'infix'): errors.append(f'{path}: infix exceeds field capability {name}')
            cap=field_cap(fd,'typo_tolerance')
            if cap=='disabled' and sf.get('typo_tolerance')!='disabled': errors.append(f'{path}: typo policy exceeds field capability {name}')
    if len(names)!=len(set(names)): errors.append(f'{path}: duplicate resolved search fields')
    # Ordered-set/list invariants.
    if len(ro.get('highlights',[])) != len(set(ro.get('highlights',[]))): errors.append(f'{path}: duplicate resolved highlights')
    proj=ro.get('projection',{})
    if proj.get('kind')=='fields' and len(proj.get('fields',[])) != len(set(proj.get('fields',[]))): errors.append(f'{path}: duplicate projection fields')
    if ro.get('sort') and ro['sort'][-1] != {'kind':'document_id','direction':'asc'}: errors.append(f'{path}: hard sort missing DocumentId final tie-break')
    facet_names=[x.get('field') for x in ro.get('facets',[]) if x.get('kind')=='values']
    if len(facet_names)!=len(set(facet_names)): errors.append(f'{path}: duplicate value facets')
    grouping=ro.get('grouping')
    if grouping and grouping.get('hits_per_group',0)<1: errors.append(f'{path}: invalid resolved hits_per_group')
    # Synonyms disabled means no expansions.
    if ro.get('synonyms') is False and ro.get('synonym_expansions'): errors.append(f'{path}: synonym expansions present while synonyms=false')
    # A hidden document must not remain pinned.
    hidden=set(cur.get('hidden_ids',[])); pinned={x.get('id') for x in cur.get('pins',[])}
    if hidden & pinned: errors.append(f'{path}: curation hide must win over pin')

def validate_resolved_index(ro,path):
    fm={f['name']:f for f in ro.get('fields',[])}
    for f in ro.get('fields',[]):
        if set(f) != {'name','kind','analyzer','stored','optional','multi','capabilities'}:
            errors.append(f'{path}: ResolvedIndexField shape mismatch for {f.get("name")}')
        caps=f.get('capabilities',{})
        if set(caps)!={'searchable','filterable','sortable','facetable','prefix','infix','typo_tolerance'}:
            errors.append(f'{path}: FieldCapabilities shape mismatch for {f.get("name")}')
        if caps.get('prefix') and not caps.get('searchable'): errors.append(f'{path}: prefix capability on nonsearchable field {f.get("name")}')
        if caps.get('infix') and not caps.get('searchable'): errors.append(f'{path}: infix capability on nonsearchable field {f.get("name")}')
    for sf in ro.get('search_fields',[]):
        if set(sf)!={'field','weight'}: errors.append(f'{path}: ResolvedIndexDefinition.search_fields must be field+weight only')
        if sf.get('field') not in fm: errors.append(f'{path}: index search field missing from fields')
        elif not fm[sf['field']]['capabilities']['searchable']: errors.append(f'{path}: nonsearchable index search field')

def validate_stateful(case,golden,idx,path):
    csteps=case.get('input',{}).get('steps',[]); gsteps=golden.get('ok',{}).get('steps',[])
    if len(csteps)!=len(gsteps): errors.append(f'{path}: stateful step count mismatch'); return
    for i,(cs,gs) in enumerate(zip(csteps,gsteps)):
        sp=f'{path}:step[{i}]'
        if cs.get('op')=='search_resolve':
            if 'resolved' in gs: validate_resolved_search(gs['resolved'],cs.get('request',{}),idx,sp)
            elif 'error' not in gs: errors.append(f'{sp}: search_resolve must yield resolved or error')

for p in case_files:
    c=load(p)
    if not c: continue
    cid=c.get('id'); cat=c.get('category'); op=c.get('operation')
    if not cid: errors.append(f'{rel(p)}: missing id'); continue
    if cid in seen: errors.append(f'{rel(p)}: duplicate id {cid}')
    seen.add(cid); counts[cat]=counts.get(cat,0)+1
    if cid != f'{cat}.{p.stem}': errors.append(f'{rel(p)}: id/path mismatch {cid}')
    if op not in allowed_ops: errors.append(f'{cid}: unknown operation {op}')
    gp=root/'golden'/cat/(p.stem+'.json')
    if not gp.exists(): errors.append(f'{cid}: missing golden'); continue
    g=load(gp)
    expected_index.append({'id':cid,'operation':op,'case':rel(p),'golden':rel(gp)})
    idxname=c.get('index_fixture'); idx=None
    if idxname:
        ip=root/'fixtures/indexes'/f'{idxname}.json'; referenced.add(('indexes',idxname))
        if not ip.exists(): errors.append(f'{cid}: missing index fixture {idxname}')
        else: idx=load(ip)
    scan_fixture_refs(c.get('input',{})); scan_errors(g,cid)
    if not g: continue
    if op=='resolve_index_definition' and 'ok' in g: validate_resolved_index(g['ok'],cid)
    elif op=='describe_index' and 'ok' in g: validate_resolved_index(g['ok'],cid)
    elif op=='describe_field' and 'ok' in g:
        f=g['ok']
        if set(f) != {'name','kind','analyzer','stored','optional','multi','capabilities'}: errors.append(f'{cid}: describe_field shape mismatch')
        if set(f.get('capabilities',{})) != {'searchable','filterable','sortable','facetable','prefix','infix','typo_tolerance'}: errors.append(f'{cid}: describe_field capabilities shape mismatch')
    elif op in ('resolve_search','inspect_search') and 'ok' in g: validate_resolved_search(g['ok'],c.get('input',{}),idx,cid)
    elif op=='resolve_write' and 'ok' in g:
        ro=g['ok']
        if ro.get('kind')=='insert' and ro.get('mode')!='create_only': errors.append(f'{cid}: insert mode not explicit')
        if ro.get('kind')=='upsert' and ro.get('mode')!='replace': errors.append(f'{cid}: upsert mode not explicit')
    elif op=='resolve_bulk_write' and 'ok' in g: validate_bulk_request(g['ok'],cid)
    elif op=='map_bulk_response' and 'ok' in g: validate_bulk_result(g['ok'],cid)
    elif op in ('map_analyze_response','analyze_text') and 'ok' in g: validate_analyze_result(g['ok'],cid)
    elif op=='run_stateful_scenario' and 'ok' in g: validate_stateful(c,g,idx,cid)

# Error detail shape is a versioned cross-language contract, recursively.
for code,shapes in sorted(error_detail_shapes.items()):
    if len(shapes)>1: errors.append(f'error {code}: inconsistent details shapes {sorted(shapes)}')

# cases.json exactness.
if index.get('cases') != expected_index: errors.append('cases.json does not exactly match case/golden files')
if manifest.get('case_count') != len(case_files): errors.append(f'manifest case_count={manifest.get("case_count")} actual={len(case_files)}')
if manifest.get('categories') != dict(sorted(counts.items())): errors.append('manifest categories mismatch')
# no orphan goldens
expected_goldens={str((root/'golden'/load(p)['category']/(p.stem+'.json')).resolve()) for p in case_files if load(p)}
for gp in golden_files:
    if str(gp.resolve()) not in expected_goldens: errors.append(f'orphan golden: {rel(gp)}')
# all named fixtures referenced
for d in ('indexes','backend-responses','presets'):
    for fp in (root/'fixtures'/d).glob('*.json'):
        if (d,fp.stem) not in referenced: errors.append(f'unused fixture: {rel(fp)}')
# checksum manifest sanity if present
sumfile=root/'IMMUTABLE_SHA256SUMS'
if sumfile.exists():
    for line in sumfile.read_text().splitlines():
        if not line.strip(): continue
        parts=line.split(None,1)
        if len(parts)!=2: errors.append('malformed checksum line'); continue
        r=parts[1].lstrip('*')
        if '__pycache__' in r or r.endswith('.pyc') or r=='IMMUTABLE_SHA256SUMS': errors.append(f'invalid checksum entry: {r}')
        if not (root/r).exists(): errors.append(f'checksum references missing file: {r}')

if errors:
    print('\n'.join(errors),file=sys.stderr)
    raise SystemExit(1)
print(f'OK: {len(case_files)} cases across {len(counts)} categories')
for k in sorted(counts): print(f'  {k}: {counts[k]}')
print('Semantic invariants PASS')
