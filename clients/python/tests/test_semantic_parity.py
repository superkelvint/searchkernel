import json, os, subprocess, tempfile, unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parents[1] / "examples"))

from searchkernel.ergonomic import SearchBuilder
from searchkernel import uint64
from searchkernel.protocol import pack_object
from client_real_e2e_parity_helpers import corpus_value
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultGroup import ResultGroupT
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultNode import ResultNodeT
from searchkernel._wire.v1.generated.SearchKernel.Wire.ResultNodeKind import ResultNodeKind
from searchkernel._wire.v1.generated.SearchKernel.Wire.TotalHits import TotalHitsT
from searchkernel._wire.v1.generated.SearchKernel.Wire.TotalHitsRelation import TotalHitsRelation
def corpus_binding_value(item):
    if 'int64' in item:
        return int(item['int64'])
    if 'uint64' in item:
        return uint64(int(item['uint64']))
    if 'string' in item:
        return item['string']
    if 'float64' in item:
        return float(item['float64'])
    raise ValueError(f"unsupported parity corpus value: {item!r}")

class CaptureClient:
    def search(self, request):
        self.request = request
        return type(
            "R",
            (),
            {
                "root": ResultNodeT(
                    ResultNodeKind.ResultGroup,
                    ResultGroupT(children=[]),
                ),
                "totalHits": TotalHitsT(
                    value=0,
                    relation=TotalHitsRelation.Exact,
                ),
                "hits": [],
                "groups": [],
                "facets": [],
                "total": 0,
                "issues": [],
            },
        )()
class CaptureIndex:
    def __init__(self): self.client=CaptureClient()
def snapshot(binary, fixture, request):
    with tempfile.NamedTemporaryFile() as f:
        f.write(pack_object(request)); f.flush()
        return json.loads(subprocess.check_output([str(binary),'--index-json',str(fixture),'--request',f.name],text=True))
class SemanticParity(unittest.TestCase):
    def test_corpus_values_follow_declared_field_types(self):
        self.assertEqual(corpus_value({"type": "float64"}, 89), 89.0)
        self.assertEqual(corpus_value({"type": "int64"}, 5), 5)

    @unittest.skipUnless(
        os.getenv("SEARCHKERNEL_PARITY_CORPUS") and os.getenv("SEARCHKERNEL_PARITY_OUTPUT"),
        "run through tools/check-client-semantic-parity.py",
    )
    def test_frozen_corpus(self):
        root=Path(__file__).parents[3]; corpus=Path(os.environ['SEARCHKERNEL_PARITY_CORPUS']); data=json.loads(corpus.read_text());
        binary=Path(os.environ['SEARCHKERNEL_SEMANTIC_SNAPSHOT']); fixture=Path(os.environ['SEARCHKERNEL_PARITY_INDEX']); out={}
        self.assertTrue(binary.is_file(), f"missing semantic snapshot: {binary}")
        self.assertTrue(fixture.is_file(), f"missing parity fixture: {fixture}")
        for scenario in data['scenarios']:
            i=scenario['intent']; idx=CaptureIndex(); b=SearchBuilder(idx,None if i['kind']=='browse' else i.get('query'))
            if 'filter' in i:
                bindings={k:corpus_binding_value(v) for k,v in i['filter']['bindings'].items()}; b.filter(i['filter']['source'],bindings)
            if 'ranking' in i: b.ranking(i['ranking'])
            for k,v in i.get('inputs',{}).items(): b.input(k,corpus_binding_value(v))
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
