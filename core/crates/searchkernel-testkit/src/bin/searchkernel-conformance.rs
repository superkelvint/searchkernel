fn main() {
    let root = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("reference/api-conformance"));
    match searchkernel_testkit::ConformanceRunner::new(root).run() {
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
