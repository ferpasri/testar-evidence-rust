use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn specs_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
        let prepared = xtask::load_prepared(repository).unwrap_or_else(|error| {
            panic!(
                "Conformance data unavailable or changed: {error}. Run cargo run -p xtask -- prepare-conformance first."
            )
        });
        eprintln!("Testing prepared specs {}.", prepared.release.tag_name);
        prepared.root
    })
}
