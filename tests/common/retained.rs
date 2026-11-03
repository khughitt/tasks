use crate::common::{TestEnv, git, git_fixture};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct RetainedEnv {
    pub env: TestEnv,
    pub dir: PathBuf,
    pub records: PathBuf,
    pub id: String,
    _root: TempDir,
}
fn repository(home: &Path, dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git_fixture::assert_repo(home, dir);
    std::fs::write(dir.join("README.md"), "Fixture\n").unwrap();
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "-qm", "seed"]);
}
impl RetainedEnv {
    pub fn new(layout: &str) -> Self {
        let env = TestEnv::new();
        let root = tempfile::tempdir().unwrap();
        let code = root.path().join("code");
        repository(env.home.path(), &code);
        let records = match layout {
            "in-tree" => {
                env.json(&code, &["init", "--prefix", "sci"]);
                code.clone()
            }
            "detached" => {
                let r = root.path().join("records");
                repository(env.home.path(), &r);
                env.json(
                    &code,
                    &["init", "--prefix", "sci", "--records", r.to_str().unwrap()],
                );
                r.join("sci")
            }
            "linked" => {
                let r = root.path().join("overlay");
                repository(env.home.path(), &r);
                env.json(
                    &r,
                    &[
                        "init",
                        "--prefix",
                        "sci",
                        "--checkout",
                        code.to_str().unwrap(),
                    ],
                );
                r
            }
            _ => panic!("layout"),
        };
        let config = records.join("tasks/.config.toml");
        let mut value: toml::Value =
            toml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        value.as_table_mut().unwrap().insert(
            "git".into(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "commit".into(),
                toml::Value::Boolean(true),
            )])),
        );
        std::fs::write(config, toml::to_string(&value).unwrap()).unwrap();
        git(&records, &["add", "."]);
        git(&records, &["commit", "-qm", "configuration"]);
        let dir = code;
        let mut f = Self {
            env,
            dir,
            records,
            id: String::new(),
            _root: root,
        };
        f.id = f.json(&["add", "Retained fixture"])["id"]
            .as_str()
            .unwrap()
            .into();
        f
    }
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut c = self.env.cmd(&self.dir);
        c.env("TASKS_SESSION", "retained-fixture");
        c
    }
    pub fn json(&self, args: &[&str]) -> Value {
        let out = self.cmd().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
    pub fn seed_note(&self, text: &str) {
        let path = self.records.join(format!("tasks/{}.md", self.id));
        let mut raw = std::fs::read_to_string(&path).unwrap();
        if !raw.contains("\n## Notes\n") {
            raw.push_str("\n## Notes\n\n");
        }
        raw.push_str(&format!("- 2026-11-03T00:00:00Z (fixture): {text}\n"));
        std::fs::write(&path, raw).unwrap();
        git(&self.records, &["add", path.to_str().unwrap()]);
        git(&self.records, &["commit", "-qm", "seed retained history"]);
    }
    pub fn note(&self, record: Value) {
        self.seed_note(&format!("retained: {record}"));
    }
    pub fn state(&self) -> Value {
        self.json(&["retained", "state", &self.id])
    }
    pub fn basis(&self) -> String {
        let out = git_fixture::command(self.env.home.path(), &self.records)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    pub fn record(&self) -> Value {
        json!({"version":1,"host":"fixture-host","kind":"evidence","path":"/data/report","state":"kept","verification":"verified","purpose":"Review","next_owner":format!("task:{}",self.id),"cleanup_trigger":"After review","sha256":"a".repeat(64)})
    }
    pub fn request(&self, name: &str, v: &Value) -> PathBuf {
        let p = self.dir.join(name);
        std::fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
        p
    }
}
