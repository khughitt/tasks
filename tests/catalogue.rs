mod common;
use common::{TestEnv, git};

#[test]
fn check_reports_a_malformed_identity_table_as_a_config_error() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\n[identity]\npurpose = 'A catalogue'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["check"]), "config");
}

#[test]
fn profile_must_follow_the_tag_grammar() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\nprofile = 'Personal Work'\n",
    )
    .unwrap();
    assert_eq!(env.fail(&dir, &["check"]), "config");
}

#[test]
fn shorthand_entries_are_nonempty_single_lines() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    for entry in ["", "Two\nlines"] {
        let entry = toml::Value::String(entry.into());
        std::fs::write(
            dir.join("tasks/.config.toml"),
            format!("prefix = 'cat'\nshorthand = [{entry}]\n"),
        )
        .unwrap();
        assert_eq!(env.fail(&dir, &["check"]), "config");
    }
}

#[test]
fn rename_carries_profile_shorthand_and_identity() {
    let mut env = TestEnv::new();
    let dir = env.init("cat");
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(
        dir.join("tasks/.config.toml"),
        "prefix = 'cat'\nprofile = 'personal'\nshorthand = ['x']\n\n[identity]\nname = 'Catalogue'\naliases = ['Public catalogue']\npurpose = 'Keeps project identities'\nsource = 'seeded'\nrefreshed = '2026-11-09T16:00:00Z'\n",
    )
    .unwrap();
    let before: toml::Value = toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.json(&dir, &["rename", "cat", "cats"]);
    let after: toml::Value = toml::from_str(&env.read(&dir, "tasks/.config.toml")).unwrap();
    assert_eq!(after["prefix"].as_str(), Some("cats"));
    for key in ["profile", "shorthand", "identity"] {
        assert_eq!(after[key], before[key], "rename changed {key}");
    }
}
