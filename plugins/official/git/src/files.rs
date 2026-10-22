//! Wire-compatible metadata for Git history; independent of the Fiex host crate.
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub color: u8,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u64,
    pub created: u64,
    pub extension: String,
    pub mime: String,
    pub readonly: bool,
    pub link_target: Option<String>,
    pub tags: Vec<Tag>,
    pub sync: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub read_only: bool,
    pub virtual_volume: bool,
    pub path: String,
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
