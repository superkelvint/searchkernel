import json, os, subprocess, tempfile, unittest
from pathlib import Path
from searchkernel.ergonomic import SearchBuilder
from searchkernel.client import value
from searchkernel.protocol import pack_object
from SearchKernel.V2.UInt64Value import UInt64ValueT
from SearchKernel.V2.Value import ValueT
from SearchKernel.V2.ValueKind import ValueKind
class CaptureClient:
    def search(self, request):
        self.request = request
        return type(
            "R",
            (),
            {"root": None, "totalHits": None, "hits": [], "groups": [], "facets": [], "total": 0, "issues": []},
        )()
class CaptureIndex:
    def __init__(self): self.client=CaptureClient()
def snapshot(binary, fixture, request):
    with tempfile.NamedTemporaryFile() as f:
        f.write(pack_object(request)); f.flush()
        return json.loads(subprocess.check_output([str(binary),'--index-json',str(fixture),'--request',f.name],text=True))
class SemanticParity(unittest.TestCase):
    @unittest.skipUnless(
        os.getenv("SEARCHKERNEL_PARITY_CORPUS") and os.getenv("SEARCHKERNEL_PARITY_OUTPUT"),
        "run through tools/check-client-semantic-parity.py",
    )
    def test_frozen_corpus(self):
        root=Path(__file__).parents[3]; corpus=Path(os.environ['SEARCHKERNEL_PARITY_CORPUS']); data=json.loads(corpus.read_text());
        subprocess.run(['cargo','build','--quiet','--no-default-features','-p','searchkernel-testkit','--bin','searchkernel-semantic-snapshot'],cwd=root,check=True)
        meta=json.loads(subprocess.check_output(['cargo','metadata','--no-deps','--format-version','1'],cwd=root,text=True)); binary=Path(meta['target_directory'])/'debug/searchkernel-semantic-snapshot'; fixture=root/data['index']; out={}
        for scenario in data['scenarios']:
            i=scenario['intent']; idx=CaptureIndex(); b=SearchBuilder(idx,None if i['kind']=='browse' else i.get('query'))
            if 'filter' in i:
                bindings={k:(int(v['int64']) if 'int64' in v else ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=int(v['uint64']))) if 'uint64' in v else v['string'] if 'string' in v else v['float64']) for k,v in i['filter']['bindings'].items()}; b.filter(i['filter']['source'],bindings)
            if 'ranking' in i: b.ranking(i['ranking'])
            for k,v in i.get('inputs',{}).items(): b.input(k,int(v['int64']) if 'int64' in v else ValueT(kindType=ValueKind.UInt64Value, kind=UInt64ValueT(value=int(v['uint64']))))
            for k,v in i.get('rank_properties',{}).items(): b.rank_property(k,v)
            if 'limit' in i: b.limit(i['limit'])
            if 'offset' in i: b.offset(i['offset'])
            b.execute(); out[scenario['id']]=snapshot(binary,fixture,idx.client.request)
        for rejection in data.get('facade_rejections', []):
            if 'python' not in rejection['clients']:
                continue
            intent = rejection['intent']
            builder = SearchBuilder(CaptureIndex(), None)
            for key, item in intent['rank_properties'].items():
                with self.subTest(rejection=rejection['id'], key=key):
                    with self.assertRaises(TypeError):
                        builder.rank_property(key, item)
        Path(os.environ['SEARCHKERNEL_PARITY_OUTPUT']).write_text(json.dumps(out,sort_keys=True))
