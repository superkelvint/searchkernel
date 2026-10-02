const { execFileSync } = require("node:child_process");
const { mkdirSync, mkdtempSync, rmSync, writeFileSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join, resolve } = require("node:path");

function fail(message) {
  console.error(message);
  process.exit(1);
}

const packageRoot = resolve(__dirname, "..");
const packedDryRun = JSON.parse(execFileSync(
  "npm",
  ["pack", "--dry-run", "--json", "--ignore-scripts"],
  { cwd: packageRoot, encoding: "utf8" },
));
const files = new Set((packedDryRun[0]?.files ?? []).map((file) => file.path));
for (const required of ["dist/index.js", "dist/index.d.ts", "dist/node.js", "dist/node.d.ts"]) {
  if (!files.has(required)) fail(`packed package is missing ${required}`);
}

require("@searchkernel/client");
try {
  require("@searchkernel/client/low-level");
  fail("package unexpectedly exports generated FlatBuffers bindings");
} catch (error) {
  if (error?.code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") throw error;
}
require("@searchkernel/client/node");

async function verifyInstalledManagedLocalContract() {
  const temporaryRoot = mkdtempSync(join(tmpdir(), "searchkernel-typescript-package-"));
  try {
    const packed = JSON.parse(execFileSync(
      "npm",
      ["pack", "--json", "--ignore-scripts", "--pack-destination", temporaryRoot],
      { cwd: packageRoot, encoding: "utf8" },
    ));
    const filename = packed[0]?.filename;
    if (!filename) fail("npm pack did not report a tarball filename");

    const consumer = join(temporaryRoot, "consumer");
    mkdirSync(consumer);
    writeFileSync(
      join(consumer, "package.json"),
      JSON.stringify({ name: "searchkernel-package-consumer", private: true, version: "0.0.0" }),
    );
    execFileSync(
      "npm",
      [
        "install",
        "--ignore-scripts",
        "--no-audit",
        "--no-fund",
        "--package-lock=false",
        join(temporaryRoot, filename),
      ],
      { cwd: consumer, stdio: "inherit" },
    );

    const probe = join(consumer, "verify-managed-local.cjs");
    writeFileSync(
      probe,
      `
const { Index } = require("@searchkernel/client/node");

delete process.env.SEARCHKERNEL_LOCAL_HOST;

(async () => {
  try {
    await Index.open("./search");
    console.error("installed package unexpectedly resolved a managed-local daemon");
    process.exit(1);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!message.includes("managed local host binary is not bundled with @searchkernel/client")) {
      console.error("installed package failed for the wrong reason:", message);
      process.exit(1);
    }
    if (!message.includes("set SEARCHKERNEL_LOCAL_HOST")) {
      console.error("installed package error omitted the supported prerequisite:", message);
      process.exit(1);
    }
    if (message.includes("node_modules/target") || message.includes("/target/debug/searchkerneld")) {
      console.error("installed package leaked a repository-only daemon fallback:", message);
      process.exit(1);
    }
  }
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
`,
    );
    execFileSync(process.execPath, [probe], { cwd: consumer, stdio: "inherit" });
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

(async () => {
  await import("@searchkernel/client");
  try {
    await import("@searchkernel/client/low-level");
    fail("package unexpectedly exports generated FlatBuffers bindings");
  } catch (error) {
    if (error?.code !== "ERR_PACKAGE_PATH_NOT_EXPORTED") throw error;
  }
  await import("@searchkernel/client/node");
  await verifyInstalledManagedLocalContract();
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
