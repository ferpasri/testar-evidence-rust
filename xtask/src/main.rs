use std::path::Path;

fn main() {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["prepare-conformance"] {
        eprintln!("Usage: cargo run -p xtask -- prepare-conformance");
        std::process::exit(2);
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be inside the repository");
    match xtask::prepare_latest(repository) {
        Ok(prepared) => {
            println!(
                "Prepared specs {} at {}. Tests can now run offline.",
                prepared.release.tag_name,
                prepared.root.display()
            );
        }
        Err(error) => {
            eprintln!("Cannot prepare conformance data: {error}");
            std::process::exit(1);
        }
    }
}
