#!/usr/bin/env python3
import argparse
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('mode',choices=['probe','enum','orphan']); p.add_argument('schema_dir'); a=p.parse_args()
root=Path(a.schema_dir); main=root/'searchkernel.fbs'; req=root/'searchkernel_ffi_request.fbs'; resp=root/'searchkernel_ffi_response.fbs'
mt=main.read_text(); rt=req.read_text(); st=resp.read_text(); marker='// -----------------------------------------------------------------------------\n// Portable errors'
if marker not in mt: raise SystemExit('portable errors marker missing')
if a.mode=='probe':
    defs='''// C6 verifier synthetic BFBS-only operation and type surface.
table VerifierProbePayload {
  signed_value:long;
  ratio:double;
  enabled:bool;
  mode:FieldKind;
}

union VerifierProbeValue {
  VerifierProbePayload
}

table VerifierProbeRequest {
  token:string;
  payload:VerifierProbePayload;
  value:VerifierProbeValue;
}

table VerifierProbeResult {
  token:string;
  payload:VerifierProbePayload;
  value:VerifierProbeValue;
}

'''
    mt=mt.replace(marker,defs+marker,1)
    needle='  timeout:OptionalUInt64;\n}'; repl='  timeout:OptionalUInt64;\n\n  // C6 verifier synthetic field.\n  verifier_probe_field:uint;\n}'
    if needle not in mt: raise SystemExit('SearchRequest insertion point missing')
    mt=mt.replace(needle,repl,1)
    needle='  SearchKernel.V1.InspectQueryRequest\n}'; repl='  SearchKernel.V1.InspectQueryRequest,\n  SearchKernel.V1.VerifierProbeRequest\n}'
    if needle not in rt: raise SystemExit('request union insertion point missing')
    rt=rt.replace(needle,repl,1)
    needle='  SearchKernel.V1.InspectQueryResult\n}'; repl='  SearchKernel.V1.InspectQueryResult,\n  SearchKernel.V1.VerifierProbeResult\n}'
    if needle not in st: raise SystemExit('response union insertion point missing')
    st=st.replace(needle,repl,1)
elif a.mode=='enum':
    needle='  DeadlineExceeded = 10\n}'; repl='  DeadlineExceeded = 10,\n  VerifierSynthetic = 11\n}'
    if needle not in mt: raise SystemExit('ErrorCode insertion point missing')
    mt=mt.replace(needle,repl,1)
else:
    defs='// C6 verifier intentionally unpaired request.\ntable VerifierOrphanRequest {\n  token:string;\n}\n\n'; mt=mt.replace(marker,defs+marker,1)
    needle='  SearchKernel.V1.InspectQueryRequest\n}'; repl='  SearchKernel.V1.InspectQueryRequest,\n  SearchKernel.V1.VerifierOrphanRequest\n}'
    if needle not in rt: raise SystemExit('request union insertion point missing')
    rt=rt.replace(needle,repl,1)
main.write_text(mt); req.write_text(rt); resp.write_text(st)
