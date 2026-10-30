use crate::error::Result;
use crate::output::Output;
use std::path::Path;

pub fn run(dir: Option<&Path>, paths: Vec<String>) -> Result<Output> {
    let dir = super::start_dir(dir)?;
    Ok(Output::AttachmentEligibility(
        crate::attachments::local_query(&dir, &paths)?,
    ))
}
