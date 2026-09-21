# M14 test matrix

| case | low-level schema | portable schema | real matcher | reopen | expected invariant |
| --- | ---: | ---: | ---: | ---: | --- |
| generated HNSW config | yes | yes | no | no | tensor attr, EUCLIDEAN, HNSW=true, max-links=4, insert-neighbors=16 |
| exact control | yes | yes | yes | no | known identical vector is top hit |
| ANN default exploration | yes | yes | yes | no | known identical vector is top hit |
| ANN extra exploration | yes | yes | yes | no | known identical vector is top hit |
| ANN distance threshold | yes | yes | yes | no | known identical vector is top hit |
| update graph | yes | yes | yes | no | updated-away `a` is no longer top; `b` becomes top |
| reopen graph | yes | yes | yes | yes | `b` remains top |
| delete graph node | yes | yes | yes | no | deleted `b` is absent |
| reopen after delete | yes | yes | yes | yes | deleted `b` remains absent |
| invalid HNSW schema | yes | yes | no | no | frozen coarse category |

The verifier intentionally does not freeze the complete approximate hit order
or an ANN recall percentage.
