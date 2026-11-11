mod common;

use common::TestEnv;

#[test]
fn check_warns_without_assigning_a_project_color() {
    let mut env = TestEnv::new();
    let dir = env.init("plain");
    let config = dir.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(!out.stdout.is_empty(), "missing colour warning");
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let warning = value["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "color_missing")
        .expect("missing colour warning");
    assert_eq!(warning["file"], "tasks/.config.toml");
    assert!(warning["detail"].as_str().unwrap().contains("plain"));
    assert_eq!(std::fs::read(&config).unwrap(), before);
    std::fs::write(&config, "prefix = \"plain\"\ncolor = \"#2f9fb1\"\n").unwrap();
    let out = env.cmd(&dir).arg("check").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
}

#[test]
fn terminal_indices_preserve_existing_consumer_colors() {
    let mut env = TestEnv::new();
    let dir = env.init("colored");
    // Pinned consumer examples from hq's stand-in tests, independently of this code.
    for (hex, index) in [
        ("#2f9fb1", 67),
        ("#d72633", 160),
        ("#776558", 242),
        ("#2f63e1", 27),
        ("#fd538f", 198),
        ("#317d01", 28),
    ] {
        std::fs::write(
            dir.join("tasks/.config.toml"),
            format!("prefix = \"colored\"\ncolor = \"{hex}\"\n"),
        )
        .unwrap();
        let resolved = env.json(
            &dir,
            &[
                "resolve",
                "colored",
                "colored-abcdef",
                dir.to_str().unwrap(),
            ],
        );
        for row in resolved["results"].as_array().unwrap() {
            assert_eq!(row["color_256"], index, "{hex}: {row}");
            assert_eq!(row["entry"]["color_256"], index, "{hex}: {row}");
        }
        let projects = env.json(&dir, &["projects"]);
        assert_eq!(projects["projects"][0]["color_256"], index, "{hex}");
    }
}

#[test]
fn resolver_color_is_config_only_and_absent_when_unset() {
    let mut env = TestEnv::new();
    let dir = env.init("plain");
    let projects = env.json(&dir, &["projects"]);
    assert!(projects["projects"][0].get("color_256").is_none());
    let config = dir.join("tasks/.config.toml");
    let before = std::fs::read(&config).unwrap();
    std::fs::write(dir.join("tasks/plain-abcdef.md"), "malformed task").unwrap();
    let resolved = env.json(&dir, &["resolve", "--all"]);
    let row = &resolved["results"][0];
    assert!(row.get("color_256").is_none());
    assert!(row["entry"].get("color_256").is_none());
    assert_eq!(std::fs::read(&config).unwrap(), before);
    std::fs::write(&config, "prefix = \"plain\"\ncolor = \"#2f9fb1\"\n").unwrap();
    let before = std::fs::read(&config).unwrap();
    let resolved = env.json(&dir, &["resolve", "--all"]);
    assert_eq!(resolved["results"][0]["color_256"], 67);
    assert_eq!(std::fs::read(&config).unwrap(), before);
}
