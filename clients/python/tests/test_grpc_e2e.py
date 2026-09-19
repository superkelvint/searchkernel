"""Python generated-object API -> gRPC -> persisted native backend."""

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

from searchkernel import Client, GrpcTransport, Index  # noqa: E402
from searchkernel.client import value  # noqa: E402
from searchkernel.protocol import pack_object  # noqa: E402
from SearchKernel.V2.DataType import DataTypeT  # noqa: E402
from SearchKernel.V2.DataTypeKind import DataTypeKind  # noqa: E402
from SearchKernel.V2.FieldDefinition import FieldDefinitionT  # noqa: E402
from SearchKernel.V2.IndexDefinition import IndexDefinitionT  # noqa: E402
from SearchKernel.V2.MatchingConfig import MatchingConfigT  # noqa: E402
from SearchKernel.V2.MatchingMode import MatchingMode  # noqa: E402
from SearchKernel.V2.OptionalBool import OptionalBoolT  # noqa: E402
from SearchKernel.V2.PrimitiveDataType import PrimitiveDataTypeT  # noqa: E402
from SearchKernel.V2.SearchKernelTextQuery import SearchKernelTextQueryT  # noqa: E402
from SearchKernel.V2.QuerySource import QuerySourceT  # noqa: E402
from SearchKernel.V2.QuerySourceKind import QuerySourceKind  # noqa: E402
from SearchKernel.V2.SearchRequest import SearchRequestT  # noqa: E402
from SearchKernel.V2.MatchAllQuery import MatchAllQueryT  # noqa: E402
from SearchKernel.V2.OptionalUInt32 import OptionalUInt32T  # noqa: E402


ROOT = Path(__file__).resolve().parents[3]
DAEMON = ROOT / "target" / "debug" / "searchkerneld"
SDK_LIB = ROOT / "core" / "native" / "sdk" / "linux-x86_64" / "lib"


def _field(name: str, primitive: int, *, matching: int = MatchingMode.Unspecified) -> FieldDefinitionT:
    return FieldDefinitionT(
        name=name,
        dataType=DataTypeT(
            kindType=DataTypeKind.PrimitiveDataType,
            kind=PrimitiveDataTypeT(kind=primitive),
        ),
        stored=OptionalBoolT(True),
        searchable=OptionalBoolT(primitive == 1 and matching == MatchingMode.Text),
        filterable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        sortable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        facetable=OptionalBoolT(primitive != 1 or matching == MatchingMode.Exact),
        matching=MatchingConfigT(mode=matching) if primitive == 1 else None,
    )


class RealGrpcPythonE2ETest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(["cargo", "build", "-p", "searchkerneld"], cwd=ROOT, check=True)

    def setUp(self):
        self.tempdir = tempfile.TemporaryDirectory(prefix="searchkernel-python-e2e-")
        root = Path(self.tempdir.name)
        self.index_path = root / "index"
        self.definition_path = root / "definition.fb"
        self.definition = IndexDefinitionT(
            name="python_e2e",
            fields=[
                _field("title", 1, matching=MatchingMode.Text),
                _field("category", 1, matching=MatchingMode.Exact),
                _field("price", 5),
            ],
        )
        self.definition_path.write_bytes(pack_object(self.definition))
        self.process = None
        self.client = None

    def tearDown(self):
        if self.client is not None:
            self.client.close()
        self._stop_server()
        self.tempdir.cleanup()

    def _start_server(self, *, definition: bool):
        socket_path = Path(self.tempdir.name) / "searchkerneld.sock"
        environment = os.environ.copy()
        environment.update({
            "LD_LIBRARY_PATH": str(SDK_LIB),
        })
        command = [
            str(DAEMON),
            "--index",
            str(self.index_path),
            "--listen",
            f"unix://{socket_path}",
        ]
        if definition:
            command.extend(["--definition", str(self.definition_path)])
        self.process = subprocess.Popen(command, cwd=ROOT, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = self.process.stderr.read().decode(errors="replace")
                self.fail(f"searchkerneld exited during startup: {output}")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(0.1)
                try:
                    sock.connect(str(socket_path))
                    return f"unix://{socket_path}"
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
        if self.process.stdout is not None:
            self.process.stdout.close()
        if self.process.stderr is not None:
            self.process.stderr.close()
        self.process = None

    def test_generated_objects_reach_persisted_native_backend(self):
        endpoint = self._start_server(definition=True)
        self.client = Client(GrpcTransport(endpoint))
        index = Index(self.client)
        index.upsert("doc:1", {"title": "hello world", "category": "a", "price": 5})
        index.upsert("doc:2", {"title": "hello again", "category": "b", "price": 2})

        stored = index.get("doc:1").document
        stored_fields = {entry.key: entry.value.kind.value for entry in stored.fields}
        self.assertEqual(stored_fields["title"], "hello world")

        request = SearchRequestT(
            query=QuerySourceT(
                sourceType=QuerySourceKind.SearchKernelTextQuery,
                source=SearchKernelTextQueryT(text="hello"),
            ),
            limit=OptionalUInt32T(value=10),
        )
        result = index.search(request)
        self.assertEqual(result.totalHits.value, 2)
        self.assertEqual({child.node.id for child in [result.root.node.children[0], result.root.node.children[1]]}, {"doc:1", "doc:2"})

        described = index.describe().index
        self.assertEqual(described.name, self.definition.name)
        self.assertEqual({field.name for field in described.fields}, {"title", "category", "price"})

        index.delete("doc:2")
        self.assertIsNone(index.get("doc:2").document)
        self.client.close()
        self.client = None
        self._stop_server()

        endpoint = self._start_server(definition=False)
        self.client = Client(GrpcTransport(endpoint))
        reopened = Index(self.client)
        surviving = reopened.search(SearchRequestT(
            query=QuerySourceT(sourceType=QuerySourceKind.MatchAllQuery, source=MatchAllQueryT()),
            limit=OptionalUInt32T(value=10),
        ))
        self.assertEqual(surviving.totalHits.value, 1)

    def test_managed_local_mode_uses_production_daemon_over_uds(self):
        index = Index.create(self.index_path, fields=self.definition)
        try:
            index.upsert("doc:1", {"title": "hello", "category": "a", "price": 5})
            self.assertEqual(index.get("doc:1")["_id"], "doc:1")
        finally:
            index.close()


if __name__ == "__main__":
    unittest.main()
