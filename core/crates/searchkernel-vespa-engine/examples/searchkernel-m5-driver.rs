use searchkernel_vespa_engine::{Document, DocumentUpdate, Engine, Error, Query, SearchRequest};
use std::error::Error as StdError;
use std::io::Write;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

const DOC1: &str = "id:test:typea::1";
const DOC2: &str = "id:test:typea::2";
const DOC3: &str = "id:test:typea::3";
const DOC4: &str = "id:test:typea::4";
const REPEAT_DOC: &str = "id:test:typea::repeat";

fn open(data_dir: &Path, config_dir: &Path) -> Result<Engine> {
    Engine::open(data_dir, config_dir).map_err(|error| Box::new(error) as Box<dyn StdError>)
}

fn search(engine: &Engine, value: i64) -> Result<Vec<String>> {
    let mut ids: Vec<String> = engine
        .search(SearchRequest::new(Query::equals("attr1", value)).limit(10))?
        .hits
        .into_iter()
        .map(|hit| hit.document_id)
        .collect();
    ids.sort();
    Ok(ids)
}

fn array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

fn put(engine: &Engine, id: &str, value: i64) -> Result<()> {
    engine.put(Document::new(id).field("attr1", value))?;
    Ok(())
}

fn update(engine: &Engine, id: &str, value: i64) -> Result<()> {
    engine.update(DocumentUpdate::new(id).assign("attr1", value))?;
    Ok(())
}

fn graceful_seed(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    put(&engine, DOC1, 10)?;
    put(&engine, DOC2, 20)?;
    put(&engine, DOC3, 30)?;

    let after_put_10 = search(&engine, 10)?;
    let after_put_20 = search(&engine, 20)?;
    let after_put_30 = search(&engine, 30)?;

    update(&engine, DOC2, 10)?;
    let was_found = engine.delete(DOC3)?;
    let after_update_delete_10 = search(&engine, 10)?;
    let after_update_delete_20 = search(&engine, 20)?;
    let after_update_delete_30 = search(&engine, 30)?;

    println!(
        "{{\"scenario\":\"m5-graceful-seed\",\"after_put\":{{\"search_10\":{},\"search_20\":{},\"search_30\":{}}},\"remove_doc3\":{},\"after_update_delete\":{{\"search_10\":{},\"search_20\":{},\"search_30\":{}}}}}",
        array(&after_put_10),
        array(&after_put_20),
        array(&after_put_30),
        if was_found { "true" } else { "false" },
        array(&after_update_delete_10),
        array(&after_update_delete_20),
        array(&after_update_delete_30),
    );
    Ok(())
}

fn graceful_verify(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    let search_10 = search(&engine, 10)?;
    let search_20 = search(&engine, 20)?;
    let search_30 = search(&engine, 30)?;
    println!(
        "{{\"scenario\":\"m5-graceful-verify\",\"search_10\":{},\"search_20\":{},\"search_30\":{}}}",
        array(&search_10),
        array(&search_20),
        array(&search_30),
    );
    Ok(())
}

fn graceful_mutate(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    update(&engine, DOC1, 40)?;
    put(&engine, DOC4, 10)?;
    let search_10 = search(&engine, 10)?;
    let search_40 = search(&engine, 40)?;
    println!(
        "{{\"scenario\":\"m5-graceful-mutate\",\"search_10\":{},\"search_40\":{}}}",
        array(&search_10),
        array(&search_40),
    );
    Ok(())
}

fn graceful_final(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    let search_10 = search(&engine, 10)?;
    let search_20 = search(&engine, 20)?;
    let search_30 = search(&engine, 30)?;
    let search_40 = search(&engine, 40)?;
    println!(
        "{{\"scenario\":\"m5-graceful-final\",\"search_10\":{},\"search_20\":{},\"search_30\":{},\"search_40\":{}}}",
        array(&search_10),
        array(&search_20),
        array(&search_30),
        array(&search_40),
    );
    Ok(())
}

fn repeated_reopen(data_dir: &Path, config_dir: &Path, cycles: usize) -> Result<()> {
    if cycles == 0 {
        return Err("repeat requires at least one cycle".into());
    }

    {
        let engine = open(data_dir, config_dir)?;
        put(&engine, REPEAT_DOC, 0)?;
    }

    for cycle in 1..=cycles {
        let engine = open(data_dir, config_dir)?;
        let value = i64::try_from(cycle)?;
        update(&engine, REPEAT_DOC, value)?;
        let hits = search(&engine, value)?;
        if hits != [REPEAT_DOC.to_owned()] {
            return Err(format!("cycle {cycle} returned unexpected hits: {hits:?}").into());
        }
    }

    let engine = open(data_dir, config_dir)?;
    let final_hits = search(&engine, i64::try_from(cycles)?)?;
    println!(
        "{{\"scenario\":\"m5-repeated-reopen\",\"cycles\":{},\"final_search\":{}}}",
        cycles,
        array(&final_hits),
    );
    Ok(())
}

fn crash_update(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    update(&engine, DOC1, 40)?;
    std::io::stdout().flush()?;
    std::process::abort();
}

fn crash_verify(data_dir: &Path, config_dir: &Path) -> Result<()> {
    let engine = open(data_dir, config_dir)?;
    let search_10 = search(&engine, 10)?;
    let search_40 = search(&engine, 40)?;
    println!(
        "{{\"scenario\":\"m5-crash-recovery\",\"search_10\":{},\"search_40\":{}}}",
        array(&search_10),
        array(&search_40),
    );
    Ok(())
}

fn usage() -> &'static str {
    "usage: m5-driver <seed|verify|mutate|verify-final|repeat|crash-update|crash-verify|open-only> DATA_DIR CONFIG_DIR [CYCLES]"
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().ok_or(usage())?.to_string_lossy().into_owned();
    let data_dir = PathBuf::from(args.next().ok_or(usage())?);
    let config_dir = PathBuf::from(args.next().ok_or(usage())?);
    let cycles = args.next();
    if mode == "repeat" {
        let cycles = cycles.ok_or(usage())?.to_string_lossy().parse::<usize>()?;
        if args.next().is_some() {
            return Err(usage().into());
        }
        return repeated_reopen(&data_dir, &config_dir, cycles);
    }
    if cycles.is_some() || args.next().is_some() {
        return Err(usage().into());
    }

    match mode.as_str() {
        "seed" => graceful_seed(&data_dir, &config_dir),
        "verify" => graceful_verify(&data_dir, &config_dir),
        "mutate" => graceful_mutate(&data_dir, &config_dir),
        "verify-final" => graceful_final(&data_dir, &config_dir),
        "crash-update" => crash_update(&data_dir, &config_dir),
        "crash-verify" => crash_verify(&data_dir, &config_dir),
        "open-only" => {
            let _engine = open(&data_dir, &config_dir)?;
            println!("{{\"scenario\":\"m5-open-only\"}}");
            Ok(())
        }
        _ => Err(usage().into()),
    }
}

fn main() {
    if let Err(error) = run() {
        let error = error
            .downcast_ref::<Error>()
            .map(ToString::to_string)
            .unwrap_or_else(|| error.to_string());
        eprintln!("{error}");
        std::process::exit(1);
    }
}
