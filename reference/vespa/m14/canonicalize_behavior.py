#!/usr/bin/env python3
import json,sys
obj=json.load(sys.stdin)

def summarize_hits(hits, forbidden=None, include_count=False):
    if not isinstance(hits,list): raise SystemExit('hits must be an array')
    out={}
    if hits: out['top_hit']=hits[0]
    if include_count: out['hit_count']=len(hits)
    out['forbidden']=[x for x in (forbidden or []) if x in hits]
    return out

if obj.get('id')=='hnsw-query-controls':
    out={'id':obj['id'],'steps':[]}
    for step in obj.get('steps',[]):
        item={'id':step['id']}
        item.update(summarize_hits(step.get('hits',[]),[],True))
        out['steps'].append(item)
elif obj.get('id')=='hnsw-mutation-reopen':
    deleted='id:ns:typea::b'
    out={'id':obj['id']}
    out['before_update']=summarize_hits(obj.get('before_update',[]))
    out['after_update']=summarize_hits(obj.get('after_update',[]))
    out['after_reopen']=summarize_hits(obj.get('after_reopen',[]))
    out['after_delete']={'forbidden':[x for x in [deleted] if x in obj.get('after_delete',[])]}
    out['after_delete_reopen']={'forbidden':[x for x in [deleted] if x in obj.get('after_delete_reopen',[])]}
else:
    raise SystemExit('unknown M14 behavior output id')
print(json.dumps(out,sort_keys=True,separators=(',',':')))
