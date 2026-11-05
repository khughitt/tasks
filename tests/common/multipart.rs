use super::TestEnv;
use std::path::Path;

pub fn seed(env: &TestEnv, root: &Path, id: &str, name: &str) -> Vec<u8> {
    let config = root.join("tasks/.config.toml");
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(
        config,
        format!("{text}\n[attachments]\nmax_bytes = 65536\nmultipart_validators = []\n"),
    )
    .unwrap();
    let bytes = "exact evidence é\n".repeat(9000).into_bytes();
    let source = env.home.path().join(name);
    std::fs::write(&source, &bytes).unwrap();
    let attached = env.json(
        root,
        &["attach", id, source.to_str().unwrap(), "--multipart"],
    );
    assert!(attached["multipart"]["parts"].as_u64().unwrap() > 1);
    bytes
}

pub fn assert_intact(env: &TestEnv, root: &Path, id: &str, name: &str, bytes: &[u8]) {
    assert_eq!(
        env.check(root),
        serde_json::json!({"errors": [], "warnings": []})
    );
    let out = env
        .cmd(root)
        .args(["export", id, name, "-o", "-"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert_eq!(out.stdout, bytes);
}
