import { createHash } from "node:crypto";
import { createReadStream, existsSync, readFileSync, rmSync } from "node:fs";
import { mkdir, mkdtemp, readFile, rename, rm, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { SearchKernelError } from "../client/index.js";

const execFileAsync = promisify(execFile);
type Descriptor = Record<string, unknown> & { runtime_identity:string; artifact_url:string; artifact_sha256:string };
function packageRoot(): string { return resolve(__dirname, "..", ".."); }
function descriptor(): Descriptor { return JSON.parse(readFileSync(resolve(packageRoot(), "runtime.json"), "utf8")) as Descriptor; }
function cacheRoot(): string { return process.env.SEARCHKERNEL_RUNTIME_CACHE ?? resolve(process.env.XDG_CACHE_HOME ?? resolve(homedir(), ".cache"), "searchkernel", "runtime"); }
async function sha256(path:string):Promise<string>{ const h=createHash("sha256"); for await(const c of createReadStream(path)) h.update(c); return h.digest("hex"); }
async function validate(root:string,d:Descriptor):Promise<void>{ const m=JSON.parse(await readFile(resolve(root,"manifest.json"),"utf8")) as Record<string,unknown>; for(const k of ["searchkernel_version","searchkernel_revision","runtime_package_generation","platform","native_target","build_environment","native_abi_version","vespa_commit","runtime_identity"]) if(m[k]!==d[k]) throw new Error(`identity mismatch for ${k}`); if(!existsSync(resolve(root,"bin","searchkerneld"))) throw new Error("runtime daemon missing"); }
export async function resolveRuntime():Promise<string>{ const d=descriptor(); const override=process.env.SEARCHKERNEL_RUNTIME_DIR; if(override){await validate(override,d);return resolve(override);} const final=resolve(cacheRoot(),"objects",d.runtime_identity); try{await validate(final,d);return final;}catch{} await mkdir(dirname(final),{recursive:true}); const stage=await mkdtemp(resolve(cacheRoot(),`.${d.runtime_identity}.`)); try { const archive=resolve(stage,"runtime.tar.gz"); const response=await fetch(d.artifact_url); if(!response.ok||!response.body) throw new Error(`download failed: HTTP ${response.status}`); await pipeline(Readable.fromWeb(response.body as never), createWriteStreamCompat(archive)); if(await sha256(archive)!==d.artifact_sha256) throw new Error("artifact digest mismatch"); const unpack=resolve(stage,"unpack"); await mkdir(unpack); await execFileAsync("tar",["-xzf",archive,"-C",unpack,"--no-same-owner","--no-same-permissions"]); await validate(unpack,d); try{await rename(unpack,final);}catch(e){await validate(final,d);} return final; } catch(e){throw new SearchKernelError(`managed-local runtime resolution failed: ${String(e)}`);} finally{await rm(stage,{recursive:true,force:true});} }
function createWriteStreamCompat(path:string){ return (require("node:fs") as typeof import("node:fs")).createWriteStream(path); }
