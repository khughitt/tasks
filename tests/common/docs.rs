use super::TestEnv;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct DocFixture {
    config: PathBuf,
    response: PathBuf,
    calls: PathBuf,
    pub script: PathBuf,
}
impl DocFixture {
    pub fn new(env: &TestEnv, body: &str) -> Self {
        let root = env.home.path().join("doc-fixture");
        fs::create_dir_all(&root).unwrap();
        let fixture = Self {
            config: env.home.path().join(".config/tasks/config.toml"),
            response: root.join("body.json"),
            calls: root.join("calls"),
            script: root.join("fetch.sh"),
        };
        let quote = |path: &Path| format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"));
        let calls = quote(&fixture.calls);
        let response = quote(&fixture.response);
        // shortcut: fixture IDs are simple names; use a preencoded reply for arbitrary-ID cases.
        fs::write(
            &fixture.script,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$1" >> {calls}
case "$1" in
    alias) id=canonical ;;
    *) id="$1" ;;
esac
printf '{{"id":"%s","body":%s}}' "$id" "$(cat {response})"
"#
            ),
        )
        .unwrap();
        let quoted = quote(&fixture.script);
        let config = serde_json::json!({"docs":{"schemes":{"stub":format!("sh {quoted}")}}});
        fs::create_dir_all(fixture.config.parent().unwrap()).unwrap();
        fs::write(&fixture.config, toml::to_string(&config).unwrap()).unwrap();
        fixture.set_body(body);
        fixture
    }
    pub fn set_body(&self, body: &str) {
        fs::write(&self.response, serde_json::to_vec(body).unwrap()).unwrap();
    }
    pub fn calls(&self) -> Vec<String> {
        match fs::read_to_string(&self.calls) {
            Ok(text) => text.lines().map(str::to_owned).collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(error) => panic!("fixture log: {error}"),
        }
    }
    pub fn disconnect(&self) {
        fs::remove_file(&self.config).unwrap();
    }
}
