import { createHash } from "node:crypto";
import { createReadStream, createWriteStream, existsSync, readFileSync } from "node:fs";
import { lstat, mkdir, mkdtemp, readFile, readdir, readlink, rename, rm } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, relative, resolve } from "node:path";
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
function descriptor(): Descriptor { const d=JSON.parse(readFileSync(resolve(packageRoot(), "runtime.json"), "utf8")) as Descriptor; if(!d.runtime_identity?.startsWith("skrt-v1-") || !/^[0-9a-f]{64}$/.test(d.artifact_sha256)) throw new SearchKernelError("descriptor-invalid: invalid runtime identity or artifact digest"); const material:Record<string,unknown>={}; for(const k of IDENTITY_KEYS.slice(0,-1).sort()) material[k]=d[k]; const derived="skrt-v1-"+createHash("sha256").update(JSON.stringify(material)).digest("hex"); if(derived!==d.runtime_identity) throw new SearchKernelError("descriptor-invalid: runtime identity does not match identity material"); if(process.platform!=="linux"||process.arch!=="x64"||d.platform!=="linux-x86_64-glibc"||d.native_target!=="x86_64-unknown-linux-gnu") throw new SearchKernelError("platform-unsupported: runtime descriptor does not match this host"); const expected=`searchkernel-runtime-${d.runtime_identity}-${d.platform}.tar.gz`; let parsed:URL; try{parsed=new URL(d.artifact_url);}catch{throw new SearchKernelError("descriptor-invalid: artifact_url is invalid");} if(!["https:","file:"].includes(parsed.protocol)||parsed.pathname.split("/").pop()!==expected) throw new SearchKernelError("descriptor-invalid: artifact_url is not the immutable runtime artifact"); return d; }
function cacheRoot(): string { return process.env.SEARCHKERNEL_RUNTIME_CACHE ?? resolve(process.env.XDG_CACHE_HOME ?? resolve(homedir(), ".cache"), "searchkernel", "runtime"); }
function artifactUrl(d:Descriptor):string { const origin=process.env.SEARCHKERNEL_RUNTIME_ORIGIN; return origin ? `${origin.replace(/\/$/,"")}/${d.artifact_url.split("/").pop()!}` : d.artifact_url; }
async function sha256(path:string):Promise<string>{ const h=createHash("sha256"); for await(const c of createReadStream(path)) h.update(c); return h.digest("hex"); }
function u64(n:number):Buffer { const b=Buffer.alloc(8); b.writeBigUInt64BE(BigInt(n)); return b; }
async function payloadDigest(root:string):Promise<string>{ const files:string[]=[]; async function walk(dir:string){ for(const name of (await readdir(dir)).sort()){ const path=resolve(dir,name); const st=await lstat(path); if(st.isDirectory()) await walk(path); else if(name!=="manifest.json") files.push(path); } } await walk(root); const h=createHash("sha256"); for(const path of files.sort()){ const rel=relative(root,path).replaceAll("\\","/"); let actual=path; const st=await lstat(path); if(st.isSymbolicLink()){ actual=resolve(dirname(path),await readlink(path)); if(relative(root,actual).startsWith("..")) throw new Error("runtime symlink escapes root"); } const data=await readFile(actual); const rb=Buffer.from(rel); h.update(u64(rb.length));h.update(rb);h.update(u64(data.length));h.update(data); } return h.digest("hex"); }
async function validate(root:string,d:Descriptor):Promise<string>{ try { const m=JSON.parse(await readFile(resolve(root,"manifest.json"),"utf8")) as Record<string,unknown>; for(const k of IDENTITY_KEYS) if(m[k]!==d[k]) throw new Error(`identity-mismatch: ${k} differs`); for(const member of ["bin/searchkerneld","bin/searchkerneld.real","lib/libsearchkernel_vespa.so.2"]) if(!existsSync(resolve(root,member))) throw new Error(`runtime-invalid: required runtime member missing: ${member}`); if(m.payload_sha256!==await payloadDigest(root)) throw new Error("digest-mismatch: runtime payload digest differs"); return resolve(root); } catch(e){ throw new SearchKernelError(`runtime-invalid: ${String(e)}`); } }
async function resolveRuntimeUnderLock(d:Descriptor):Promise<string>{ const root=cacheRoot(), final=resolve(root,"objects",d.runtime_identity); try{return await validate(final,d);}catch{} await rm(final,{recursive:true,force:true}); await mkdir(resolve(root,"staging"),{recursive:true}); const stage=await mkdtemp(resolve(root,"staging",`${d.runtime_identity}.`)); try { const archive=resolve(stage,"runtime.tar.gz"); const url=artifactUrl(d); if(url.startsWith("file:")) await pipeline(createReadStream(fileURLToPath(url)),createWriteStream(archive)); else { const response=await fetch(url,{signal:AbortSignal.timeout(30_000)}); if(!response.ok||!response.body) throw new Error(`acquire-failed: HTTP ${response.status}`); await pipeline(Readable.fromWeb(response.body as never),createWriteStream(archive)); } if(await sha256(archive)!==d.artifact_sha256) throw new Error("artifact-digest-mismatch: downloaded archive digest differs"); const {stdout}=await execFileAsync("tar",["-tzf",archive]); for(const name of stdout.split("\n").filter(Boolean)){ if(name.startsWith("/")||name.split("/").includes("..")) throw new Error("artifact-invalid: archive path escapes runtime root"); } const unpack=resolve(stage,"unpack"); await mkdir(unpack); await execFileAsync("tar",["-xzf",archive,"-C",unpack,"--no-same-owner","--no-same-permissions"]); await validate(unpack,d); await mkdir(dirname(final),{recursive:true}); await rename(unpack,final); return await validate(final,d); } finally{await rm(stage,{recursive:true,force:true});} }

/**
 * Run the cache mutation in a `flock` child so it shares the persistent file
 * lock used by Python and Ruby. Keeping the lock in the worker process means
 * the kernel releases it on termination; no stale lock directory can wedge a
 * later consumer.
 */
export async function runRuntimeResolutionWorker():Promise<void>{
  try { process.stdout.write(await resolveRuntimeUnderLock(descriptor())); }
  catch(e) { process.stderr.write(e instanceof Error ? e.message : String(e)); process.exitCode=1; }
}

export async function resolveRuntime():Promise<string>{
  const d=descriptor(); const override=process.env.SEARCHKERNEL_RUNTIME_DIR; if(override) return validate(override,d);
  const root=cacheRoot(), final=resolve(root,"objects",d.runtime_identity); try{return await validate(final,d);}catch{}
  const lockPath=resolve(root,"locks",`${d.runtime_identity}.lock`); await mkdir(dirname(lockPath),{recursive:true});
  const worker=`require(${JSON.stringify(__filename)}).runRuntimeResolutionWorker()`;
  try {
    const {stdout}=await execFileAsync("flock",["--exclusive",lockPath,process.execPath,"-e",worker],{timeout:300_000});
    return stdout.trim();
  } catch(e) {
    const diagnostic=e instanceof Error ? e.message : String(e);
    throw new SearchKernelError(`managed-local runtime resolution failed: ${diagnostic}`);
  }
}
