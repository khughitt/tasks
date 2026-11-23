//! `tasks register` and `tasks notices`: one host takes up what another host put in a
//! shared records store (docs/project-operations.md#taking-up-synced-records).
mod common;
use common::{TestEnv, git};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// A git repository with one commit at `dir`.
fn repo(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "seed",
        ],
    );
    dir.canonicalize().unwrap()
}

/// The shared tree two hosts see: a records store and one code checkout per prefix.
struct Tree {
    _temp: tempfile::TempDir,
    base: PathBuf,
    records: PathBuf,
}

impl Tree {
    fn new() -> Tree {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let records = repo(&base.join("records"));
        Tree {
            _temp: temp,
            base,
            records,
        }
    }

    fn code(&self, prefix: &str) -> PathBuf {
        self.base.join("code").join(prefix)
    }

    /// Creates `prefix`'s checkout and registers it on `host` with detached records.
    fn detached(&self, host: &TestEnv, prefix: &str) -> PathBuf {
        let code = self.code(prefix);
        if !code.exists() {
            repo(&code);
        }
        host.json(&code, &self.init_args(prefix));
        code
    }

    fn init_args<'a>(&'a self, prefix: &'a str) -> Vec<&'a str> {
        vec![
            "init",
            "--prefix",
            prefix,
            "--records",
            self.records.to_str().unwrap(),
        ]
    }

    fn commit_records(&self) {
        git(&self.records, &["add", "-A"]);
        git(
            &self.records,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "records",
            ],
        );
    }
}

fn registry_text(env: &TestEnv) -> String {
    fs::read_to_string(env.home.path().join(".config/tasks/projects.toml")).unwrap()
}

/// `notices` in JSON; an empty stdout (nothing to take up) is no rows.
fn notices(env: &TestEnv, dir: &Path) -> Vec<Value> {
    let out = env.cmd(dir).arg("notices").output().unwrap();
    assert!(out.status.success(), "{out:?}");
    if out.stdout.is_empty() {
        return Vec::new();
    }
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    value["notices"].as_array().unwrap().clone()
}

/// Splits a printed command as a POSIX shell would for the quoting the commands use:
/// bare words and single-quoted spans, with `'\''` for a quote inside one.
fn shell_split(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = command.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' => words.extend(word.take()),
            '\'' => {
                let word = word.get_or_insert_with(String::new);
                for quoted in chars.by_ref() {
                    if quoted == '\'' {
                        break;
                    }
                    word.push(quoted);
                }
            }
            '\\' => word
                .get_or_insert_with(String::new)
                .push(chars.next().expect("escaped character")),
            other => word.get_or_insert_with(String::new).push(other),
        }
    }
    words.extend(word);
    words
}

/// Runs a notice's or row's command word by word, as a person pasting it would.
fn run_command(env: &TestEnv, command: &str) -> Value {
    let words = shell_split(command);
    assert_eq!(words[0], "tasks", "{command}");
    let args: Vec<&str> = words[1..].iter().map(String::as_str).collect();
    env.json(Path::new("/"), &args)
}

#[test]
fn notices_and_register_take_up_every_entry_present_on_this_host() {
    let a = TestEnv::new();
    let b = TestEnv::new();
    let tree = Tree::new();
    let anchor = tree.detached(&a, "aa");
    for prefix in ["bb", "cc", "gone", "old"] {
        tree.detached(&a, prefix);
    }
    a.json(&tree.code("old"), &["retire", "--reason", "ended"]);
    fs::remove_dir_all(tree.code("gone")).unwrap();
    // Host b registered one project in the store before the others appeared.
    b.json(&anchor, &tree.init_args("aa"));

    let rows = notices(&b, &anchor);
    let expected = |prefix: &str| {
        format!(
            "tasks -C {} init --prefix {prefix} --records {}",
            tree.code(prefix).display(),
            tree.records.display()
        )
    };
    assert_eq!(rows.len(), 2, "{rows:?}");
    for (row, prefix) in rows.iter().zip(["bb", "cc"]) {
        assert_eq!(row["kind"], "unregistered");
        assert_eq!(row["prefix"], prefix);
        assert_eq!(row["command"], expected(prefix));
        assert!(row.get("old").is_none());
    }
    let pretty = b.pretty(&anchor, &["notices"]);
    let lines: Vec<_> = pretty.lines().collect();
    assert_eq!(lines.len(), 2, "{pretty}");
    assert!(
        lines[0].starts_with("tasks: bb is a records entry"),
        "{pretty}"
    );
    assert!(lines[0].ends_with(&format!("; run `{}`", expected("bb"))));

    let before = registry_text(&b);
    let explained = b.json(&anchor, &["register", "--explain"]);
    assert_eq!(explained["explain"], true);
    let outcomes: Vec<_> = explained["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["prefix"].clone(),
                row["outcome"].clone(),
                row["command"].clone(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            ("bb".into(), "would_register".into(), expected("bb").into()),
            ("cc".into(), "would_register".into(), expected("cc").into()),
        ]
    );
    assert_eq!(explained["results"][0]["kind"], "detached");
    assert_eq!(registry_text(&b), before, "--explain wrote the registry");

    for (prefix, why) in [
        ("gone", "is not on this host"),
        ("old", "retired"),
        ("aa", "already registered"),
        ("zz", "no records store"),
    ] {
        let out = b
            .cmd(&anchor)
            .args(["register", "bb", prefix])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{out:?}");
        let error: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(error["error"]["kind"], "validation");
        let detail = error["error"]["detail"].as_str().unwrap();
        assert!(detail.contains(prefix) && detail.contains(why), "{detail}");
        assert_eq!(registry_text(&b), before, "a refused register wrote");
    }

    // The printed command is exact: running it registers the entry.
    run_command(&b, &expected("bb"));
    let rows = notices(&b, &anchor);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["prefix"], "cc");

    let registered = b.json(&anchor, &["register"]);
    assert_eq!(registered["results"][0]["prefix"], "cc");
    assert_eq!(registered["results"][0]["outcome"], "registered");
    assert_eq!(
        b.json(&tree.code("cc"), &["where"])["project"]["registered"],
        true
    );

    // Nothing left: silent in both formats, exit 0.
    for args in [&["notices"][..], &["--pretty", "notices"][..]] {
        let out = b.cmd(&anchor).args(args).output().unwrap();
        assert!(out.status.success(), "{out:?}");
        assert!(out.stdout.is_empty(), "{out:?}");
        assert!(out.stderr.is_empty(), "{out:?}");
    }
    assert_eq!(
        b.json(&anchor, &["register"])["results"],
        serde_json::json!([])
    );
    assert!(
        b.pretty(&anchor, &["register"])
            .starts_with("nothing to register")
    );
}

#[test]
fn a_failed_init_is_an_error_row_and_exit_1_after_every_entry_is_tried() {
    let a = TestEnv::new();
    let b = TestEnv::new();
    let tree = Tree::new();
    let anchor = tree.detached(&a, "aa");
    tree.detached(&a, "bb");
    tree.detached(&a, "cc");
    b.json(&anchor, &tree.init_args("aa"));
    // bb's checkout is no longer a git repository, so its init refuses.
    fs::remove_dir_all(tree.code("bb").join(".git")).unwrap();
    let out = b.cmd(&anchor).arg("register").output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["results"].as_array().unwrap();
    assert_eq!(rows[0]["prefix"], "bb");
    assert_eq!(rows[0]["outcome"], "error");
    assert!(
        rows[0]["detail"].as_str().unwrap().contains("git"),
        "{value}"
    );
    assert_eq!(rows[1]["prefix"], "cc");
    assert_eq!(rows[1]["outcome"], "registered");
}

#[test]
fn notices_name_a_detached_rename_another_host_made_with_its_adopt_command() {
    let a = TestEnv::new();
    let b = TestEnv::new();
    let tree = Tree::new();
    let anchor = tree.detached(&a, "aa");
    let code = tree.detached(&a, "det");
    b.json(&anchor, &tree.init_args("aa"));
    b.json(&code, &tree.init_args("det"));
    tree.commit_records();
    a.json(&code, &["rename", "det", "dtx"]);
    assert!(tree.records.join("dtx").is_dir());

    let rows = notices(&b, &anchor);
    assert_eq!(
        rows.len(),
        1,
        "the new prefix is a rename, not a registration: {rows:?}"
    );
    assert_eq!(rows[0]["kind"], "unadopted_rename");
    assert_eq!(rows[0]["old"], "det");
    assert_eq!(rows[0]["prefix"], "dtx");
    let command = format!("tasks -C {} rename det dtx --adopt", code.display());
    assert_eq!(rows[0]["command"], command);
    assert!(
        b.pretty(&anchor, &["notices"])
            .contains("det was renamed to dtx on another host")
    );
    let refused = b.cmd(&anchor).args(["register", "dtx"]).output().unwrap();
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("adopt the rename"));

    run_command(&b, &command);
    assert_eq!(notices(&b, &anchor), Vec::<Value>::new());
}

#[test]
fn notices_name_an_in_tree_rename_another_host_made() {
    let a = TestEnv::new();
    let b = TestEnv::new();
    let temp = tempfile::tempdir().unwrap();
    let dir = repo(&temp.path().join("it"));
    a.json(&dir, &["init", "--prefix", "it"]);
    b.json(&dir, &["init", "--prefix", "it"]);
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "init",
        ],
    );
    a.json(&dir, &["rename", "it", "itx"]);

    let rows = notices(&b, &dir);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["kind"], "unadopted_rename");
    assert_eq!(
        (&rows[0]["old"], &rows[0]["prefix"]),
        (&"it".into(), &"itx".into())
    );
    let command = format!("tasks -C {} rename it itx --adopt", dir.display());
    assert_eq!(rows[0]["command"], command);
    run_command(&b, &command);
    assert_eq!(notices(&b, &dir), Vec::<Value>::new());
}

#[test]
fn shell_split_reads_the_quoting_the_commands_print() {
    assert_eq!(
        shell_split("tasks -C '/a b/it'\\''s' init"),
        ["tasks", "-C", "/a b/it's", "init"]
    );
}

/// A linked entry, its checkout under a path with a space, and an in-tree entry, both in
/// an overlay store this host knows through one linked project.
#[test]
fn notices_and_register_cover_linked_and_in_tree_entries_and_quote_paths() {
    let a = TestEnv::new();
    let b = TestEnv::new();
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let overlay = repo(&base.join("overlay"));
    let linked = |host: &TestEnv, prefix: &str| {
        let code = base.join("code dir").join(prefix);
        if !code.exists() {
            repo(&code);
        }
        let dir = overlay.join(prefix);
        fs::create_dir_all(&dir).unwrap();
        host.json(
            &dir,
            &[
                "init",
                "--prefix",
                prefix,
                "--checkout",
                code.to_str().unwrap(),
            ],
        );
        (dir, code)
    };
    let (anchor, _) = linked(&a, "an");
    let (ln, ln_code) = linked(&a, "ln");
    let it = overlay.join("it");
    fs::create_dir_all(&it).unwrap();
    a.json(&it, &["init", "--prefix", "it"]);
    linked(&b, "an");

    let rows = notices(&b, &anchor);
    let commands: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row["prefix"].as_str().unwrap(),
                row["command"].as_str().unwrap(),
            )
        })
        .collect();
    let ln_command = format!(
        "tasks -C {} init --prefix ln --checkout '{}'",
        ln.display(),
        ln_code.display()
    );
    let it_command = format!("tasks -C {} init --prefix it", it.display());
    assert_eq!(
        commands,
        [("it", it_command.as_str()), ("ln", ln_command.as_str())]
    );
    let explained = b.json(&anchor, &["register", "--explain"]);
    let kinds: Vec<_> = explained["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["in-tree", "linked"]);

    // The quoted command registers the linked entry; register takes the in-tree one.
    run_command(&b, &ln_command);
    let registered = b.json(&anchor, &["register"]);
    assert_eq!(registered["results"][0]["prefix"], "it");
    assert_eq!(registered["results"][0]["outcome"], "registered");
    assert_eq!(notices(&b, &anchor), Vec::<Value>::new());
    let resolved = b.json(&anchor, &["resolve", "ln", "it"]);
    for (row, kind) in resolved["results"]
        .as_array()
        .unwrap()
        .iter()
        .zip(["linked", "in-tree"])
    {
        assert_eq!(row["status"], "resolved", "{resolved}");
        assert_eq!(row["entry"]["kind"], kind, "{resolved}");
    }
    assert_eq!(
        resolved["results"][0]["entry"]["checkout"],
        ln_code.to_str().unwrap()
    );
}
