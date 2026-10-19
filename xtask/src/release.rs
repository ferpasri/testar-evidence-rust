use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Release {
    pub id: u64,
    pub tag_name: String,
    pub published_at: Option<String>,
    pub draft: bool,
    pub prerelease: bool,
    pub zipball_url: String,
}

pub(crate) fn newest(releases: Vec<Release>) -> Result<Release> {
    releases
        .into_iter()
        .filter(|release| !release.draft && release.published_at.is_some())
        .max_by(|left, right| {
            (&left.published_at, left.id).cmp(&(&right.published_at, right.id))
        })
        .ok_or_else(|| "No published specs release was found".into())
}
