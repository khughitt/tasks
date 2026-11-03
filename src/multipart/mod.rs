pub mod audit;
pub mod catalog;
pub mod content;
#[allow(dead_code)] // Wired into attach and recovery by the following plan tasks.
pub mod cut;
pub mod declaration;
pub mod names;

pub const RECOVERY_MAX_BYTES: u64 = 268_435_456;
pub const RECOVERY_MAX_PARTS: usize = 256;

#[derive(Debug, Clone)]
#[allow(dead_code)] // Consumed by the multipart admission path in the next steps.
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
