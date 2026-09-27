const { execFileSync } = require("node:child_process");
const { mkdirSync, mkdtempSync, rmSync, writeFileSync } = require("node:fs");
const { createRequire } = require("node:module");
const { tmpdir } = require("node:os");
const { join, resolve } = require("node:path");

function fail(message) {
  throw new Error(message);
}

async function main() {
  const hostBinary = process.env.SEARCHKERNEL_LOCAL_HOST;
  if (!hostBinary) fail("SEARCHKERNEL_LOCAL_HOST is required for installed-package E2E");
  const npm = process.env.NPM;
  if (!npm) fail("NPM is required from the canonical runtime launcher");

  const packageRoot = resolve(__dirname, "..");
  const temporaryRoot = mkdtempSync(join(tmpdir(), "searchkernel-typescript-installed-e2e-"));

  try {
    const packed = JSON.parse(execFileSync(
      npm,
      ["pack", "--json", "--ignore-scripts", "--pack-destination", temporaryRoot],
      { cwd: packageRoot, encoding: "utf8" },
    ));
    const filename = packed[0]?.filename;
    if (!filename) fail("npm pack did not report a tarball filename");

    const consumer = join(temporaryRoot, "consumer");
    mkdirSync(consumer);
    const consumerPackage = join(consumer, "package.json");
    writeFileSync(
      consumerPackage,
      JSON.stringify({ name: "searchkernel-installed-e2e", private: true, version: "0.0.0" }),
    );
    execFileSync(
      npm,
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

    const requireFromConsumer = createRequire(consumerPackage);
    const { Index, Text } = requireFromConsumer("@searchkernel/client/node");
    const indexPath = join(temporaryRoot, "managed-index");

    const created = await Index.create(indexPath, {
      fields: {
        title: new Text({ stored: true, searchable: true }),
      },
    });
    await created.close();

    const reopened = await Index.open(indexPath);
    await reopened.close();
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
