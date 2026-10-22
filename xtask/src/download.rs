use std::path::Path;
use std::time::Duration;

use crate::cache::install;
use crate::release::newest;
use crate::{PreparedSpecs, Release, Result};

const RELEASES: &str =
    "https://api.github.com/repos/ferpasri/testar-evidence-specs/releases";
const MAX_DOWNLOAD: u64 = 64 * 1024 * 1024;

fn download(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>> {
    if !url.starts_with("https://") {
        return Err("Release downloads require HTTPS".into());
    }
    let bytes = agent
        .get(url)
        .header("User-Agent", "testar-evidence-conformance")
        .header("Accept", "application/vnd.github+json")
        .call()?
        .body_mut()
        .with_config()
        .limit(MAX_DOWNLOAD)
        .read_to_vec()?;
    Ok(bytes)
}

pub fn prepare_latest(repository: &Path) -> Result<PreparedSpecs> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent();
    let mut releases = Vec::new();
    let mut page = 1;
    loop {
        let url = format!("{RELEASES}?per_page=100&page={page}");
        let content = download(&agent, &url)
            .map_err(|error| format!("Cannot query specs releases: {error}"))?;
        let batch: Vec<Release> = serde_json::from_slice(&content)?;
        let complete = batch.len() < 100;
        releases.extend(batch);
        if complete {
            break;
        }
        page += 1;
    }
    let release = newest(releases)?;
    println!(
        "Selected specs {} ({}, published {}).",
        release.tag_name,
        if release.prerelease {
            "prerelease"
        } else {
            "release"
        },
        release.published_at.as_deref().unwrap()
    );
    let content = download(&agent, &release.zipball_url).map_err(|error| {
        format!("Cannot download selected specs release: {error}")
    })?;
    install(repository, release, &content)
}
