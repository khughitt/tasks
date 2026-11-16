mod common;
use common::TestEnv;
use std::process::Command;

fn resolve_relay_on_test_path(env: &TestEnv) -> String {
    let out = Command::new("sh")
        .env_clear()
        .envs(common::isolated_env(env.home.path()))
        .args(["-c", "command -v relay || echo none"])
        .output()
        .unwrap();
    assert!(out.status.success(), "sh failed: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn host_relay_is_not_on_test_path() {
    let env = TestEnv::new();
    assert_eq!(resolve_relay_on_test_path(&env), "none");

    let script = env.fake_relay("#!/bin/sh\necho fake\n");
    assert_eq!(script, env.fake_bin().join("relay"));
    assert_eq!(
        resolve_relay_on_test_path(&env),
        script.display().to_string()
    );
}
