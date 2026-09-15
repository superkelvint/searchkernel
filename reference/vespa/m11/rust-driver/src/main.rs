use searchkernel_vespa::{
    hardening::{self, Action},
    ConfigCompiler, Engine, Field, FieldType, Query, Schema, TextDocument, TextQuery,
};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const SEED: u64 = 0x4d11_2026_0914;
type AnyResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn schema() -> Schema {
    Schema::new("typea")
        .field(Field::new("title", FieldType::Text).indexed())
        .field(Field::new("qty", FieldType::Integer).attribute())
}

fn prepare_config(path: &Path) -> AnyResult<()> {
    if path.is_dir() {
        return Ok(());
    }
    Ok(ConfigCompiler.compile_to(&schema(), path)?)
}

fn open(config: &Path, data: &Path) -> AnyResult<Engine> {
    prepare_config(config)?;
    fs::create_dir_all(data)?;
    Ok(Engine::open(data, config)?)
}

fn error_message(error: impl std::fmt::Display) -> String {
    error.to_string().replace('\n', " ")
}

fn lifecycle(config: &Path, root: &Path) -> AnyResult<()> {
    for cycle in 0..1_000 {
        let data = root.join(format!("cycle-{cycle}"));
        let engine = open(config, &data)?;
        drop(engine);
    }
    println!("lifecycle cycles=1000");
    Ok(())
}

fn reopen(config: &Path, data: &Path) -> AnyResult<()> {
    let mut expected = BTreeMap::new();
    for cycle in 0..250 {
        let engine = open(config, data)?;
        let id = format!("reopen-{cycle:04}");
        engine.put(&id, "qty", cycle)?;
        expected.insert(id, cycle);
        engine.close()?;

        let engine = open(config, data)?;
        for (id, value) in &expected {
            let hits = engine.search("qty", *value, 1_000_000)?;
            if !hits.iter().any(|hit| hit == id) {
                panic!("reopen cycle {cycle} lost {id}");
            }
        }
        engine.close()?;
    }
    println!("reopen cycles=250 documents={}", expected.len());
    Ok(())
}

fn malformed(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = Arc::new(open(config, data)?);
    let cases = [
        engine.put("", "qty", 1).err().map(error_message),
        engine.put("bad", "missing", 1).err().map(error_message),
        engine.search("missing", 1, 1).err().map(error_message),
        engine
            .search_text(TextQuery::Term { field: "qty", text: "x" }, "default")
            .err()
            .map(error_message),
        engine
            .search_query(
                &Query::And(Vec::new()),
                "default",
            )
            .err()
            .map(error_message),
        Query::from_yql("select * from sources * where qty = 999999999999")
            .err()
            .map(error_message),
    ];
    if cases.iter().any(Option::is_none) {
        panic!("malformed input unexpectedly succeeded");
    }
    let invalid_query = Query::Term {
        field: "missing".to_owned(),
        text: "x".to_owned(),
    };
    if engine.search_query(&invalid_query, "default").is_ok() {
        panic!("unknown query field unexpectedly succeeded");
    }
    let mut deep = Query::Term {
        field: "qty".to_owned(),
        text: "1".to_owned(),
    };
    for _ in 0..257 {
        deep = Query::Not(Box::new(deep));
    }
    if engine.search_query(&deep, "default").is_ok() {
        panic!("deep query unexpectedly succeeded");
    }
    let wide = Query::And(
        (0..65_537)
            .map(|_| Query::Equals {
                field: "qty".to_owned(),
                value: 1,
            })
            .collect(),
    );
    if engine.search_query(&wide, "default").is_ok() {
        panic!("wide query unexpectedly succeeded");
    }
    if Query::from_yql(&"x".repeat(4 * 1024 * 1024 + 1)).is_ok() {
        panic!("oversized YQL unexpectedly succeeded");
    }
    println!("malformed cases=10 controlled=true");
    Ok(())
}

fn failures(config: &Path, data: &Path) -> AnyResult<()> {
    hardening::set_open("open_before_native_create", 1, Action::StdException)?;
    let open_error = match Engine::open(data, config) {
        Ok(_) => panic!("open failpoint must fail"),
        Err(error) => error,
    };
    if open_error.code() != 2 {
        panic!("std exception code {} != 2", open_error.code());
    }
    hardening::clear_open("open_before_native_create")?;

    hardening::set_open("open_after_native_create", 1, Action::StdException)?;
    if Engine::open(data, config).is_ok() {
        panic!("post-create open failpoint must fail");
    }
    hardening::clear_open("open_after_native_create")?;

    let engine = Arc::new(open(config, data)?);
    hardening::set(&engine, "mutation_before_native_call", 1, Action::Error)?;
    if engine.put("before", "qty", 7).is_ok() {
        panic!("before-call failure unexpectedly succeeded");
    }
    if !engine.search("qty", 7, 10)?.is_empty() {
        panic!("before-call failure changed state");
    }

    hardening::set(&engine, "mutation_after_native_call", 1, Action::StdException)?;
    if engine.put("after", "qty", 8).is_ok() {
        panic!("after-call failure unexpectedly succeeded");
    }
    if !engine.search("qty", 8, 10)?.iter().any(|id| id == "after") {
        panic!("after-call failure incorrectly rolled back native mutation");
    }

    for (name, action, expected) in [
        ("search_before_match", Action::StdException, 2),
        ("search_after_match", Action::UnknownException, 2),
        ("result_after_allocation", Action::BadAlloc, 3),
        ("result_before_copy", Action::StdException, 2),
    ] {
        hardening::set(&engine, name, 1, action)?;
        let error = engine.search("qty", 8, 10).expect_err("failure injection must fail");
        if error.code() != expected {
            panic!("failpoint returned code {}, expected {expected}", error.code());
        }
    }
    hardening::set(&engine, "search_before_match", 1, Action::Block)?;
    let blocked_engine = Arc::clone(&engine);
    let blocked_search = thread::spawn(move || blocked_engine.search("qty", 8, 10));
    for _ in 0..100 {
        if hardening::is_blocked(&engine, "search_before_match")? {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    if !hardening::is_blocked(&engine, "search_before_match")? {
        panic!("block failpoint did not block a search");
    }
    hardening::release(&engine, "search_before_match")?;
    blocked_search.join().map_err(|_| "blocked search panicked")??;
    hardening::set(&engine, "destroy_before_native_release", 1, Action::Error)?;
    drop(Arc::try_unwrap(engine).map_err(|_| "engine owners remain")?);
    println!("failures exception_barrier=true cleanup=true");
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct ResourceSnapshot {
    fds: u64,
    threads: u64,
    pss_kib: u64,
    files: u64,
    bytes: u64,
}

fn resource_files(root: &Path) -> AnyResult<Vec<std::fs::DirEntry>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    let mut stack = vec![root.to_owned()];
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                stack.push(entry.path());
            } else if entry.file_type()?.is_file() {
                files.push(entry);
            }
        }
    }
    Ok(files)
}

fn resource_snapshot(data: &Path) -> AnyResult<ResourceSnapshot> {
    let fds = fs::read_dir("/proc/self/fd")?.count() as u64;
    let status = fs::read_to_string("/proc/self/status")?;
    let threads = status
        .lines()
        .find_map(|line| line.strip_prefix("Threads:").and_then(|v| v.trim().parse().ok()))
        .ok_or("missing /proc/self/status Threads")?;
    let smaps = fs::read_to_string("/proc/self/smaps_rollup")?;
    let pss_kib = smaps
        .lines()
        .find_map(|line| line.strip_prefix("Pss:").and_then(|v| v.split_whitespace().next()))
        .ok_or("missing /proc/self/smaps_rollup Pss")?
        .parse()?;
    let files = resource_files(data)?;
    let bytes = files.iter().try_fold(0_u64, |total, entry| {
        Ok::<_, std::io::Error>(total.saturating_add(entry.metadata()?.len()))
    })?;
    Ok(ResourceSnapshot {
        fds,
        threads,
        pss_kib,
        files: files.len() as u64,
        bytes,
    })
}

fn resources(config: &Path, data: &Path) -> AnyResult<()> {
    for cycle in 0..5 {
        let engine = open(config, data)?;
        if cycle == 0 {
            engine.put("resource", "qty", 1)?;
        }
        engine.search("qty", 1, 10)?;
        engine.close()?;
    }
    let baseline = resource_snapshot(data)?;
    for _ in 0..100 {
        let engine = open(config, data)?;
        engine.search("qty", 1, 10)?;
        engine.close()?;
    }
    let final_state = resource_snapshot(data)?;
    println!(
        "{{\"baseline_fds\":{},\"final_fds\":{},\"baseline_threads\":{},\"final_threads\":{},\"baseline_pss_kib\":{},\"final_pss_kib\":{},\"baseline_files\":{},\"final_files\":{},\"baseline_bytes\":{},\"final_bytes\":{}}}",
        baseline.fds,
        final_state.fds,
        baseline.threads,
        final_state.threads,
        baseline.pss_kib,
        final_state.pss_kib,
        baseline.files,
        final_state.files,
        baseline.bytes,
        final_state.bytes
    );
    Ok(())
}

fn next_random(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    *state
}

fn stress(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = open(config, data)?;
    let mut model = BTreeMap::new();
    let mut state = SEED;
    for operation in 0..100_000u32 {
        let slot = (next_random(&mut state) % 128) as u32;
        let id = format!("stress-{slot:03}");
        match next_random(&mut state) % 4 {
            0 => {
                let value = (next_random(&mut state) % 32) as i32;
                engine.put(&id, "qty", value)?;
                model.insert(id, value);
            }
            1 if model.contains_key(&id) => {
                let value = (next_random(&mut state) % 32) as i32;
                engine.update(&id, "qty", value)?;
                model.insert(id, value);
            }
            2 if model.remove(&id).is_some() => {
                engine.remove(&id)?;
            }
            _ => {
                let value = (next_random(&mut state) % 32) as i32;
                if model.contains_key(&id) {
                    engine.update(&id, "qty", value)?;
                } else {
                    engine.put(&id, "qty", value)?;
                }
                model.insert(id, value);
            }
        }
        if operation % 10_000 == 0 {
            for value in 0..32 {
                let mut expected: Vec<_> = model
                    .iter()
                    .filter_map(|(id, actual)| (*actual == value).then_some(id.clone()))
                    .collect();
                expected.sort();
                let mut actual = engine.search("qty", value, 1_000_000)?;
                actual.sort();
                if actual != expected {
                    panic!("seed={SEED} operation={operation} state mismatch");
                }
            }
        }
    }
    println!("stress seed={SEED} operations=100000 documents={}", model.len());
    Ok(())
}

fn concurrency(config: &Path, data: &Path) -> AnyResult<()> {
    let engine = Arc::new(open(config, data)?);
    engine.put_text(TextDocument { id: "stable-text", field: "title", text: "stable fox" })?;
    let mut workers = Vec::new();
    for worker in 0..8u32 {
        let engine = Arc::clone(&engine);
        workers.push(thread::spawn(move || -> AnyResult<()> {
            for operation in 0..50_000u32 {
                let slot = (operation / 4) % 100;
                let id = format!("writer-{worker:02}-{slot:03}");
                match operation % 4 {
                    0 => engine.put(&id, "qty", 1)?,
                    1 => engine.update(&id, "qty", 2)?,
                    2 => {
                        engine.remove(&id)?;
                    }
                    _ => engine.put(&id, "qty", 3)?,
                }
            }
            Ok(())
        }));
    }
    let mut readers = Vec::new();
    for _ in 0..4 {
        let engine = Arc::clone(&engine);
        readers.push(thread::spawn(move || -> AnyResult<()> {
            for _ in 0..50_000 {
                let _ = engine.search("qty", 3, 1_000)?;
                let result = engine.search_text(
                    TextQuery::Term { field: "title", text: "fox" },
                    "default",
                )?;
                if result.hits.iter().any(|hit| hit.document_id != "stable-text") {
                    panic!("impossible text hit");
                }
            }
            Ok(())
        }));
    }
    for worker in workers {
        worker.join().map_err(|_| "mutation worker panicked")??;
    }
    for reader in readers {
        reader.join().map_err(|_| "search worker panicked")??;
    }
    let engine = Arc::try_unwrap(engine)
        .map_err(|_| "engine owners remain")?;
    let final_hits = engine.search("qty", 3, 1_000_000)?;
    if final_hits.len() != 8 * 100 {
        panic!("final state has {} qty=3 documents", final_hits.len());
    }
    engine.close()?;
    println!("concurrency mutation_workers=8 search_workers=4 operations=800000");
    Ok(())
}

fn main() {
    let mut args = env::args_os();
    let _program = args.next();
    let mode = args.next().expect("mode");
    let config = PathBuf::from(args.next().expect("config directory"));
    let data = PathBuf::from(args.next().expect("data directory"));
    let result = match mode.to_string_lossy().as_ref() {
        "lifecycle" => lifecycle(&config, &data),
        "reopen" => reopen(&config, &data),
        "malformed" => malformed(&config, &data),
        "failures" => failures(&config, &data),
        "resources" => resources(&config, &data),
        "stress" => stress(&config, &data),
        "concurrency" => concurrency(&config, &data),
        other => Err(format!("unknown mode {other}").into()),
    };
    if let Err(error) = result {
        eprintln!("m11 failure: {error}");
        std::process::exit(1);
    }
}
