fn main() {
    let mut args = std::env::args_os().skip(1);
    let update = args
        .next()
        .as_deref()
        .is_some_and(|value| value == "--update-goldens");
    let root = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("reference/api-conformance"));
    let runner = searchkernel_testkit::ConformanceRunner::new(root);
    let result = if update {
        runner.regenerate_goldens()
    } else {
        runner.run()
    };
    match result {
        Ok(report) => {
            println!("PASS: {} API→IR conformance cases", report.passed);
            for (category, count) in report.categories {
                println!("  {category}: {count}");
            }
        }
        Err(error) => {
            eprintln!("conformance failure: {error}");
            std::process::exit(1);
        }
    }
}
