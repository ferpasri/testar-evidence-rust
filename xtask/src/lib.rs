mod archive;
mod cache;
mod download;
mod release;

pub use cache::{PreparedSpecs, case_directories, load_prepared};
pub use download::prepare_latest;
pub use release::Release;

pub type Result<T> =
    std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(test)]
mod tests;
