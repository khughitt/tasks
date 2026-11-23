//! The words that name a project, prefix, snapshot name and aliases, and shorthand, and
//! the words two catalogued entries both claim (docs/configuration.md).
use crate::registry::Registry;
use crate::repo::Project;
use std::collections::BTreeMap;

/// One entry's claim on a word, and which of its fields makes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub prefix: String,
    pub field: &'static str,
}

/// A word, compared case-insensitively, that more than one entry claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    pub word: String,
    pub claims: Vec<Claim>,
}

impl Collision {
    pub fn involves(&self, prefix: &str) -> bool {
        self.claims.iter().any(|claim| claim.prefix == prefix)
    }

    pub fn detail(&self) -> String {
        let claims = self
            .claims
            .iter()
            .map(|claim| format!("{} ({})", claim.prefix, claim.field))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{:?} is claimed by {claims}; a word names one project, so rename or drop all but one",
            self.word
        )
    }
}

fn words(project: &Project) -> Vec<(String, &'static str)> {
    let mut words = vec![(project.prefix.clone(), "prefix")];
    if let Some(identity) = &project.identity {
        words.push((identity.name.clone(), "name"));
        words.extend(
            identity
                .aliases
                .iter()
                .map(|alias| (alias.clone(), "alias")),
        );
    }
    words.extend(
        project
            .shorthand
            .iter()
            .map(|word| (word.clone(), "shorthand")),
    );
    words
}

/// Words claimed by more than one of `entries`. A retired entry's words forward to the
/// successor its `[retired]` names, so that pair never collides and the successor owns
/// the word; any other claimant still collides.
pub fn collisions<'a>(
    registry: &Registry,
    entries: impl IntoIterator<Item = &'a Project>,
) -> Vec<Collision> {
    let mut claims: BTreeMap<String, Vec<Claim>> = BTreeMap::new();
    let mut successors: BTreeMap<String, String> = BTreeMap::new();
    for project in entries {
        if let Some(successor) = project.retired.as_ref().and_then(|r| r.successor.as_ref()) {
            successors.insert(
                project.prefix.clone(),
                registry.canonical_prefix(successor).to_string(),
            );
        }
        for (word, field) in words(project) {
            let key = word.trim().to_lowercase();
            let list = claims.entry(key).or_default();
            if !list.iter().any(|claim| claim.prefix == project.prefix) {
                list.push(Claim {
                    prefix: project.prefix.clone(),
                    field,
                });
            }
        }
    }
    claims
        .into_iter()
        .filter(|(_, list)| {
            let owners = list
                .iter()
                .filter(|claim| {
                    !successors
                        .get(&claim.prefix)
                        .is_some_and(|successor| list.iter().any(|c| &c.prefix == successor))
                })
                .count();
            owners > 1
        })
        .map(|(word, claims)| Collision { word, claims })
        .collect()
}
