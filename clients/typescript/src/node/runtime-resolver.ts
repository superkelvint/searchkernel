import { createHash } from "node:crypto";
import { createReadStream, createWriteStream, readFileSync } from "node:fs";
import { lstat, mkdir, mkdtemp, readFile, readdir, readlink, rename, rm, realpath, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, relative, resolve, sep } from "node:path";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { SearchKernelError } from "../client/index.js";

const execFileAsync = promisify(execFile);
const IDENTITY_KEYS = ["searchkernel_version","searchkernel_revision","runtime_package_generation","platform","native_target","build_environment","native_abi_version","vespa_commit","runtime_identity"] as const;
type Descriptor = Record<string, unknown> & { runtime_identity:string; artifact_url:string; artifact_sha256:string };
function packageRoot(): string { return resolve(__dirname, "..", ".."); }
function descriptor(): Descriptor {
  let value: unknown;
  try {
    value = JSON.parse(readFileSync(resolve(packageRoot(), "runtime.json"), "utf8"));
  } catch (error) {
    throw new SearchKernelError(`descriptor-invalid: ${String(error)}`);
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new SearchKernelError("descriptor-invalid: descriptor must be an object");
  }
  const d = value as Descriptor;
  if (d.format_version !== 1) throw new SearchKernelError("descriptor-invalid: unsupported descriptor format");
  if (!d.runtime_identity?.startsWith("skrt-v1-") || !/^[0-9a-f]{64}$/.test(d.artifact_sha256)) {
    throw new SearchKernelError("descriptor-invalid: invalid runtime identity or artifact digest");
  }
  const material: Record<string, unknown> = {};
  for (const key of IDENTITY_KEYS.slice(0, -1).sort()) material[key] = d[key];
  const derived = `skrt-v1-${createHash("sha256").update(JSON.stringify(material)).digest("hex")}`;
  if (derived !== d.runtime_identity) {
    throw new SearchKernelError("descriptor-invalid: runtime identity does not match identity material");
  }
  let glibc = false;
  if (process.platform === "linux" && process.arch === "x64") {
    try {
      const report = process.report?.getReport() as { header?: { glibcVersionRuntime?: string } } | undefined;
      glibc = Boolean(report?.header?.glibcVersionRuntime);
    } catch {
      glibc = false;
    }
  }
  if (!glibc || d.platform !== "linux-x86_64-glibc" || d.native_target !== "x86_64-unknown-linux-gnu") {
    throw new SearchKernelError("platform-mismatch: descriptor does not match a supported glibc host");
  }
  const expected = `searchkernel-runtime-${d.runtime_identity}-${d.platform}.tar.gz`;
  let parsed: URL;
  try {
    parsed = new URL(d.artifact_url);
  } catch {
    throw new SearchKernelError("origin-invalid: artifact_url is invalid");
  }
  if (!["https:", "file:"].includes(parsed.protocol) || decodeURIComponent(parsed.pathname.split("/").pop() ?? "") !== expected) {
    throw new SearchKernelError("origin-invalid: artifact_url is not the immutable runtime artifact");
  }
  return d;
}
function cacheRoot(): string { return process.env.SEARCHKERNEL_RUNTIME_CACHE ?? resolve(process.env.XDG_CACHE_HOME ?? resolve(homedir(), ".cache"), "searchkernel", "runtime"); }
function artifactUrl(d:Descriptor):string { const origin=process.env.SEARCHKERNEL_RUNTIME_ORIGIN; return origin ? `${origin.replace(/\/$/,"")}/${d.artifact_url.split("/").pop()!}` : d.artifact_url; }
async function sha256(path:string):Promise<string>{ const h=createHash("sha256"); for await(const c of createReadStream(path)) h.update(c); return h.digest("hex"); }
function u64(n:number):Buffer { const b=Buffer.alloc(8); b.writeBigUInt64BE(BigInt(n)); return b; }
async function payloadDigest(root: string): Promise<string> {
  const files: string[] = [];
  async function walk(dir: string): Promise<void> {
    for (const name of (await readdir(dir)).sort()) {
      const path = resolve(dir, name);
      const stat = await lstat(path);
      if (stat.isDirectory()) await walk(path);
      else if (name !== "manifest.json") files.push(path);
    }
  }
  await walk(root);
  const base = resolve(root);
  const hash = createHash("sha256");
  for (const path of files.sort()) {
    const rel = relative(base, path).replaceAll("\\", "/");
    let actual = path;
    const entryStat = await lstat(path);
    if (entryStat.isSymbolicLink()) {
      actual = resolve(dirname(path), await readlink(path));
      const target = await realpath(actual);
      if (target !== base && !target.startsWith(`${base}${sep}`)) {
        throw new SearchKernelError("runtime-invalid: runtime symlink escapes runtime root");
      }
      actual = target;
    }
    if ((await stat(actual)).isDirectory()) continue;
    const data = await readFile(actual);
    const name = Buffer.from(rel);
    hash.update(u64(name.length)).update(name).update(u64(data.length)).update(data);
  }
  return hash.digest("hex");
}
async function validate(root: string, d: Descriptor): Promise<string> {
  const manifestPath = resolve(root, "manifest.json");
  try {
    if ((await lstat(manifestPath)).isSymbolicLink()) {
      const actual = await realpath(manifestPath);
      const base = await realpath(root);
      if (actual !== base && !actual.startsWith(`${base}${sep}`)) {
        throw new SearchKernelError("runtime-invalid: runtime manifest symlink escapes runtime root");
      }
    }
  } catch (error) {
    if (error instanceof SearchKernelError) throw error;
    throw new SearchKernelError(`runtime-invalid: manifest unavailable: ${String(error)}`);
  }
  let manifest: Record<string, unknown>;
  try {
    manifest = JSON.parse(await readFile(manifestPath, "utf8")) as Record<string, unknown>;
  } catch (error) {
    throw new SearchKernelError(`runtime-invalid: manifest unavailable: ${String(error)}`);
  }
  if (manifest.format_version !== 1) {
    throw new SearchKernelError("runtime-invalid: unsupported runtime manifest format");
  }
  for (const key of IDENTITY_KEYS) {
    if (manifest[key] !== d[key]) throw new SearchKernelError(`identity-mismatch: ${key} differs`);
  }
  for (const member of ["bin/searchkerneld", "bin/searchkerneld.real", "lib/libsearchkernel_vespa.so.2"]) {
    const path = resolve(root, member);
    try {
      if (!(await stat(path)).isFile()) throw new Error("not a regular file");
      const actual = await realpath(path);
      const base = await realpath(root);
      if (actual !== base && !actual.startsWith(`${base}${sep}`)) {
        throw new SearchKernelError("runtime-invalid: required runtime member escapes runtime root");
      }
    } catch (error) {
      if (error instanceof SearchKernelError) throw error;
      throw new SearchKernelError(`runtime-invalid: required runtime member unavailable: ${member}`);
    }
  }
  for (const member of ["bin/searchkerneld", "bin/searchkerneld.real"]) {
    if (!((await stat(resolve(root, member))).mode & 0o111)) {
      throw new SearchKernelError(`runtime-invalid: required runtime executable is not executable: ${member}`);
    }
  }
  if (manifest.payload_sha256 !== await payloadDigest(root)) {
    throw new SearchKernelError("digest-mismatch: runtime payload digest differs");
  }
  return resolve(root);
}
async function acquireLock(path:string):Promise<()=>Promise<void>>{ await mkdir(dirname(path),{recursive:true}); for(let i=0;i<600;i++){ try{await mkdir(path);return async()=>rm(path,{recursive:true,force:true});}catch(e){if((e as NodeJS.ErrnoException).code!=="EEXIST") throw e; await new Promise(r=>setTimeout(r,50));} } throw new Error("runtime cache lock timeout"); }
export async function resolveRuntime():Promise<string>{ const d=descriptor(); const override=process.env.SEARCHKERNEL_RUNTIME_DIR; if(override) return validate(override,d); const root=cacheRoot(), final=resolve(root,"objects",d.runtime_identity); try{return await validate(final,d);}catch{} const unlock=await acquireLock(resolve(root,"locks",`${d.runtime_identity}.lock`)); try { try{return await validate(final,d);}catch{await rm(final,{recursive:true,force:true});} await mkdir(resolve(root,"staging"),{recursive:true}); const stage=await mkdtemp(resolve(root,"staging",`${d.runtime_identity}.`)); try { const archive=resolve(stage,"runtime.tar.gz"); const url=artifactUrl(d); if(url.startsWith("file:")) await pipeline(createReadStream(fileURLToPath(url)),createWriteStream(archive)); else { const response=await fetch(url); if(!response.ok||!response.body) throw new Error(`acquire-failed: HTTP ${response.status}`); await pipeline(Readable.fromWeb(response.body as never),createWriteStream(archive)); } if(await sha256(archive)!==d.artifact_sha256) throw new Error("artifact-digest-mismatch: downloaded archive digest differs"); const {stdout}=await execFileAsync("tar",["-tzf",archive]); for(const name of stdout.split("\n").filter(Boolean)){ if(name.startsWith("/")||name.split("/").includes("..")) throw new Error("artifact-invalid: archive path escapes runtime root"); } const unpack=resolve(stage,"unpack"); await mkdir(unpack); await execFileAsync("tar",["-xzf",archive,"-C",unpack,"--no-same-owner","--no-same-permissions"]); await validate(unpack,d); await mkdir(dirname(final),{recursive:true}); await rename(unpack,final); return await validate(final,d); } finally{await rm(stage,{recursive:true,force:true});} } catch(e){throw e instanceof SearchKernelError ? e : new SearchKernelError(`managed-local runtime resolution failed: ${String(e)}`);} finally{await unlock();} }
