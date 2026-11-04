pub mod audit;
pub mod catalog;
pub mod content;
pub mod cut;
pub mod declaration;
pub mod git_objects;
pub mod names;

pub const RECOVERY_MAX_BYTES: u64 = 268_435_456;
pub const RECOVERY_MAX_PARTS: usize = 256;

#[derive(Debug, Clone)]
pub struct MultipartConfig {
    pub max_bytes: u64,
    pub max_parts: usize,
    pub validators: Option<Vec<ValidatorSpec>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorSpec {
    #[serde(rename = "match")]
    pub glob: String,
    pub command: Vec<String>,
}

pub mod publication;
pub mod validate;
