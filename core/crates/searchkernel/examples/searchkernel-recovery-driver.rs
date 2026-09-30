use searchkernel::{Field, IndexDefinition, IndexRuntime};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type AnyResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const BASE_1: &str = "recovery-base-1";
const BASE_2: &str = "recovery-base-2";
const BASE_3: &str = "recovery-base-3";
const INSERT_TARGET: &str = "recovery-insert-target";
const UPDATE_VALUE: i64 = 40;
const MATRIX_REPETITIONS: usize = 3;
const REPEATED_CYCLES: usize = 20;

fn insert_target(cycle: usize) -> String {
    if cycle == usize::MAX {
        INSERT_TARGET.to_owned()
    } else {
        format!("recovery-insert-{cycle}")
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Insert,
    Update,
    Delete,
    Close,
}

impl Operation {
    fn name(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Close => "close",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Before,
    After,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

type State = BTreeMap<String, i64>;
type Identity = Vec<(PathBuf, Vec<u8>)>;

fn product_definition() -> IndexDefinition {
    IndexDefinition::new()
        .name("recovery")
        .field(Field::text("title").stored(true))
        .field(Field::int64("qty").stored(true).filterable(true))
}

fn product_document(id: &str, qty: i64) -> JsonValue {
    serde_json::json!({"title": format!("recovery document {id}"), "qty": qty})
}

fn seed_state() -> State {
    State::from([
        (BASE_1.to_owned(), 10),
        (BASE_2.to_owned(), 20),
        (BASE_3.to_owned(), 30),
    ])
}

fn identity(root: &Path) -> AnyResult<Identity> {
    let relative = [
        "manifest/index.json",
        "manifest/index-definition.bfbs",
        "manifest/portable-index-definition.bfbs",
        "config/searchkernel-schema.json",
        "config/indexschema.cfg",
    ];
    relative
        .into_iter()
        .map(|name| {
            let path = root.join(name);
            Ok((
                path.clone(),
                fs::read(&path)
                    .map_err(|error| format!("read identity file {}: {error}", path.display()))?,
            ))
        })
        .collect()
}

fn assert_identity(snapshot: &Identity) -> AnyResult<()> {
    for (path, expected) in snapshot {
        let actual = fs::read(path)?;
        if &actual != expected {
            return Err(format!("durable identity changed: {}", path.display()).into());
        }
    }
    Ok(())
}

fn assert_root_shape(root: &Path) -> AnyResult<()> {
    for relative in [
        "manifest/index.json",
        "manifest/index-definition.bfbs",
        "data",
    ] {
        if !root.join(relative).exists() {
            return Err(format!("reopened index is missing {relative}").into());
        }
    }
    Ok(())
}

fn product_schema(runtime: &IndexRuntime) -> AnyResult<()> {
    let description = runtime.describe()?;
    if description.index_name() != Some("recovery") {
        return Err("product describe returned the wrong index name".into());
    }
    let fields: Vec<_> = description
        .fields()
        .iter()
        .map(|field| field.name())
        .collect();
    if fields != ["title", "qty"] {
        return Err(format!("product describe returned unexpected fields: {fields:?}").into());
    }
    Ok(())
}

fn product_state(runtime: &IndexRuntime) -> AnyResult<State> {
    product_schema(runtime)?;
    let index = runtime.index();
    let mut hits: Vec<String> = index
        .browse()
        .limit(100)
        .execute::<JsonValue>()?
        .hits
        .into_iter()
        .map(|hit| hit.id.to_string())
        .collect();
    hits.sort();
    let mut state = State::new();
    for id in hits {
        let document: JsonValue = index
            .get(&id)?
            .ok_or_else(|| format!("search returned a document that get cannot read: {id}"))?;
        let qty = document
            .get("qty")
            .and_then(JsonValue::as_i64)
            .ok_or_else(|| format!("document {id} has no integer qty"))?;
        state.insert(id, qty);
    }
    Ok(state)
}

fn product_create(root: &Path) -> AnyResult<Identity> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    if let Some(parent) = root.parent() {
        fs::create_dir_all(parent)?;
    }
    let runtime = IndexRuntime::create(root, product_definition())?;
    let index = runtime.index();
    for (id, qty) in seed_state() {
        index.insert(&id, &product_document(&id, qty))?;
    }
    product_schema(&runtime)?;
    runtime.close()?;
    assert_root_shape(root)?;
    identity(root)
}

fn product_open(root: &Path) -> AnyResult<IndexRuntime> {
    Ok(IndexRuntime::open(root)?)
}

fn product_insert(root: &Path, id: &str, qty: i64) -> AnyResult<()> {
    let runtime = product_open(root)?;
    runtime.index().insert(id, &product_document(id, qty))?;
    runtime.close()?;
    Ok(())
}

fn product_seed_delete_target(root: &Path, id: &str) -> AnyResult<()> {
    product_insert(root, id, 70)
}

fn product_crash(root: &Path, operation: Operation, phase: Phase, cycle: usize) -> AnyResult<()> {
    let runtime = product_open(root)?;
    if matches!(phase, Phase::Before) {
        abort_with_marker(root, operation, phase);
    }
    let index = runtime.index();
    match operation {
        Operation::Insert => {
            let id = insert_target(cycle);
            index.insert(&id, &product_document(&id, 40))?;
        }
        Operation::Update => {
            index.patch(BASE_2, searchkernel::Patch::new().set("qty", UPDATE_VALUE))?
        }
        Operation::Delete => {
            let id = if cycle == usize::MAX {
                BASE_3.to_owned()
            } else {
                format!("recovery-delete-{cycle}")
            };
            if !index.delete(id)? {
                return Err(
                    format!("product delete target was absent: recovery-delete-{cycle}").into(),
                );
            }
        }
        Operation::Close => runtime.close()?,
    }
    abort_with_marker(root, operation, phase);
}

fn abort_with_marker(root: &Path, operation: Operation, phase: Phase) -> ! {
    let marker = root.join(format!(
        ".crash-marker-{}-{}",
        operation.name(),
        phase.name()
    ));
    fs::write(marker, format!("{}:{}", operation.name(), phase.name()))
        .expect("write crash marker");
    std::process::abort()
}

fn product_verify(
    root: &Path,
    identity: &Identity,
    baseline: &State,
    operation: Operation,
    cycle: usize,
) -> AnyResult<State> {
    assert_root_shape(root)?;
    assert_identity(identity)?;
    let runtime = product_open(root)?;
    let state = product_state(&runtime)?;
    let expected = allowed_states(baseline, operation, cycle);
    if !expected.iter().any(|candidate| candidate == &state) {
        return Err(format!(
            "product recovered unexpected state for {}: {state:?}",
            operation.name()
        )
        .into());
    }
    runtime.close()?;
    Ok(state)
}

fn allowed_states(baseline: &State, operation: Operation, cycle: usize) -> Vec<State> {
    let mut states = vec![baseline.clone()];
    match operation {
        Operation::Insert => {
            let id = insert_target(cycle);
            let mut post = baseline.clone();
            post.insert(id, 40);
            states.push(post);
        }
        Operation::Update => {
            let mut post = baseline.clone();
            post.insert(BASE_2.to_owned(), UPDATE_VALUE);
            states.push(post);
        }
        Operation::Delete => {
            let id = if cycle == usize::MAX {
                BASE_3.to_owned()
            } else {
                format!("recovery-delete-{cycle}")
            };
            let mut post = baseline.clone();
            post.remove(&id);
            states.push(post);
        }
        Operation::Close => {}
    }
    states
}

fn marker_path(root: &Path, operation: Operation, phase: Phase) -> PathBuf {
    root.join(format!(
        ".crash-marker-{}-{}",
        operation.name(),
        phase.name()
    ))
}

fn run_child(
    operation: Operation,
    phase: Phase,
    root: &Path,
    cycle: usize,
) -> AnyResult<()> {
    product_crash(root, operation, phase, cycle)
}

fn spawn_crash(
    operation: Operation,
    phase: Phase,
    root: &Path,
    cycle: usize,
) -> AnyResult<()> {
    let executable = env::current_exe()?;
    let status = Command::new(executable)
        .arg("child")
        .arg(operation.name())
        .arg(phase.name())
        .arg(root)
        .arg(cycle.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.signal() != Some(6) {
        return Err(format!(
            "{} {} child did not abort with SIGABRT: {status:?}",
            operation.name(),
            phase.name()
        )
        .into());
    }
    let marker = marker_path(root, operation, phase);
    let actual = fs::read_to_string(&marker)?;
    let expected = format!("{}:{}", operation.name(), phase.name());
    if actual != expected {
        return Err(format!("crash marker was {actual:?}, expected {expected:?}").into());
    }
    fs::remove_file(marker)?;
    Ok(())
}

fn setup(root: &Path) -> AnyResult<Identity> {
    product_create(root)
}

fn verify(
    root: &Path,
    identity: &Identity,
    baseline: &State,
    operation: Operation,
    cycle: usize,
) -> AnyResult<State> {
    product_verify(root, identity, baseline, operation, cycle)
}

fn prepare_delete_target(root: &Path, id: &str) -> AnyResult<()> {
    product_seed_delete_target(root, id)
}

fn assert_missing_open(root: &Path) -> AnyResult<()> {
    if product_open(root).is_ok() {
        return Err("product open unexpectedly created a missing index".into());
    }
    Ok(())
}

fn run_matrix(root: &Path) -> AnyResult<()> {
    let operations = [
        Operation::Insert,
        Operation::Update,
        Operation::Delete,
        Operation::Close,
    ];
    let phases = [Phase::Before, Phase::After];
    for repetition in 0..MATRIX_REPETITIONS {
        for operation in operations {
            for phase in phases {
                let case_root = root.join(format!(
                    "matrix-{repetition}-{}-{}",
                    operation.name(),
                    phase.name()
                ));
                let identity = setup(&case_root)?;
                let baseline = seed_state();
                spawn_crash(operation, phase, &case_root, usize::MAX)?;
                verify(
                    &case_root,
                    &identity,
                    &baseline,
                    operation,
                    usize::MAX,
                )?;
            }
        }
    }
    Ok(())
}

fn run_repeated(root: &Path) -> AnyResult<()> {
    {
        let case_root = root.join("repeated-product");
        let identity = setup(&case_root)?;
        let mut model = seed_state();
        for cycle in 0..REPEATED_CYCLES {
            let operation = match cycle % 4 {
                0 => Operation::Insert,
                1 => Operation::Update,
                2 => Operation::Delete,
                _ => Operation::Close,
            };
            let phase = if (cycle / 4) % 2 == 0 {
                Phase::Before
            } else {
                Phase::After
            };
            if matches!(operation, Operation::Delete) {
                let id = format!("recovery-delete-{cycle}");
                prepare_delete_target(&case_root, &id)?;
                model.insert(id, 70);
            }
            spawn_crash(operation, phase, &case_root, cycle)?;
            model = verify(&case_root, &identity, &model, operation, cycle)?;
        }
    }
    Ok(())
}

fn parse_operation(value: &str) -> AnyResult<Operation> {
    match value {
        "insert" => Ok(Operation::Insert),
        "update" => Ok(Operation::Update),
        "delete" => Ok(Operation::Delete),
        "close" => Ok(Operation::Close),
        _ => Err(format!("unknown operation {value}").into()),
    }
}

fn parse_phase(value: &str) -> AnyResult<Phase> {
    match value {
        "before" => Ok(Phase::Before),
        "after" => Ok(Phase::After),
        _ => Err(format!("unknown phase {value}").into()),
    }
}

fn main() -> AnyResult<()> {
    let mut args = env::args_os();
    let _program = args.next();
    let mode = args.next().ok_or("missing mode")?;
    match mode.to_string_lossy().as_ref() {
        "child" => {
            let operation =
                parse_operation(&args.next().ok_or("missing operation")?.to_string_lossy())?;
            let phase = parse_phase(&args.next().ok_or("missing phase")?.to_string_lossy())?;
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            let cycle = args
                .next()
                .ok_or("missing cycle")?
                .to_string_lossy()
                .parse::<usize>()?;
            run_child(operation, phase, &root, cycle)
        }
        "matrix" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            run_matrix(&root)?;
            println!("recovery matrix boundaries=1 cases=8 repetitions={MATRIX_REPETITIONS}");
            Ok(())
        }
        "repeat" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            run_repeated(&root)?;
            println!("recovery repeated boundaries=1 cycles={REPEATED_CYCLES}");
            Ok(())
        }
        "missing" => {
            let root = PathBuf::from(args.next().ok_or("missing root")?);
            assert_missing_open(&root.join("product"))?;
            println!("recovery missing-open boundaries=1 created=false");
            Ok(())
        }
        other => Err(format!("unknown mode {other}").into()),
    }
}
