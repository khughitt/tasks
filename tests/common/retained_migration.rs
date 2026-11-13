use super::{
    git, git_fixture,
    retained::{HistoricalScenario, RetainedEnv, seed_historical_operation},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct MigrationFixture {
    pub f: RetainedEnv,
    pub source: PathBuf,
    pub destination: PathBuf,
    pub cutover: String,
    pub import: String,
    pub original: Value,
    pub declaration: Value,
}
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn oid(f: &RetainedEnv, root: &Path) -> String {
    let out = git_fixture::command(f.env.home.path(), root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().into()
}
pub fn migration_fixture(scenario: HistoricalScenario) -> MigrationFixture {
    let mut f = RetainedEnv::new("in-tree");
    seed_historical_operation(&f, scenario);
    let original = f.state();
    let source = f.dir.clone();
    let cutover = oid(&f, &source);
    let destination = source.parent().unwrap().join("migrated-records");
    std::fs::create_dir(&destination).unwrap();
    git(&destination, &["init", "-q", "-b", "main"]);
    git_fixture::assert_repo(f.env.home.path(), &destination);
    std::fs::write(destination.join("README"), "records seed\n").unwrap();
    git(&destination, &["add", "."]);
    git(&destination, &["commit", "-qm", "records seed"]);
    git(&destination, &["checkout", "-q", "--orphan", "export"]);
    git(&destination, &["rm", "-qf", "README"]);
    copy_tree(&source.join("tasks"), &destination.join("sci/tasks"));
    git(&destination, &["add", "."]);
    git(&destination, &["commit", "-qm", "prefixed snapshot"]);
    git(&destination, &["checkout", "-q", "main"]);
    let message = format!(
        "Import fixture\n\nRecords-Migrate-Prefix: sci\nRecords-Migrate-Source: {cutover}\nRecords-Migrate-Paths: tasks docs/specs docs/plans docs/notes docs/superpowers docs/designs docs/reports\nRecords-Migrate-Citations: 2"
    );
    git(
        &destination,
        &[
            "merge",
            "-q",
            "--no-ff",
            "--allow-unrelated-histories",
            "export",
            "-m",
            &message,
        ],
    );
    let import = oid(&f, &destination);
    git(&source, &["rm", "-qr", "tasks"]);
    git(&source, &["commit", "-qm", "migrate records"]);
    f.json(&[
        "init",
        "--prefix",
        "sci",
        "--records",
        destination.to_str().unwrap(),
        "--force",
    ]);
    f.records = destination.join("sci");
    let tasks = std::fs::read_dir(f.records.join("tasks"))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            name.strip_suffix(".md")
                .filter(|v| v.starts_with("sci-"))
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let declaration = json!({"version":1,"source_cutover":cutover,"destination_import":import,"source_ref":"refs/heads/main","tasks":tasks,"refs":[{"original_ref":"refs/heads/main","endpoint":cutover}],"manifest_bindings":[]});
    let fixture = MigrationFixture {
        f,
        source,
        destination,
        cutover,
        import,
        original,
        declaration,
    };
    fixture.write_declaration(&fixture.declaration);
    fixture
}
impl MigrationFixture {
    pub fn write_declaration(&self, declaration: &Value) {
        let path = self.f.records.join("tasks/.config.toml");
        let mut config: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        config.as_table_mut().unwrap().insert(
            "retention_recovery".into(),
            toml::Value::try_from(declaration).unwrap(),
        );
        std::fs::write(path, toml::to_string(&config).unwrap()).unwrap();
        git(&self.destination, &["add", "."]);
        git(&self.destination, &["commit", "-qm", "declare recovery"]);
    }
}
impl MigrationFixture {
    pub fn git(&self, root: &Path, args: &[&str]) -> String {
        let out = git_fixture::command(self.f.env.home.path(), root)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    pub fn replace_import(&mut self, case: &str) {
        let config_path = self.f.records.join("tasks/.config.toml");
        let config = std::fs::read(&config_path).unwrap();
        self.git(&self.destination, &["reset", "--hard", &self.import]);
        let task = self.f.records.join(format!("tasks/{}.md", self.f.id));
        match case {
            "config" => std::fs::write(&config_path, "prefix='sci'\n# changed blob\n").unwrap(),
            "task" => {
                let raw = std::fs::read_to_string(&task).unwrap();
                std::fs::write(task, raw.replace("Retained fixture", "Different import")).unwrap();
            }
            "mode" => {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(task, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            "tree" => std::fs::write(
                self.f
                    .records
                    .join(format!("tasks/files/{}/extra", self.f.id)),
                "not in source",
            )
            .unwrap(),
            _ => {}
        }
        self.git(&self.destination, &["add", "."]);
        let message = format!(
            "Import fixture\n\nRecords-Migrate-Prefix: sci\nRecords-Migrate-Source: {}\nRecords-Migrate-Paths: tasks docs/specs docs/plans docs/notes docs/superpowers docs/designs docs/reports\nRecords-Migrate-Citations: 2",
            self.cutover
        );
        let message = match case {
            "missing_trailer" => message.replace("Records-Migrate-Citations: 2", ""),
            "duplicate_trailer" => format!("{message}\nRecords-Migrate-Prefix: sci"),
            "body_trailer" => format!("{message}\n\nNot a trailer block"),
            "rule" => message.replace(
                "Records-Migrate-Citations: 2",
                "Records-Migrate-Citations: 1",
            ),
            _ => message,
        };
        self.git(&self.destination, &["commit", "--amend", "-qm", &message]);
        self.import = self.git(&self.destination, &["rev-parse", "HEAD"]);
        std::fs::write(config_path, config).unwrap();
        self.declaration["destination_import"] = json!(self.import);
        self.write_declaration(&self.declaration);
    }
}
