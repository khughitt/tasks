mod common;
use common::TestEnv;
use std::path::Path;

fn configure(dir: &Path, attachments: &str) {
    let config = dir.join("tasks/.config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap();
    text.push_str(&format!("\n[attachments]\n{attachments}\n"));
    std::fs::write(config, text).unwrap();
}

#[test]
fn config_rejects_limits_above_the_recovery_ceiling() {
    for (key, value, ceiling) in [
        ("multipart_max_bytes", "268435457", "268435456"),
        ("multipart_max_parts", "257", "256"),
    ] {
        let mut env = TestEnv::new();
        let dir = env.init("tst");
        configure(&dir, &format!("{key} = {value}"));
        let output = env.cmd(&dir).arg("list").output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "config");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(detail.contains(key), "{detail}");
        assert!(detail.contains(ceiling), "{detail}");
    }
}

#[test]
fn config_accepts_zero_bytes_and_an_empty_validator_list() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    configure(&dir, "multipart_max_bytes = 0\n multipart_validators = []");
    env.json(&dir, &["list"]);
}

#[test]
fn config_rejects_a_validator_without_command() {
    let mut env = TestEnv::new();
    let dir = env.init("tst");
    configure(&dir, "multipart_validators = [{ match = \"*.md\" }]");
    let output = env.cmd(&dir).arg("list").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "config");
    assert!(
        error["error"]["detail"]
            .as_str()
            .unwrap()
            .contains("command")
    );
}
