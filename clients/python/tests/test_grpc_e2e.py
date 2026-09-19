"""Python -> socket -> searchkerneld -> native Vespa backend acceptance test."""

from __future__ import annotations

import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

from searchkernel import Client, GrpcTransport, Index, generated  # noqa: E402
from searchkernel.wire import encode_index_definition  # noqa: E402


ROOT = Path(__file__).resolve().parents[3]
HARNESS = ROOT / "target" / "debug" / "searchkerneld-e2e-harness"
SDK_LIB = ROOT / "core" / "native" / "sdk" / "linux-x86_64" / "lib"


def _free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


class RealGrpcPythonE2ETest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Always rebuild from the current source. A pre-existing binary is not
        # evidence that this checkout can build or that the E2E uses its code.
        subprocess.run(["cargo", "build", "-p", "searchkerneld-e2e-harness"], cwd=ROOT, check=True)

    def setUp(self):
        self.tempdir = tempfile.TemporaryDirectory(prefix="searchkernel-python-e2e-")
        self.root = Path(self.tempdir.name)
        self.index_path = self.root / "index"
        self.definition_path = self.root / "definition.fb"
        fields = [
            generated.FieldDefinition(
                name="title",
                data_type=generated.DataType(kind="primitive", payload_kind="text"),
                stored=True,
                searchable=True,
            ),
            generated.FieldDefinition(
                name="category",
                data_type=generated.DataType(kind="primitive", payload_kind="keyword"),
                stored=True,
                filterable=True,
                sortable=True,
                facetable=True,
            ),
            generated.FieldDefinition(
                name="price",
                data_type=generated.DataType(kind="primitive", payload_kind="int64"),
                stored=True,
                filterable=True,
                sortable=True,
            ),
        ]
        self.definition_path.write_bytes(
            encode_index_definition(generated.IndexDefinition(name="python_e2e", fields=fields))
        )
        self.process = None
        self.client = None

    def tearDown(self):
        if self.client is not None:
            self.client.close()
        self._stop_server()
        self.tempdir.cleanup()

    def _start_server(self, *, definition: bool):
        port = _free_port()
        environment = os.environ.copy()
        environment.update(
            {
                "SEARCHKERNEL_E2E_INDEX_PATH": str(self.index_path),
                "SEARCHKERNEL_E2E_GRPC_ADDR": f"127.0.0.1:{port}",
                "LD_LIBRARY_PATH": str(SDK_LIB),
            }
        )
        if definition:
            environment["SEARCHKERNEL_E2E_INDEX_DEFINITION"] = str(self.definition_path)
        else:
            environment.pop("SEARCHKERNEL_E2E_INDEX_DEFINITION", None)
        self.process = subprocess.Popen(
            [str(HARNESS)], cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE
        )
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = self.process.stderr.read().decode(errors="replace")
                self.fail(f"searchkerneld exited during startup: {output}")
            with socket.socket() as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(("127.0.0.1", port))
                    return port
                except OSError:
                    time.sleep(0.05)
        self.fail("searchkerneld did not open its gRPC socket")

    def _stop_server(self):
        if self.process is None:
            return
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
                self.fail("searchkerneld did not shut down")
        if self.process.stdout is not None:
            self.process.stdout.close()
        if self.process.stderr is not None:
            self.process.stderr.close()
        self.process = None

    @staticmethod
    def _search_request():
        count = generated.GroupingOperation(
            kind="output",
            outputs=[
                generated.GroupingOutput(
                    expression=generated.GroupingExpression(kind="aggregation")
                )
            ],
        )
        each = generated.GroupingOperation(
            kind="each",
            operations=[
                generated.GroupingOperation(kind="max", value=10),
                generated.GroupingOperation(
                    kind="order",
                    expressions=[
                        generated.GroupingOrderExpr(
                            expression=generated.GroupingExpression(
                                kind="negative",
                                expression=generated.GroupingExpression(kind="aggregation"),
                            )
                        )
                    ],
                ),
                count,
            ],
        )
        grouping = generated.GroupingRequest(
            source=generated.GroupingSourceKind(
                kind="structured",
                value=generated.GroupingAllOperation(
                    operations=[
                        generated.GroupingOperation(
                            kind="group",
                            expression=generated.GroupingExpression(
                                kind="attribute", field="category"
                            ),
                        ),
                        each,
                    ]
                ),
            )
        )
        return generated.SearchRequest(
            query=generated.QuerySource(kind="text", text="hello"),
            search_fields=generated.SearchFieldSelection(
                fields=[generated.SearchField(field="title")]
            ),
            filters=generated.FilterRequest(
                structured=[
                    generated.FilterExpr(
                        kind="comparison",
                        field=generated.QueryFieldRef(path="price"),
                        op="gte",
                        value=generated.Value.int64(2),
                    )
                ]
            ),
            sort=[
                generated.SortExpr(
                    target=generated.SortTarget(kind="field", field="price"), direction="desc"
                )
            ],
            grouping=[grouping],
            limit=10,
        )

    def test_python_public_api_reaches_persisted_native_backend(self):
        port = self._start_server(definition=True)
        self.client = Client(GrpcTransport(f"127.0.0.1:{port}"))
        index = Index(self.client)

        index.upsert("doc:1", {"title": "hello world", "category": "a", "price": 5})
        index.upsert("doc:2", {"title": "hello again", "category": "b", "price": 2})

        stored = index.get("doc:1").document
        self.assertIsNotNone(stored)
        self.assertEqual(stored.fields["title"].value, "hello world")
        self.assertEqual(stored.fields["price"].value, 5)

        result = index.search(self._search_request())
        self.assertEqual(result.total_hits.value, 2)
        self.assertEqual([child.id for child in result.root.children[:2]], ["doc:1", "doc:2"])
        self.assertEqual(result.root.children[-1].id, "category")
        self.assertEqual(len(result.root.children[-1].children), 2)

        analysis = index.analyze("Hello world", field="title")
        self.assertEqual([token.term for token in analysis.tokens], ["hello", "world"])
        self.assertEqual(index.describe().index.name, "python_e2e")
        self.assertIsNotNone(index.inspect_query(self._search_request()).resolved_request)

        index.delete("doc:2")
        self.assertIsNone(index.get("doc:2").document)

        self.client.close()
        self.client = None
        self._stop_server()

        port = self._start_server(definition=False)
        self.client = Client(GrpcTransport(f"127.0.0.1:{port}"))
        reopened = Index(self.client)
        surviving = reopened.search(
            generated.SearchRequest(query=generated.QuerySource(kind="match_all"), limit=10)
        )
        self.assertEqual(surviving.total_hits.value, 1)
        self.assertEqual(surviving.root.children[0].id, "doc:1")


if __name__ == "__main__":
    unittest.main()
