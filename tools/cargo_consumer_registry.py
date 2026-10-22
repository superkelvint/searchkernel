"""A qualification-only sparse registry serving the exact release .crate bytes."""
from __future__ import annotations

import hashlib
import json
import tarfile
import threading
import tomllib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def crate_manifest(crate: Path) -> dict:
    with tarfile.open(crate, "r:gz") as archive:
        root = crate.name.removesuffix(".crate")
        stream = archive.extractfile(f"{root}/Cargo.toml")
        if stream is None:
            raise RuntimeError(f"missing normalized Cargo.toml in {crate.name}")
        return tomllib.loads(stream.read().decode("utf-8"))


def crate_index_path(name: str) -> str:
    lower = name.lower()
    if len(lower) == 1:
        return f"1/{lower}"
    if len(lower) == 2:
        return f"2/{lower}"
    if len(lower) == 3:
        return f"3/{lower[0]}/{lower}"
    return f"{lower[:2]}/{lower[2:4]}/{lower}"


def cargo_index_record(crate: Path) -> tuple[str, bytes, str, str]:
    manifest = crate_manifest(crate)
    package = manifest["package"]
    name, version = package["name"], package["version"]
    dependencies: list[dict[str, object]] = []
    for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
        for alias, value in manifest.get(kind, {}).items():
            dependency = {"version": value} if isinstance(value, str) else value
            dependencies.append({
                "name": alias,
                "req": dependency.get("version", "*"),
                "features": dependency.get("features", []),
                "optional": dependency.get("optional", False),
                "default_features": dependency.get("default-features", True),
                "target": None,
                "kind": {"dependencies": "normal", "build-dependencies": "build", "dev-dependencies": "dev"}[kind],
                "registry": None,
                "package": dependency.get("package"),
            })
    for target, sections in manifest.get("target", {}).items():
        for kind in ("dependencies", "build-dependencies", "dev-dependencies"):
            for alias, value in sections.get(kind, {}).items():
                dependency = {"version": value} if isinstance(value, str) else value
                dependencies.append({
                    "name": alias,
                    "req": dependency.get("version", "*"),
                    "features": dependency.get("features", []),
                    "optional": dependency.get("optional", False),
                    "default_features": dependency.get("default-features", True),
                    "target": target,
                    "kind": {"dependencies": "normal", "build-dependencies": "build", "dev-dependencies": "dev"}[kind],
                    "registry": None,
                    "package": dependency.get("package"),
                })
    features = manifest.get("features", {})
    record: dict[str, object] = {
        "name": name,
        "vers": version,
        "deps": dependencies,
        "cksum": hashlib.sha256(crate.read_bytes()).hexdigest(),
        "features": features,
        "yanked": False,
    }
    if package.get("links"):
        record["links"] = package["links"]
    if package.get("rust-version"):
        record["rust_version"] = package["rust-version"]
    if any(any(token.startswith("dep:") for token in values) for values in features.values()):
        record["features"] = {}
        record["features2"] = features
        record["v"] = 2
    index = (json.dumps(record, separators=(",", ":")) + "\n").encode()
    return name, index, version, crate.name


class LocalCargoRegistry:
    """Expose generated .crate artifacts while proxying normal crates.io reads."""

    def __init__(self, crates: list[Path]):
        self.packages: dict[tuple[str, str], Path] = {}
        self.index: dict[str, bytes] = {}
        for crate in crates:
            name, record, version, _ = cargo_index_record(crate)
            self.packages[(name.lower(), version)] = crate
            path = crate_index_path(name)
            self.index[path] = self.index.get(path, b"") + record
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self) -> None:
                path = self.path.split("?", 1)[0].lstrip("/")
                if path == "config.json":
                    body = json.dumps({"dl": f"http://127.0.0.1:{server.server_port}/crates/{{crate}}/{{version}}/download"}).encode()
                    self.send_response(200)
                    self.send_header("Content-Length", str(len(body)))
                    self.end_headers()
                    self.wfile.write(body)
                    return
                if path.startswith("crates/") and path.endswith("/download"):
                    parts = path.split("/")
                    if len(parts) != 4:
                        self.send_error(404)
                        return
                    key = (parts[1].lower(), parts[2])
                    local = owner.packages.get(key)
                    if local is not None:
                        self._send(local.read_bytes())
                        return
                    if parts[1].lower().startswith("searchkernel"):
                        self.send_error(404, "release crate was not supplied")
                        return
                    self._proxy(f"https://static.crates.io/crates/{parts[1]}/{parts[1]}-{parts[2]}.crate")
                    return
                local_index = owner.index.get(path)
                if local_index is not None:
                    self._send(local_index)
                    return
                if path.rsplit("/", 1)[-1].startswith("searchkernel"):
                    self.send_error(404, "release crate was not supplied")
                    return
                self._proxy(f"https://index.crates.io/{path}")

            def _send(self, body: bytes) -> None:
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def _proxy(self, url: str) -> None:
                try:
                    with urlopen(Request(url, headers={"User-Agent": "searchkernel-consumer-qualifier"}), timeout=60) as response:
                        self._send(response.read())
                except HTTPError as error:
                    self.send_error(error.code)
                except (OSError, URLError):
                    self.send_error(502, "crates.io proxy request failed")

            def log_message(self, _format: str, *_args: object) -> None:
                return

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server = server
        self.thread = threading.Thread(target=server.serve_forever, daemon=True)

    def __enter__(self) -> "LocalCargoRegistry":
        self.thread.start()
        return self

    def __exit__(self, *_args: object) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()


