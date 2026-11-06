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
    pub fn check_hook(&self) {
        use std::os::unix::fs::PermissionsExt;
        let output = git_fixture::command(self.env.home.path(), &self.records)
            .args(["rev-parse", "--absolute-git-dir"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let dir = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        assert!(
            dir.canonicalize()
                .unwrap()
                .starts_with(self._root.path().canonicalize().unwrap())
        );
        let binary = PathBuf::from(self.cmd().get_program());
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let hook = dir.join("hooks/pre-commit");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nexec {} -C {} check >/dev/null\n",
                quote(binary.to_str().unwrap()),
                quote(self.dir.to_str().unwrap())
            ),
        )
        .unwrap();
        std::fs::set_permissions(hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    pub fn request(&self, name: &str, v: &Value) -> PathBuf {
        let p = self.dir.join(name);
        std::fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
        p
    }
}

pub fn checkpoint_fixture() -> RetainedEnv {
    let f = RetainedEnv::new("in-tree");
    f.json(&["start", &f.id]);
    f
}

pub fn host() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .unwrap()
        .trim()
        .into()
}

pub fn checkpoint_request(
    f: &RetainedEnv,
    n: u64,
    declarations: serde_json::Value,
) -> serde_json::Value {
    json!({"version":1,"operation_id":format!("{n:032x}"),"host":host(),"inputs":[{"task":f.id,"basis":f.state()["basis"]}],"declarations":declarations})
}

pub fn declaration(path: &std::path::Path, kind: &str) -> serde_json::Value {
    json!({"kind":kind,"path":path.to_str().unwrap(),"purpose":"Review","cleanup_trigger":"Review completes"})
}

pub fn checkpoint(f: &RetainedEnv, request: &serde_json::Value) -> serde_json::Value {
    let path = f.request("request.json", request);
    f.json(&[
        "retained",
        "checkpoint",
        &f.id,
        "--request",
        path.to_str().unwrap(),
    ])
}

pub fn transfer_fixture() -> (RetainedEnv, String, std::path::PathBuf) {
    let f = checkpoint_fixture();
    let random = f.json(&["add", "Receiver"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let to = "sci-000002".to_string();
    let path = f.records.join(format!("tasks/{random}.md"));
    let bytes = std::fs::read_to_string(&path)
        .unwrap()
        .replace(&random, &to);
    std::fs::remove_file(path).unwrap();
    std::fs::write(f.records.join(format!("tasks/{to}.md")), bytes).unwrap();
    git(&f.records, &["add", "tasks"]);
    git(&f.records, &["commit", "-qm", "fixed receiver identity"]);
    f.json(&["start", &to]);
    let file = f.dir.join("payload");
    std::fs::write(&file, "payload").unwrap();
    checkpoint(
        &f,
        &checkpoint_request(&f, 1, json!([declaration(&file, "evidence")])),
    );
    (f, to, file)
}

pub fn transfer_request(f: &RetainedEnv, to: &str) -> serde_json::Value {
    json!({"version":1,"operation_id":"a".repeat(32),"host":host(),"inputs":[{"task":f.id,"basis":f.state()["basis"]},{"task":to,"basis":f.json(&["retained","state",to])["basis"]}],"items":[f.state()["items"][0]["identity"]],"purpose":"Continue review","cleanup_trigger":"Review and disposition complete"})
}

pub fn transfer(f: &RetainedEnv, to: &str, request: &serde_json::Value) -> serde_json::Value {
    let path = f.request("transfer.json", request);
    f.json(&[
        "retained",
        "transfer",
        &f.id,
        to,
        "--request",
        path.to_str().unwrap(),
    ])
}

pub fn cross_receiver(f: &RetainedEnv, shared: bool) -> (std::path::PathBuf, String) {
    let root = f.dir.parent().unwrap().join("other");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-qb", "main"]);
    git_fixture::assert_repo(f.env.home.path(), &root);
    let mut args = vec!["-C", root.to_str().unwrap(), "init", "--prefix", "fam"];
    if shared {
        args.extend(["--records", f.records.to_str().unwrap()]);
    }
    f.json(&args);
    let records = if shared {
        f.records.join("fam")
    } else {
        root.clone()
    };
    let path = records.join("tasks/.config.toml");
    let mut config: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    config.as_table_mut().unwrap().insert(
        "git".into(),
        toml::Value::Table(toml::map::Map::from_iter([(
            "commit".into(),
            toml::Value::Boolean(true),
        )])),
    );
    std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
    git(&records, &["add", "."]);
    git(&records, &["commit", "-qm", "receiver configuration"]);
    let to = f.json(&["add", "Foreign receiver", "--project", "fam"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    f.json(&["start", &to]);
    (root, to)
}
