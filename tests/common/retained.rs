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
        let environment = super::isolated_env(self.env.home.path())
            .into_iter()
            .map(|(key, value)| format!("{key}={}", quote(value.to_str().unwrap())))
            .collect::<Vec<_>>()
            .join(" ");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nexec env -i {environment} {} -C {} check >/dev/null\n",
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
    let f = RetainedEnv::new("detached");
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
    cross_receiver_layout(f, shared, false)
}
pub fn cross_receiver_layout(
    f: &RetainedEnv,
    shared: bool,
    in_tree: bool,
) -> (std::path::PathBuf, String) {
    let root = f.dir.parent().unwrap().join("other");
    std::fs::create_dir(&root).unwrap();
    git(&root, &["init", "-qb", "main"]);
    git_fixture::assert_repo(f.env.home.path(), &root);
    let mut args = vec!["-C", root.to_str().unwrap(), "init", "--prefix", "fam"];
    let checkout = root.with_file_name("other-code");
    if shared {
        args.extend(["--records", f.records.to_str().unwrap()]);
    } else if !in_tree {
        repository(f.env.home.path(), &checkout);
        args.extend(["--checkout", checkout.to_str().unwrap()]);
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

#[derive(Clone, Copy)]
pub enum HistoricalScenario {
    CheckpointPending,
    CheckpointCompleted,
    TransferReceiverPublished,
    TransferSenderReleased,
    Reconciled,
}
pub struct HistoricalOperation {
    pub request: Value,
    pub receiver: Option<String>,
    pub reconciliation: Option<Value>,
}

fn historical_batch(
    f: &RetainedEnv,
    id: &str,
    at: &str,
    artifacts: &[(&str, &[u8])],
    notes: &[String],
) {
    let path = f.records.join(format!("tasks/{id}.md"));
    let mut raw = std::fs::read_to_string(&path).unwrap();
    if !raw.contains("\n## Notes\n") {
        raw.push_str("\n## Notes\n\n");
    }
    for (name, bytes) in artifacts {
        let dir = f.records.join(format!("tasks/files/{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), bytes).unwrap();
        raw.push_str(&format!(
            "- {at} (fixture): attached: {name} ({} bytes)\n",
            bytes.len()
        ));
    }
    for note in notes {
        raw.push_str(&format!("- {at} (fixture): {note}\n"));
    }
    let updated = raw
        .lines()
        .find(|l| l.starts_with("updated: "))
        .unwrap()
        .to_string();
    raw = raw.replacen(&updated, &format!("updated: {at}"), 1);
    std::fs::write(&path, raw).unwrap();
    git(&f.records, &["add", "tasks"]);
    git(
        &f.records,
        &["commit", "-qm", "seed historical retained publication"],
    );
}
fn historical_phase(
    request: &Value,
    locator: &Value,
    phase: &str,
    at: &str,
    outputs: Value,
) -> String {
    format!(
        "retention operation: {}",
        json!({"version":1,"key":{"coordinator":request["inputs"][0]["task"],"operation_id":request["operation_id"]},"phase":phase,"request":locator,"input_bases":request["inputs"],"host":request["host"],"at":at,"observer":null,"outputs":outputs,"diagnostics":[]})
    )
}
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
fn historical_basis(f: &RetainedEnv, id: &str) -> Value {
    f.json(&["retained", "state", id])["basis"].clone()
}

/// Portable data, using the phase templates in src/retained/{model,tests}.rs at
/// 43753e826bfd3457c9a0c87ca6390a9e4c46cdfc. Bind local paths/host/IDs before hashing;
/// each publication uses real preceding Git commits, without invoking a guarded writer.
pub fn seed_historical_operation(
    f: &RetainedEnv,
    scenario: HistoricalScenario,
) -> HistoricalOperation {
    f.json(&["start", &f.id]);
    let transfer = matches!(
        scenario,
        HistoricalScenario::TransferReceiverPublished | HistoricalScenario::TransferSenderReleased
    );
    let receiver = transfer.then(|| {
        let id = f.json(&["add", "Historical receiver"])["id"]
            .as_str()
            .unwrap()
            .to_string();
        f.json(&["start", &id]);
        id
    });
    let payload = f.dir.join("payload");
    std::fs::write(&payload, "original").unwrap();
    let mut record = f.record();
    record["host"] = json!(host());
    record["path"] = json!(payload);
    record["sha256"] = json!(digest(b"original"));
    record["cleanup_trigger"] = json!("Review completes");
    if transfer {
        f.note(record.clone());
    }
    let operation_id = format!(
        "{:032x}",
        f.state()["operations"].as_array().unwrap().len() + 1
    );
    let mut tick = 0;
    let initial = time::OffsetDateTime::parse(
        f.json(&["show", &f.id])["task"]["updated"]
            .as_str()
            .unwrap(),
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap();
    let mut at = || {
        tick += 1;
        (initial + time::Duration::seconds(tick))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let mut inputs = vec![json!({"task":f.id,"basis":historical_basis(f, &f.id)})];
    if let Some(id) = &receiver {
        inputs.push(json!({"task":id,"basis":historical_basis(f,id)}));
    }
    let mut request =
        json!({"version":1,"operation_id":operation_id,"host":host(),"inputs":inputs});
    if transfer {
        request["items"] = json!([{"host":record["host"],"kind":"evidence","path":record["path"]}]);
        request["purpose"] = json!("Continue review");
        request["cleanup_trigger"] = json!("Review completes");
    } else {
        request["declarations"] = json!([declaration(&payload, "evidence")]);
    }
    let bytes = serde_json::to_vec(&request).unwrap();
    f.request("request.json", &request);
    let name = format!(
        "retained-op-{}.request.json",
        request["operation_id"].as_str().unwrap()
    );
    let locator = json!({"type":"attachment","task":f.id,"name":name,"sha256":digest(&bytes)});
    let time = at();
    historical_batch(
        f,
        &f.id,
        &time,
        &[(&name, &bytes)],
        &[historical_phase(
            &request,
            &locator,
            "intent",
            &time,
            json!({}),
        )],
    );
    let intent_basis = historical_basis(f, &f.id);
    let mut reconciliation = None;
    match scenario {
        HistoricalScenario::CheckpointPending => {}
        HistoricalScenario::CheckpointCompleted => {
            let ordinal = f.json(&["show", &f.id])["task"]["notes"]
                .as_array()
                .unwrap()
                .len()
                + 1;
            let time = at();
            historical_batch(
                f,
                &f.id,
                &time,
                &[],
                &[
                    format!("retained: {record}"),
                    historical_phase(
                        &request,
                        &locator,
                        "items_published",
                        &time,
                        json!({"items":[ordinal],"artifacts":[],"resolutions":[]}),
                    ),
                ],
            );
            let item_basis = historical_basis(f, &f.id);
            let count = f.json(&["show", &f.id])["task"]["notes"]
                .as_array()
                .unwrap()
                .len();
            let time = at();
            let checkpoint = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":request["operation_id"]},"observed_at":time,"host":host(),"observer":null,"basis":item_basis,"scope":{"mode":"all_known_kept_on_host","host":host(),"other_hosts":"outside_observation"},"counts":{"kept_verified":1,"kept_unknown":0,"released":0},"changed":1,"actions":[],"complete":true});
            let publications = json!([{"task":f.id,"phase":"intent","basis":intent_basis},{"task":f.id,"phase":"items_published","basis":item_basis}]);
            historical_batch(
                f,
                &f.id,
                &time,
                &[],
                &[
                    format!("retention checkpoint: {checkpoint}"),
                    historical_phase(
                        &request,
                        &locator,
                        "completed",
                        &time,
                        json!({"outcome":"verified","publications":publications,"checkpoint":count+1}),
                    ),
                ],
            );
        }
        HistoricalScenario::TransferReceiverPublished
        | HistoricalScenario::TransferSenderReleased => {
            let to = receiver.as_ref().unwrap();
            let mut foreign_locator = locator.clone();
            foreign_locator["basis"] = intent_basis;
            let time = at();
            historical_batch(
                f,
                to,
                &time,
                &[],
                &[historical_phase(
                    &request,
                    &foreign_locator,
                    "intent",
                    &time,
                    json!({}),
                )],
            );
            let mapping = json!([{"sender":request["items"][0],"receiver":request["items"][0]}]);
            let mut receiving = record.clone();
            receiving["next_owner"] = json!(format!("task:{to}"));
            receiving["purpose"] = request["purpose"].clone();
            receiving["cleanup_trigger"] = request["cleanup_trigger"].clone();
            let ordinal = f.json(&["show", to])["task"]["notes"]
                .as_array()
                .unwrap()
                .len()
                + 1;
            let time = at();
            historical_batch(
                f,
                to,
                &time,
                &[],
                &[
                    format!("retained: {receiving}"),
                    historical_phase(
                        &request,
                        &foreign_locator,
                        "receiver_published",
                        &time,
                        json!({"items":[ordinal],"artifacts":[],"mapping":mapping}),
                    ),
                ],
            );
            if matches!(scenario, HistoricalScenario::TransferSenderReleased) {
                let receiver_basis = historical_basis(f, to);
                let ordinal = f.json(&["show", &f.id])["task"]["notes"]
                    .as_array()
                    .unwrap()
                    .len()
                    + 1;
                record["state"] = json!("released");
                record["next_owner"] = json!("none");
                record["cleanup_trigger"] = json!("none");
                record["purpose"] = json!(format!(
                    "Transferred to {to}; custody transferred and bytes remain in place"
                ));
                record.as_object_mut().unwrap().remove("sha256");
                let time = at();
                historical_batch(
                    f,
                    &f.id,
                    &time,
                    &[],
                    &[
                        format!("retained: {record}"),
                        historical_phase(
                            &request,
                            &locator,
                            "sender_released",
                            &time,
                            json!({"items":[ordinal],"receiver_basis":receiver_basis,"mapping":mapping}),
                        ),
                    ],
                );
            }
        }
        HistoricalScenario::Reconciled => {
            let directive = json!({"version":1,"operation":{"coordinator":f.id,"operation_id":request["operation_id"]},"reconciliation_id":"c".repeat(32),"inputs":[{"task":f.id,"basis":historical_basis(f,&f.id)}],"reason":"Historical fixture recovery"});
            let data = serde_json::to_vec(&directive).unwrap();
            let name = format!("retained-op-{}.request.json", "c".repeat(32));
            let directive_locator =
                json!({"type":"attachment","task":f.id,"name":name,"sha256":digest(&data)});
            let time = at();
            historical_batch(
                f,
                &f.id,
                &time,
                &[(&name, &data)],
                &[historical_phase(
                    &request,
                    &locator,
                    "reconcile_intent",
                    &time,
                    json!({"directive":directive_locator,"reconciliation_id":directive["reconciliation_id"]}),
                )],
            );
            let time = at();
            historical_batch(
                f,
                &f.id,
                &time,
                &[],
                &[historical_phase(
                    &request,
                    &locator,
                    "reconcile_ready",
                    &time,
                    json!({"directive":directive_locator,"reconciliation_id":directive["reconciliation_id"],"items":[],"unresolved":[]}),
                )],
            );
            let ready = historical_basis(f, &f.id);
            let time = at();
            historical_batch(
                f,
                &f.id,
                &time,
                &[],
                &[historical_phase(
                    &request,
                    &locator,
                    "reconciled",
                    &time,
                    json!({"directive":directive_locator,"reconciliation_id":directive["reconciliation_id"],"ready_bases":[{"task":f.id,"phase":"reconcile_ready","basis":ready}],"unresolved":[]}),
                )],
            );
            reconciliation = Some(directive);
        }
    }
    // Setup validates full immutable proof before any behavior test retries it.
    let state = f.state();
    assert_eq!(state["complete"], true, "{state}");
    if let Some(to) = &receiver {
        assert_eq!(f.json(&["retained", "state", to])["complete"], true);
    }
    let actual = std::fs::read(f.records.join(format!("tasks/files/{}/{name}", f.id))).unwrap();
    assert_eq!(actual, bytes);
    assert_eq!(digest(&actual), locator["sha256"]);
    let root = state["basis"]["records_subdir"].as_str().unwrap();
    let path = format!("{root}/tasks/files/{}/{name}", f.id)
        .trim_start_matches("./")
        .to_string();
    let object = git_fixture::command(f.env.home.path(), &f.records)
        .args([
            "show",
            &format!("{}:{path}", state["basis"]["commit"].as_str().unwrap()),
        ])
        .output()
        .unwrap();
    assert!(object.status.success());
    assert_eq!(object.stdout, bytes);
    HistoricalOperation {
        request,
        receiver,
        reconciliation,
    }
}
