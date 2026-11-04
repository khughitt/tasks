use super::audit::{self, DeclState};
use super::git_objects::Revision;
use super::validate::{Validation, run_declared};
use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::repo::Project;
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
pub struct Integrity {
    pub passed: bool,
    pub problems: Vec<String>,
}

#[derive(Serialize)]
pub struct Publication {
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reuse: Option<&'static str>,
}

impl Publication {
    pub fn local(id: &TaskId, name: &str, cause: &'static str) -> Self {
        let verify = format!("tasks export {id} {name} --at HEAD --verify");
        let next = match cause {
            "commit_unavailable" => format!(
                "put the records directory in a Git work tree, then tasks commit and {verify}"
            ),
            "validation_failed" | "validation_unavailable" => {
                format!("fix the validator or content, then {verify}")
            }
            _ => format!("resolve {cause}, then tasks commit and {verify}"),
        };
        Self {
            state: "local",
            commit: None,
            cause: Some(cause),
            next: Some(next),
            reuse: None,
        }
    }
    pub fn published(commit: String) -> Self {
        Self {
            state: "published",
            commit: Some(commit),
            cause: None,
            next: None,
            reuse: None,
        }
    }
}

pub fn report(
    project: &Project,
    id: &TaskId,
    name: &str,
    commit: String,
    warnings: &mut Vec<String>,
) -> (Publication, Option<Validation>) {
    match verify_published(project, id, name, &commit) {
        Ok((integrity, validation)) => {
            let publication = if !integrity.passed {
                Publication::local(id, name, "verify_failed")
            } else if !validation.passed {
                Publication::local(id, name, validation_cause(&validation))
            } else {
                Publication::published(commit)
            };
            (publication, Some(validation))
        }
        Err(error) => {
            let cause = if matches!(error, Error::MultipartInvalidContent(_)) {
                "verify_failed"
            } else {
                "validation_unavailable"
            };
            warnings.push(format!("publication verification failed: {error}"));
            (Publication::local(id, name, cause), None)
        }
    }
}

pub fn validate(project: &Project, name: &str, bytes: &[u8]) -> Result<Validation> {
    let config = std::fs::read_to_string(project.records.join(crate::repo::CONFIG_REL))?;
    let current = Project::from_config(project.records.clone(), &config)?;
    let validators = current.multipart.validators.as_deref().ok_or_else(|| {
        Error::MultipartValidatorsUndeclared(
            "declare [attachments] multipart_validators (use [] for no extra whole-document gates)"
                .into(),
        )
    })?;
    let toplevel =
        crate::record_commit::working_tree(&project.records).map_err(Error::Validation)?;
    // Local artifacts outside Git still validate before admission.
    let cwd = toplevel.as_deref().unwrap_or(&project.records);
    Ok(run_declared(
        validators,
        config.as_bytes(),
        cwd,
        name,
        bytes,
        Duration::from_secs(120),
    ))
}

pub fn validation_cause(validation: &Validation) -> &'static str {
    if validation
        .validators
        .iter()
        .any(|run| run.outcome == "unavailable")
        || validation.validators.is_empty()
    {
        "validation_unavailable"
    } else {
        "validation_failed"
    }
}

pub fn verify_published(
    project: &Project,
    id: &TaskId,
    name: &str,
    commit: &str,
) -> Result<(Integrity, Validation)> {
    let bytes = rebuild_published(project, id, name, commit)
        .map_err(|error| Error::MultipartInvalidContent(error.to_string()))?;
    // Git readers and scoped fetch workers have joined before validator ownership begins.
    let validation = validate(project, name, &bytes)?;
    Ok((
        Integrity {
            passed: true,
            problems: Vec::new(),
        },
        validation,
    ))
}

fn rebuild_published(project: &Project, id: &TaskId, name: &str, commit: &str) -> Result<Vec<u8>> {
    let revision = Revision::resolve(&project.records, commit)?;
    let task = revision
        .task_at(project, id)?
        .ok_or_else(|| Error::Validation(format!("missing task {id} at {commit}")))?;
    if !matches!(
        audit::declarations(&task).get(name),
        Some(DeclState::Live(_))
    ) {
        return Err(Error::Validation(format!(
            "no live multipart attachment {name} at {commit}"
        )));
    }
    audit::rebuild(project, &task, name, Some(&revision))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multipart::catalog::{Catalog, CatalogPart};
    use crate::multipart::content::sha256_hex;
    use crate::multipart::declaration::Declaration;
    use std::path::Path;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(root)
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap())
            .env("HOME", root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().into()
    }

    #[test]
    fn verify_published_rejects_corrupt_committed_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        assert_eq!(
            Path::new(&git(&root, &["rev-parse", "--show-toplevel"])),
            root
        );
        assert!(Path::new(&git(&root, &["rev-parse", "--absolute-git-dir"])).starts_with(&root));
        let project = Project::from_config(
            root.clone(),
            "prefix = 'tst'\n[attachments]\nmultipart_validators=[]",
        )
        .unwrap();
        let id = TaskId::parse("tst-000001").unwrap();
        let member_dir = crate::attachments::task_dir(&project, &id);
        std::fs::create_dir_all(&member_dir).unwrap();
        let catalog = Catalog {
            name: "a.txt".into(),
            bytes: 5,
            sha256: sha256_hex(b"hello"),
            parts: vec![CatalogPart {
                name: "a.txt.part-0001".into(),
                bytes: 5,
                sha256: sha256_hex(b"hello"),
            }],
        }
        .to_bytes();
        let declaration = Declaration {
            name: "a.txt".into(),
            bytes: 5,
            parts: 1,
            sha256: sha256_hex(b"hello"),
            catalog_sha256: sha256_hex(&catalog),
            caption: None,
        };
        std::fs::write(member_dir.join("a.txt.part-0001"), "wrong").unwrap();
        std::fs::write(member_dir.join("a.txt.catalog.json"), &catalog).unwrap();
        let record = format!(
            "---\nid: {id}\ntitle: Evidence\nstatus: todo\npriority: 2\ncreated: 2026-11-03T00:00:00Z\nupdated: 2026-11-03T00:00:00Z\ndepends: []\ntags: []\n---\n\n## Notes\n\n- 2026-11-03T00:00:00Z (fixture): attached: a.txt.part-0001 (5 bytes)\n- 2026-11-03T00:00:00Z (fixture): attached: a.txt.catalog.json ({} bytes)\n- 2026-11-03T00:00:00Z (fixture): {}\n",
            catalog.len(),
            declaration.note()
        );
        std::fs::write(root.join(format!("tasks/{id}.md")), record).unwrap();
        git(&root, &["add", "tasks"]);
        git(
            &root,
            &["-c", "maintenance.auto=false", "commit", "-qm", "fixture"],
        );
        // Repairing the working file cannot make this corrupt committed artifact valid.
        std::fs::write(member_dir.join("a.txt.part-0001"), "hello").unwrap();
        let error = match verify_published(&project, &id, "a.txt", "HEAD") {
            Ok(_) => panic!("corrupt committed artifact accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), "multipart_invalid_content");
        assert!(error.to_string().contains("sha256"), "{error}");
        let mut warnings = Vec::new();
        let (publication, validation) =
            report(&project, &id, "a.txt", "HEAD".into(), &mut warnings);
        assert_eq!(publication.cause, Some("verify_failed"));
        assert!(validation.is_none());
        assert!(warnings[0].contains("sha256"));
    }
}
