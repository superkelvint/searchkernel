import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { environmentForHost } from "../src/node/managed-local-host.js";


describe("managed-local native runtime contract", () => {
  let sdkLib: string | undefined;
  let loaderPath: string | undefined;

  beforeEach(() => {
    sdkLib = process.env.SEARCHKERNEL_SDK_LIB;
    loaderPath = process.env.LD_LIBRARY_PATH;
  });

  afterEach(() => {
    if (sdkLib === undefined) delete process.env.SEARCHKERNEL_SDK_LIB;
    else process.env.SEARCHKERNEL_SDK_LIB = sdkLib;
    if (loaderPath === undefined) delete process.env.LD_LIBRARY_PATH;
    else process.env.LD_LIBRARY_PATH = loaderPath;
  });

  it("does not probe the retired monolithic native SDK path", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/node/managed-local-host.ts"),
      "utf8",
    );

    expect(source).not.toContain('"native",\n      "sdk"');
    expect(source).not.toContain("core/native/sdk");
  });

  it("leaves the inherited loader path alone without an explicit adapter path", () => {
    delete process.env.SEARCHKERNEL_SDK_LIB;
    process.env.LD_LIBRARY_PATH = "/existing";

    expect(environmentForHost().LD_LIBRARY_PATH).toBe("/existing");
  });

  it("prepends the exact explicit adapter path without mutating process env", () => {
    process.env.SEARCHKERNEL_SDK_LIB = "/exact/native-adapter/lib";
    process.env.LD_LIBRARY_PATH = "/existing";

    const environment = environmentForHost();

    expect(environment.LD_LIBRARY_PATH).toBe(
      "/exact/native-adapter/lib:/existing",
    );
    expect(process.env.LD_LIBRARY_PATH).toBe("/existing");
  });
});
