mod common;

use common::{TestEnv, git, git_fixture};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn init_git_project(env: &TestEnv, root: &Path, name: &str, prefix: &str) -> PathBuf {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git_fixture::assert_repo(env.home.path(), &dir);
    std::fs::write(dir.join("README.md"), format!("{prefix}\n")).unwrap();
    git(&dir, &["add", "README.md"]);
    git(&dir, &["commit", "-qm", "seed"]);
    env.json(&dir, &["init", "--prefix", prefix]);
    let cfg = dir.join("tasks/.config.toml");
    let mut doc: toml::Value = toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
    doc.as_table_mut().unwrap().insert(
        "git".into(),
        toml::Value::Table(toml::map::Map::from_iter([(
            "commit".into(),
            toml::Value::Boolean(true),
        )])),
    );
    std::fs::write(&cfg, toml::to_string(&doc).unwrap()).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "enable git commit"]);
    dir
}

fn kept_worktree(host_name: &str, path: &str, next_owner: &str) -> Value {
    json!({
        "version": 1,
        "host": host_name,
        "kind": "worktree",
        "path": path,
        "state": "kept",
        "verification": "verified",
        "purpose": "Retained worktree fixture",
        "next_owner": next_owner,
        "cleanup_trigger": "After target task completes"
    })
}

fn released_worktree(host_name: &str, path: &str) -> Value {
    json!({
        "version": 1,
        "host": host_name,
        "kind": "worktree",
        "path": path,
        "state": "released",
        "verification": "verified",
        "purpose": "Removed worktree after verification",
        "next_owner": "none",
        "cleanup_trigger": "none"
    })
}

fn seed_raw_note(records_dir: &Path, task_id: &str, note_text: &str) {
    let path = records_dir.join(format!("tasks/{task_id}.md"));
    let mut raw = std::fs::read_to_string(&path).unwrap();
    if !raw.contains("\n## Notes\n") {
        raw.push_str("\n## Notes\n\n");
    }
    raw.push_str(&format!("- 2026-11-18T00:00:00Z (fixture): {note_text}\n"));
    std::fs::write(&path, raw).unwrap();
    git(records_dir, &["add", "tasks"]);
    git(records_dir, &["commit", "-qm", "seed raw note"]);
}

#[test]
fn same_project_and_all_projects_custody_with_retired_projects_and_host_filter() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    // 1. Unregistered project + empty registry: local query has zero registry warnings,
    //    while --all-projects emits `registry_empty` diagnostic and `"registry is empty"` warning.
    let unreg = init_git_project(&env, &root, "unreg", "unr");
    env.json(&unreg, &["unregister", "unr"]);
    let u_focal = env.json(&unreg, &["add", "Unregistered focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let u_holder = env.json(&unreg, &["add", "Unregistered holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &unreg,
        &[
            "note",
            &u_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/u1", &format!("task:{u_focal}"))
            ),
        ],
    );
    let local_unreg = env.json(&unreg, &["retained", "state", &u_focal]);
    assert_eq!(local_unreg["warnings"], json!([]));
    assert_eq!(local_unreg["incoming"]["scope"], "project");
    assert_eq!(local_unreg["incoming"]["complete"], true);
    assert_eq!(
        local_unreg["incoming"]["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(local_unreg["incoming"]["items"][0]["holder"], u_holder);

    let empty_all = env.json(&unreg, &["retained", "state", &u_focal, "--all-projects"]);
    assert_eq!(empty_all["incoming"]["scope"], "all_projects");
    assert_eq!(empty_all["incoming"]["complete"], false);
    assert_eq!(
        empty_all["incoming"]["diagnostics"][0]["code"],
        "registry_empty"
    );
    assert!(
        empty_all["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("registry is empty")),
        "{empty_all}"
    );

    // 2. Registered projects: `sci` (active), `fam` (active), `old` (retired without --successor).
    let sci = init_git_project(&env, &root, "sci", "sci");
    let fam = init_git_project(&env, &root, "fam", "fam");
    let old = init_git_project(&env, &root, "old", "old");

    let focal = env.json(&sci, &["add", "Focal target"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sci_holder = env.json(&sci, &["add", "Same-project holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let fam_holder = env.json(&fam, &["add", "Cross-project holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let old_holder = env.json(&old, &["add", "Retired-project holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();

    env.json(
        &sci,
        &[
            "note",
            &sci_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/sci-item", &format!("task:{focal}"))
            ),
        ],
    );
    env.json(
        &fam,
        &[
            "note",
            &fam_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-b", "/work/fam-item", &format!("task:{focal}"))
            ),
        ],
    );
    env.json(
        &old,
        &[
            "note",
            &old_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/old-item", &format!("task:{focal}"))
            ),
        ],
    );
    env.json(&old, &["start", &old_holder]);
    env.json(&old, &["done", &old_holder, "Closed before retirement"]);
    env.json(&old, &["retire", "--reason", "Project completed"]);
    git(&old, &["add", "."]);
    git(&old, &["commit", "-qm", "retire old"]);

    let same_only = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(same_only["incoming"]["scope"], "project");
    assert_eq!(same_only["incoming"]["complete"], true);
    let same_holders: Vec<&str> = same_only["incoming"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["holder"].as_str().unwrap())
        .collect();
    assert_eq!(same_holders, vec![sci_holder.as_str()]);

    let all = env.json(&sci, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(all["incoming"]["scope"], "all_projects");
    assert_eq!(all["incoming"]["complete"], true, "{all}");
    let mut expected_holders = vec![fam_holder.clone(), old_holder.clone(), sci_holder.clone()];
    expected_holders.sort();
    let actual_holders: Vec<String> = all["incoming"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["holder"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(actual_holders, expected_holders);

    // 3. --host filter narrows incoming.items and host-scoped diagnostics while preserving task-wide diagnostics.
    let host_b_only = env.json(
        &sci,
        &[
            "retained",
            "state",
            &focal,
            "--all-projects",
            "--host",
            "host-b",
        ],
    );
    assert_eq!(host_b_only["incoming"]["complete"], true);
    assert_eq!(
        host_b_only["incoming"]["items"].as_array().unwrap().len(),
        1
    );
    assert_eq!(host_b_only["incoming"]["items"][0]["holder"], fam_holder);

    // 4. Unreachable registered project emits `project_unreachable`.
    let gone = init_git_project(&env, &root, "gone", "gon");
    std::fs::remove_dir_all(&gone).unwrap();
    let unreachable = env.json(
        &sci,
        &[
            "retained",
            "state",
            &focal,
            "--all-projects",
            "--host",
            "host-b",
        ],
    );
    assert_eq!(unreachable["incoming"]["complete"], false);
    assert!(
        unreachable["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "project_unreachable" && d["project"] == "gon"),
        "{unreachable}"
    );
}

#[test]
fn incoming_excludes_self_and_focal_move_chain_counterparts() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let alpha = init_git_project(&env, &root, "alpha", "alp");
    let beta = init_git_project(&env, &root, "beta", "bet");

    let old_focal = env.json(&alpha, &["add", "Focal before move"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &alpha,
        &[
            "note",
            &old_focal,
            &format!(
                "retained: {}",
                kept_worktree(
                    "host-a",
                    ".worktrees/self-item",
                    &format!("task:{old_focal}")
                )
            ),
        ],
    );

    let moved = env.json(&alpha, &["move", &old_focal, "--to", "bet"]);
    let new_focal = moved["id"].as_str().unwrap().to_string();

    env.json(
        &beta,
        &[
            "note",
            &new_focal,
            &format!(
                "retained: {}",
                kept_worktree(
                    "host-a",
                    ".worktrees/self-item",
                    &format!("task:{new_focal}")
                )
            ),
        ],
    );

    for extra in [vec![], vec!["--all-projects"]] {
        let mut args = vec!["retained", "state", new_focal.as_str()];
        args.extend(extra.clone());
        let st = env.json(&beta, &args);
        assert_eq!(st["complete"], true, "{extra:?}: {st}");
        assert_eq!(st["items"].as_array().unwrap().len(), 1);
        assert_eq!(st["incoming"]["complete"], true, "{extra:?}: {st}");
        assert_eq!(st["incoming"]["items"], json!([]), "{extra:?}: {st}");
    }
}

#[test]
fn focal_and_holder_move_chains_aliases_predecessor_and_unadopted_lineage() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let alpha = init_git_project(&env, &root, "alpha", "alp");
    let beta = init_git_project(&env, &root, "beta", "bet");
    let peer = init_git_project(&env, &root, "peer", "per");

    // 1. Focal task moved `alp -> bet`, then `bet` renamed to `gam` (`aliases: bet -> gam`).
    let focal_alp = env.json(&alpha, &["add", "Focal moving"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let moved_focal = env.json(&alpha, &["move", &focal_alp, "--to", "bet"]);
    let focal_bet = moved_focal["id"].as_str().unwrap().to_string();

    env.json(&beta, &["rename", "bet", "gam"]);
    env.json(&beta, &["commit", "-m", "rename bet -> gam"]);
    let focal_gam = format!("gam-{}", focal_bet.split_once('-').unwrap().1);

    // External holder1 names pre-move `focal_alp`; holder2 names retired alias `focal_bet`.
    let holder_pre_move = env.json(&peer, &["add", "Holder pre-move"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let holder_alias = env.json(&peer, &["add", "Holder alias"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &peer,
        &[
            "note",
            &holder_pre_move,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/h1", &format!("task:{focal_alp}"))
            ),
        ],
    );
    env.json(
        &peer,
        &[
            "note",
            &holder_alias,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/h2", &format!("task:{focal_bet}"))
            ),
        ],
    );

    // 2. Moved candidate holder `h_local_old` in `gam` moved out to `per`:
    //    local `scope: "project"` emits `holder_moved_out_of_scope`, while `--all-projects` resolves via `h_live`.
    let h_local_old = env.json(&beta, &["add", "Holder moving out"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &beta,
        &[
            "note",
            &h_local_old,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/h3", &format!("task:{focal_gam}"))
            ),
        ],
    );
    let moved_holder = env.json(&beta, &["move", &h_local_old, "--to", "per"]);
    let h_live = moved_holder["id"].as_str().unwrap().to_string();

    let local_out = env.json(&beta, &["retained", "state", &focal_gam]);
    assert_eq!(local_out["incoming"]["scope"], "project");
    assert_eq!(local_out["incoming"]["complete"], false, "{local_out}");
    assert!(
        local_out["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "holder_moved_out_of_scope" && d["holder"] == h_local_old),
        "{local_out}"
    );

    let wide_out = env.json(&beta, &["retained", "state", &focal_gam, "--all-projects"]);
    assert_eq!(wide_out["incoming"]["complete"], true, "{wide_out}");
    let mut want_holders = vec![holder_pre_move, holder_alias, h_live];
    want_holders.sort();
    let got_holders: Vec<String> = wide_out["incoming"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["holder"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(got_holders, want_holders);

    // 3. Two-step `[predecessor]` displaced suffix: `prd` retired for successor `suc`,
    //    renamed `prd -> prd1` then `suc -> prd`. `task:prd-<old_hex>` routes ONLY to `prd1-<old_hex>`.
    let prd = init_git_project(&env, &root, "prd", "prd");
    let suc = init_git_project(&env, &root, "suc", "suc");
    let prd_task = env.json(&prd, &["add", "Displaced predecessor task"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let prd_hex = prd_task.split_once('-').unwrap().1.to_string();
    env.json(&prd, &["start", &prd_task]);
    env.json(&prd, &["done", &prd_task, "Done before retirement"]);
    env.json(
        &prd,
        &["retire", "--successor", "suc", "--reason", "Replaced"],
    );
    git(&prd, &["add", "-A"]);
    let _ = git_fixture::command(env.home.path(), &prd)
        .args(["commit", "-qm", "retire prd"])
        .output();
    env.json(&prd, &["rename", "prd", "prd1"]);
    env.json(&prd, &["commit", "-m", "rename prd -> prd1"]);
    env.json(&suc, &["rename", "suc", "prd"]);
    git(&prd, &["add", "-A"]);
    let _ = git_fixture::command(env.home.path(), &prd)
        .args(["commit", "-qm", "handoff predecessor config"])
        .output();
    env.json(&suc, &["commit", "-m", "rename suc -> prd"]);

    let prd_new_task = env.json(&suc, &["add", "New successor task"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let holder_displaced = env.json(&peer, &["add", "Holder naming displaced prd suffix"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &peer,
        &[
            "note",
            &holder_displaced,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/displaced", &format!("task:prd-{prd_hex}"))
            ),
        ],
    );

    let displaced_target = format!("prd1-{prd_hex}");
    let st_prd1 = env.json(
        &prd,
        &["retained", "state", &displaced_target, "--all-projects"],
    );
    assert_eq!(st_prd1["incoming"]["complete"], true, "{st_prd1}");
    assert_eq!(st_prd1["incoming"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(st_prd1["incoming"]["items"][0]["holder"], holder_displaced);

    let st_succ = env.json(
        &suc,
        &["retained", "state", &prd_new_task, "--all-projects"],
    );
    assert_eq!(st_succ["incoming"]["complete"], true);
    assert_eq!(st_succ["incoming"]["items"], json!([]));

    // 4. Unadopted `[retention_lineage]` fallback (synced rename without registry alias) + corrupt lineage failure.
    let lin = init_git_project(&env, &root, "lin", "lin0");
    let lin_focal_old = env.json(&lin, &["add", "Lineage focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let lin_hex = lin_focal_old.split_once('-').unwrap().1.to_string();
    let cutover = String::from_utf8(
        git_fixture::command(env.home.path(), &lin)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    // Simulate synced Git rename `lin0 -> lin1` with `[retention_lineage]` and registry pointing only `lin1` (no alias!).
    let lin_new = format!("lin1-{lin_hex}");
    let old_md = lin.join(format!("tasks/{lin_focal_old}.md"));
    let new_md_raw = std::fs::read_to_string(&old_md).unwrap().replacen(
        &format!("id: {lin_focal_old}\n"),
        &format!("id: {lin_new}\n"),
        1,
    );
    std::fs::remove_file(&old_md).unwrap();
    std::fs::write(lin.join(format!("tasks/{lin_new}.md")), new_md_raw).unwrap();
    let cfg_path = lin.join("tasks/.config.toml");
    std::fs::write(
        &cfg_path,
        format!(
            "prefix = \"lin1\"\n\n[git]\ncommit = true\n\n[[retention_lineage]]\nversion = 1\nfrom_prefix = \"lin0\"\nto_prefix = \"lin1\"\nlayout = \"in-tree\"\nfrom_subdir = \".\"\nto_subdir = \".\"\ncommit = \"{cutover}\"\nretained_ref = \"refs/heads/main\"\n"
        ),
    )
    .unwrap();
    git(&lin, &["add", "-A"]);
    git(&lin, &["commit", "-qm", "unadopted rename lin0 -> lin1"]);
    let reg_path = env.home.path().join(".config/tasks/projects.toml");
    let mut reg_doc: toml::Value =
        toml::from_str(&std::fs::read_to_string(&reg_path).unwrap()).unwrap();
    let proj_val = reg_doc["projects"]
        .as_table_mut()
        .unwrap()
        .remove("lin0")
        .unwrap();
    reg_doc["projects"]
        .as_table_mut()
        .unwrap()
        .insert("lin1".into(), proj_val);
    std::fs::write(&reg_path, toml::to_string(&reg_doc).unwrap()).unwrap();

    let holder_unadopted = env.json(&peer, &["add", "Holder naming unadopted lin0"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &peer,
        &[
            "note",
            &holder_unadopted,
            &format!(
                "retained: {}",
                kept_worktree(
                    "host-a",
                    "/work/unadopted",
                    &format!("task:{lin_focal_old}")
                )
            ),
        ],
    );

    let st_unadopted = env.json(&lin, &["retained", "state", &lin_new, "--all-projects"]);
    assert_eq!(st_unadopted["incoming"]["complete"], true, "{st_unadopted}");
    assert_eq!(
        st_unadopted["incoming"]["items"][0]["holder"],
        holder_unadopted
    );

    // Corrupt `[retention_lineage]` commit on a focal `moved_from` hop -> fails closed via `incoming.diagnostics` (`incoming.complete == false`).
    let moved_lin_focal = env.json(&lin, &["move", &lin_new, "--to", "per"]);
    let lin_in_peer = moved_lin_focal["id"].as_str().unwrap().to_string();
    let bad_cfg = std::fs::read_to_string(&cfg_path)
        .unwrap()
        .replace(&cutover, &"0".repeat(40));
    std::fs::write(&cfg_path, bad_cfg).unwrap();
    git(&lin, &["add", "-A"]);
    git(&lin, &["commit", "-qm", "corrupt retention_lineage"]);
    let st_corrupt_lin = env.json(
        &peer,
        &["retained", "state", &lin_in_peer, "--all-projects"],
    );
    assert_eq!(
        st_corrupt_lin["incoming"]["complete"], false,
        "{st_corrupt_lin}"
    );
    assert!(
        !st_corrupt_lin["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{st_corrupt_lin}"
    );

    // 5. Broken reciprocity on focal `moved_from` emits `move_history_incomplete`.
    let alp_tombstone = alpha.join(format!("tasks/{focal_alp}.md"));
    let broken_tomb = std::fs::read_to_string(&alp_tombstone)
        .unwrap()
        .replace(&format!("moved_to: {focal_bet}"), "moved_to: gam-ffffff");
    std::fs::write(&alp_tombstone, broken_tomb).unwrap();
    git(&alpha, &["add", "-A"]);
    git(&alpha, &["commit", "-qm", "break reciprocity"]);
    let st_broken_move = env.json(&beta, &["retained", "state", &focal_gam]);
    assert_eq!(st_broken_move["incoming"]["complete"], false);
    assert!(
        st_broken_move["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "move_history_incomplete"),
        "{st_broken_move}"
    );
}

#[test]
fn released_and_rehomed_holders_including_moved_tombstone_report_empty_incoming() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let sci = init_git_project(&env, &root, "sci", "sci");
    let fam = init_git_project(&env, &root, "fam", "fam");

    let focal = env.json(&sci, &["add", "Focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = env.json(&sci, &["add", "Other owner"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_rel = env.json(&sci, &["add", "Direct released holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_rehomed = env.json(&sci, &["add", "Direct rehomed holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_move_old = env.json(&sci, &["add", "Moved then released holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();

    for (h, p) in [
        (&h_rel, ".worktrees/rel"),
        (&h_rehomed, ".worktrees/rehome"),
        (&h_move_old, "/work/moved-rel"),
    ] {
        env.json(
            &sci,
            &[
                "note",
                h,
                &format!(
                    "retained: {}",
                    kept_worktree("host-a", p, &format!("task:{focal}"))
                ),
            ],
        );
    }

    env.json(
        &sci,
        &[
            "note",
            &h_rel,
            &format!(
                "retained: {}",
                released_worktree("host-a", ".worktrees/rel")
            ),
        ],
    );
    env.json(
        &sci,
        &[
            "note",
            &h_rehomed,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/rehome", &format!("task:{other}"))
            ),
        ],
    );

    let moved = env.json(&sci, &["move", &h_move_old, "--to", "fam"]);
    let h_move_live = moved["id"].as_str().unwrap().to_string();
    env.json(
        &fam,
        &[
            "note",
            &h_move_live,
            &format!(
                "retained: {}",
                released_worktree("host-a", "/work/moved-rel")
            ),
        ],
    );

    let st = env.json(&sci, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(st["incoming"]["complete"], true, "{st}");
    assert_eq!(st["incoming"]["items"], json!([]), "{st}");
    assert_eq!(st["incoming"]["diagnostics"], json!([]), "{st}");
}

#[test]
fn archived_notes_integrity_and_non_git_or_corrupt_git_projects() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let sci = init_git_project(&env, &root, "sci", "sci");
    let focal = env.json(&sci, &["add", "Focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let holder_arc = env.json(&sci, &["add", "Archived holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &sci,
        &[
            "note",
            &holder_arc,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/arc", &format!("task:{focal}"))
            ),
        ],
    );
    for idx in 0..5 {
        env.json(
            &sci,
            &[
                "note",
                &holder_arc,
                &format!(
                    "Historical progress note {idx} padding active-record bytes so lossless archive-notes saves bytes cleanly"
                ),
            ],
        );
    }

    // 1. Archive inline notes via `tasks archive-notes` -> custody still surfaces cleanly!
    let preview = env.json(&sci, &["archive-notes", &holder_arc]);
    let expect_sha = preview["source_sha256"].as_str().unwrap().to_string();
    let orig_count = preview["note_count"].as_u64().unwrap();
    env.json(
        &sci,
        &["archive-notes", &holder_arc, "--expect", &expect_sha],
    );

    let st_ok = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_ok["incoming"]["complete"], true, "{st_ok}");
    assert_eq!(st_ok["incoming"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(st_ok["incoming"]["items"][0]["holder"], holder_arc);

    let orig_commit = String::from_utf8(
        git_fixture::command(env.home.path(), &sci)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let files_dir = sci.join(format!("tasks/files/{holder_arc}"));
    let archive_file = std::fs::read_dir(&files_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "notes"))
        .unwrap();
    let archive_name = archive_file
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let orig_sha = archive_name
        .strip_prefix("notes-")
        .unwrap()
        .strip_suffix(".notes")
        .unwrap()
        .to_string();

    // 2a. Corrupt archive replaced with parseable notes having mismatched SHA-256 -> `code == "parse"`.
    let fake_notes = b"## Notes\n\n- 2026-11-18T00:00:00Z (fixture): harmless note\n";
    std::fs::write(&archive_file, fake_notes).unwrap();
    git(&sci, &["add", "-A"]);
    git(&sci, &["commit", "-qm", "tamper archive bytes"]);
    let st_bad_sha = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_bad_sha["incoming"]["complete"], false, "{st_bad_sha}");
    assert_eq!(
        st_bad_sha["incoming"]["diagnostics"][0]["code"], "parse",
        "{st_bad_sha}"
    );
    assert_eq!(
        st_bad_sha["incoming"]["diagnostics"][0]["holder"],
        holder_arc
    );

    // 2b. Reset, replace archive + descriptor + ledger with matching SHA-256 but wrong `note_count` -> `code == "parse"`.
    git(&sci, &["reset", "--hard", &orig_commit]);
    let fake_sha = format!("{:x}", Sha256::digest(fake_notes));
    let fake_name = format!("notes-{fake_sha}.notes");
    std::fs::remove_file(&archive_file).unwrap();
    std::fs::write(files_dir.join(&fake_name), fake_notes).unwrap();
    let md_path = sci.join(format!("tasks/{holder_arc}.md"));
    let md_wrong_count = std::fs::read_to_string(&md_path)
        .unwrap()
        .replace(
            &format!("v1:{orig_sha}:{orig_count}"),
            &format!("v1:{fake_sha}:99"),
        )
        .replace(
            &format!("attached: {archive_name}"),
            &format!("attached: {fake_name}"),
        );
    std::fs::write(&md_path, md_wrong_count).unwrap();
    git(&sci, &["add", "-A"]);
    git(&sci, &["commit", "-qm", "tamper note_count"]);
    let st_bad_count = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_bad_count["incoming"]["complete"], false);
    assert_eq!(
        st_bad_count["incoming"]["diagnostics"][0]["code"], "parse",
        "{st_bad_count}"
    );

    // 2c. Reset, append `detached: <archive_name>: ...` -> `code == "parse"`.
    git(&sci, &["reset", "--hard", &orig_commit]);
    seed_raw_note(
        &sci,
        &holder_arc,
        &format!("detached: {archive_name}: removed"),
    );
    let st_detached = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_detached["incoming"]["complete"], false);
    assert_eq!(
        st_detached["incoming"]["diagnostics"][0]["code"], "parse",
        "{st_detached}"
    );

    // 2d. Reset, mismatch frontmatter `id` on a candidate holder -> `code == "retained_history_blocked"`.
    git(&sci, &["reset", "--hard", &orig_commit]);
    let bad_id_holder = env.json(&sci, &["add", "Mismatched id holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &sci,
        &[
            "note",
            &bad_id_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/bad-id", &format!("task:{focal}"))
            ),
        ],
    );
    let bad_md_path = sci.join(format!("tasks/{bad_id_holder}.md"));
    let bad_md = std::fs::read_to_string(&bad_md_path).unwrap().replacen(
        &format!("id: {bad_id_holder}\n"),
        "id: sci-ffffff\n",
        1,
    );
    std::fs::write(&bad_md_path, bad_md).unwrap();
    git(&sci, &["add", "-A"]);
    git(&sci, &["commit", "-qm", "mismatched frontmatter id"]);
    let st_bad_id = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_bad_id["incoming"]["complete"], false);
    assert!(
        st_bad_id["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_history_blocked" && d["holder"] == bad_id_holder),
        "{st_bad_id}"
    );
    git(&sci, &["reset", "--hard", &orig_commit]);

    // 3. Non-Git project: silent when 0 matches, `retained_publication_unsupported` when mentioning suffix.
    let nongit = root.join("nongit");
    std::fs::create_dir_all(&nongit).unwrap();
    env.json(&nongit, &["init", "--prefix", "ngt"]);
    let ngt_unrelated = env.json(&nongit, &["add", "Unrelated non-git task"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = ngt_unrelated;

    let st_nongit_clean = env.json(&sci, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(
        st_nongit_clean["incoming"]["complete"], true,
        "{st_nongit_clean}"
    );

    let ngt_holder = env.json(&nongit, &["add", "Matching non-git holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &nongit,
        &[
            "note",
            &ngt_holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/ngt", &format!("task:{focal}"))
            ),
        ],
    );
    let st_nongit_match = env.json(&sci, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(st_nongit_match["incoming"]["complete"], false);
    assert!(
        st_nongit_match["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_publication_unsupported"
                && d["project"] == "ngt"
                && d["holder"] == ngt_holder),
        "{st_nongit_match}"
    );

    // 4. Corrupt `.git` marker in a registered project with zero tasks fails closed.
    let badgit = root.join("badgit");
    std::fs::create_dir_all(&badgit).unwrap();
    env.json(&badgit, &["init", "--prefix", "bgt"]);
    std::fs::write(badgit.join(".git"), "gitdir: /nonexistent-gitdir-target\n").unwrap();
    let st_badgit = env.json(&sci, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(st_badgit["incoming"]["complete"], false);
    assert!(
        st_badgit["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["project"] == "bgt" && d["holder"].is_null()),
        "{st_badgit}"
    );
}

#[test]
fn incoming_spaceless_resolution_clears_guard_and_respects_host_filter() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let sci = init_git_project(&env, &root, "sci", "sci");
    let focal = env.json(&sci, &["add", "Focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let focal_hex = focal.split_once('-').unwrap().1;

    // 1. Malformed spaced `retained: ` mentioning suffix (`"next_owner": "sci-<hex>"`) + blocked item.
    let h_mal = env.json(&sci, &["add", "Malformed spaced holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    seed_raw_note(
        &sci,
        &h_mal,
        &format!(
            r#"retained: {{"version":1,"host":"host-a","kind":"worktree","path":".worktrees/m1","state":"kept","verification":"verified","purpose":"p","next_owner":"sci-{focal_hex}","cleanup_trigger":"c"}}"#
        ),
    );

    let h_blocked = env.json(&sci, &["add", "Quarantined item holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(
        &sci,
        &[
            "note",
            &h_blocked,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/b1", &format!("task:{focal}"))
            ),
        ],
    );
    seed_raw_note(
        &sci,
        &h_blocked,
        r#"retained: {"host":"host-a","version":2}"#,
    );

    let st_mal = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_mal["incoming"]["complete"], false, "{st_mal}");
    assert_eq!(
        st_mal["incoming"]["items"],
        json!([]),
        "quarantined item must not surface in incoming.items"
    );
    let codes: Vec<&str> = st_mal["incoming"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"malformed_record"), "{st_mal}");
    assert!(codes.contains(&"holder_item_blocked"), "{st_mal}");

    // 2. Clean repo for spaceless note + blast-radius quarantine + standalone resolution + host filter.
    let clean = init_git_project(&env, &root, "clean", "cln");
    let cln_focal = env.json(&clean, &["add", "Clean focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_sp = env.json(&clean, &["add", "Spaceless holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    env.json(&clean, &["start", &h_sp]);
    env.json(&clean, &["done", &h_sp, "Closed holder"]);

    // Seed valid kept item followed by unresolved spaceless note for the same identity.
    env.json(
        &clean,
        &[
            "note",
            &h_sp,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/sp", &format!("task:{cln_focal}"))
            ),
        ],
    );
    let spaceless = format!(
        r#"retained:{{"version":1,"host":"host-a","kind":"worktree","path":".worktrees/sp","state":"kept","verification":"verified","purpose":"p","next_owner":"task:{cln_focal}","cleanup_trigger":"c"}}"#
    );
    seed_raw_note(&clean, &h_sp, &spaceless);

    let st_unresolved = env.json(&clean, &["retained", "state", &cln_focal]);
    assert_eq!(st_unresolved["incoming"]["complete"], false);
    assert_eq!(
        st_unresolved["incoming"]["items"],
        json!([]),
        "spaceless blast radius must quarantine earlier matching item"
    );
    assert_eq!(
        st_unresolved["incoming"]["diagnostics"][0]["code"],
        "malformed_record"
    );
    assert_eq!(
        st_unresolved["incoming"]["diagnostics"][0]["detail"],
        "missing space after 'retained:'"
    );
    assert_eq!(
        st_unresolved["incoming"]["diagnostics"][0]["action"],
        "Append a valid 'retained: {\"version\":1,...}' successor note if needed and a 'retention resolution: ...' note via 'tasks note' binding bad.basis and bad.ordinal"
    );

    // Host filter `--host host-b` skips host-a spaceless note when resolved/localized to host-a.
    let st_other_host = env.json(
        &clean,
        &["retained", "state", &cln_focal, "--host", "host-b"],
    );
    assert_eq!(st_other_host["incoming"]["complete"], true);
    assert_eq!(st_other_host["incoming"]["items"], json!([]));

    // Directly committed uncovered standalone resolution (`successors: []`) fails closed even under `--host host-b`!
    let pre_tamper = String::from_utf8(
        git_fixture::command(env.home.path(), &clean)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let bad_basis = env.json(&clean, &["retained", "state", &h_sp])["basis"].clone();
    let show = env.json(&clean, &["show", &h_sp]);
    let bad_ordinal = show["task"]["notes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|n| n["text"] == spaceless)
        .unwrap() as u64
        + 1;
    let uncovered_res = json!({
        "version": 1,
        "bad": {"basis": bad_basis, "ordinal": bad_ordinal},
        "successors": [],
        "reason": "Uncovered committed resolution"
    });
    seed_raw_note(
        &clean,
        &h_sp,
        &format!("retention resolution: {uncovered_res}"),
    );
    for args in [
        vec!["retained", "state", cln_focal.as_str()],
        vec!["retained", "state", cln_focal.as_str(), "--host", "host-b"],
    ] {
        let st_uncov = env.json(&clean, &args);
        assert_eq!(
            st_uncov["incoming"]["complete"], false,
            "uncovered resolution must fail closed under {args:?}: {st_uncov}"
        );
    }

    // Reset tampered resolution and append valid released successor + standalone `retention resolution:` via `tasks note`.
    git(&clean, &["reset", "--hard", &pre_tamper]);
    env.json(
        &clean,
        &[
            "note",
            &h_sp,
            &format!("retained: {}", released_worktree("host-a", ".worktrees/sp")),
        ],
    );
    let succ_ordinal = bad_ordinal + 1;
    let ok_res = json!({
        "version": 1,
        "bad": {"basis": bad_basis, "ordinal": bad_ordinal},
        "successors": [succ_ordinal],
        "reason": "Repaired historical spaceless retained note"
    });
    env.json(
        &clean,
        &["note", &h_sp, &format!("retention resolution: {ok_res}")],
    );

    for args in [
        vec!["retained", "state", cln_focal.as_str()],
        vec!["retained", "state", cln_focal.as_str(), "--host", "host-a"],
        vec!["retained", "state", cln_focal.as_str(), "--host", "host-b"],
    ] {
        let st_cleared = env.json(&clean, &args);
        assert_eq!(
            st_cleared["incoming"]["complete"], true,
            "{args:?}: {st_cleared}"
        );
        assert_eq!(
            st_cleared["incoming"]["items"],
            json!([]),
            "{args:?}: {st_cleared}"
        );
        assert_eq!(
            st_cleared["incoming"]["diagnostics"],
            json!([]),
            "{args:?}: {st_cleared}"
        );
    }
}

#[test]
fn staged_only_index_handoff_and_working_tree_dirty_holder_fail_closed() {
    use std::io::Write;
    use std::process::Stdio;

    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let sci = init_git_project(&env, &root, "sci", "sci");
    let focal = env.json(&sci, &["add", "Focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let holder = env.json(&sci, &["add", "Non-candidate holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let arc_holder = env.json(&sci, &["add", "Archived clean holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    for idx in 0..5 {
        env.json(
            &sci,
            &[
                "note",
                &arc_holder,
                &format!(
                    "Clean archived note {idx} with enough padding bytes to enable lossless archive-notes"
                ),
            ],
        );
    }
    let preview = env.json(&sci, &["archive-notes", &arc_holder]);
    let expect_sha = preview["source_sha256"].as_str().unwrap().to_string();
    env.json(
        &sci,
        &["archive-notes", &arc_holder, "--expect", &expect_sha],
    );

    let clean_commit = String::from_utf8(
        git_fixture::command(env.home.path(), &sci)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    let holder_path = sci.join(format!("tasks/{holder}.md"));
    let orig_holder_raw = std::fs::read_to_string(&holder_path).unwrap();

    // 1. Staged-only `:0:` index handoff when BOTH `B_P` and working tree on disk lack the pointer!
    let mut staged_raw = orig_holder_raw.clone();
    staged_raw.push_str(&format!(
        "- 2026-11-18T00:00:01Z (fixture): retained: {}\n",
        kept_worktree("host-a", ".worktrees/staged-only", &format!("task:{focal}"))
    ));
    std::fs::write(&holder_path, &staged_raw).unwrap();
    git(&sci, &["add", &format!("tasks/{holder}.md")]);
    std::fs::write(&holder_path, &orig_holder_raw).unwrap();

    let st_staged = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_staged["incoming"]["complete"], false, "{st_staged}");
    assert!(
        st_staged["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_stale_basis" && d["holder"] == holder),
        "{st_staged}"
    );

    // 2. Unmerged index stage (`1..=3`) on `<holder>.md` -> `retained_stale_basis`.
    git(&sci, &["reset", "--hard", &clean_commit]);
    let head_blob = String::from_utf8(
        git_fixture::command(env.home.path(), &sci)
            .args(["rev-parse", &format!("HEAD:tasks/{holder}.md")])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let mut upd = git_fixture::command(env.home.path(), &sci)
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        upd.stdin.as_mut().unwrap(),
        "000000 0000000000000000000000000000000000000000 0\ttasks/{holder}.md\n100644 {head_blob} 2\ttasks/{holder}.md"
    )
    .unwrap();
    assert!(upd.wait().unwrap().success());

    let st_unmerged = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_unmerged["incoming"]["complete"], false, "{st_unmerged}");
    assert!(
        st_unmerged["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_stale_basis" && d["holder"] == holder),
        "{st_unmerged}"
    );

    // 3. Working-tree dirty edit introducing `<focal>`'s suffix (unstaged) -> `retained_stale_basis`.
    git(&sci, &["reset", "--hard", &clean_commit]);
    std::fs::write(&holder_path, &staged_raw).unwrap();
    let st_wt = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_wt["incoming"]["complete"], false, "{st_wt}");
    assert!(
        st_wt["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_stale_basis" && d["holder"] == holder),
        "{st_wt}"
    );

    // 4. Working-tree corrupt `.notes` archive on non-candidate task -> `retained_stale_basis`.
    git(&sci, &["reset", "--hard", &clean_commit]);
    let arc_dir = sci.join(format!("tasks/files/{arc_holder}"));
    let arc_file = std::fs::read_dir(&arc_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "notes"))
        .unwrap();
    std::fs::write(&arc_file, b"corrupted working-tree archive bytes\n").unwrap();
    let st_wt_arc = env.json(&sci, &["retained", "state", &focal]);
    assert_eq!(st_wt_arc["incoming"]["complete"], false, "{st_wt_arc}");
    assert!(
        st_wt_arc["incoming"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "retained_stale_basis" && d["holder"] == arc_holder),
        "{st_wt_arc}"
    );
}

#[test]
fn recheck_ignores_untracked_ordinary_attachments_on_focal_holder_and_unrelated_tasks() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let sci = init_git_project(&env, &root, "sci", "sci");
    let focal = env.json(&sci, &["add", "Focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let holder = env.json(&sci, &["add", "Verified incoming holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let other = env.json(&sci, &["add", "Unrelated task"])["id"]
        .as_str()
        .unwrap()
        .to_string();

    env.json(
        &sci,
        &[
            "note",
            &holder,
            &format!(
                "retained: {}",
                kept_worktree("host-a", ".worktrees/h-clean", &format!("task:{focal}"))
            ),
        ],
    );

    for (tid, filename) in [
        (&focal, "draft.json"),
        (&holder, "note.txt"),
        (&other, "img.png"),
    ] {
        let dir = sci.join(format!("tasks/files/{tid}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(filename), b"ordinary untracked payload").unwrap();
    }

    for extra in [vec![], vec!["--all-projects"]] {
        let mut args = vec!["retained", "state", focal.as_str()];
        args.extend(extra.clone());
        let st = env.json(&sci, &args);
        assert_eq!(st["complete"], true, "{extra:?}: {st}");
        assert_eq!(st["incoming"]["complete"], true, "{extra:?}: {st}");
        assert_eq!(st["incoming"]["items"].as_array().unwrap().len(), 1);
        assert_eq!(st["incoming"]["items"][0]["holder"], holder);
        assert_eq!(st["incoming"]["diagnostics"], json!([]), "{extra:?}: {st}");
    }
}

#[test]
fn all_projects_shared_detached_git_root_isolates_per_project_subdirs() {
    let env = TestEnv::new();
    let ws = tempfile::tempdir().unwrap();
    let root = ws.path().canonicalize().unwrap();

    let shared_records = root.join("records");
    std::fs::create_dir_all(&shared_records).unwrap();
    git(&shared_records, &["init", "-q", "-b", "main"]);
    git_fixture::assert_repo(env.home.path(), &shared_records);
    std::fs::write(
        shared_records.join("README.md"),
        "Shared detached records\n",
    )
    .unwrap();
    git(&shared_records, &["add", "README.md"]);
    git(&shared_records, &["commit", "-qm", "seed shared records"]);

    let init_detached = |name: &str, prefix: &str| {
        let checkout = root.join(name);
        std::fs::create_dir_all(&checkout).unwrap();
        git(&checkout, &["init", "-q", "-b", "main"]);
        git_fixture::assert_repo(env.home.path(), &checkout);
        env.json(
            &checkout,
            &[
                "init",
                "--prefix",
                prefix,
                "--records",
                shared_records.to_str().unwrap(),
            ],
        );
        let cfg = shared_records.join(format!("{prefix}/tasks/.config.toml"));
        let mut cfg_val: toml::Value =
            toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        cfg_val.as_table_mut().unwrap().insert(
            "git".into(),
            toml::Value::Table(toml::map::Map::from_iter([(
                "commit".into(),
                toml::Value::Boolean(true),
            )])),
        );
        std::fs::write(&cfg, toml::to_string(&cfg_val).unwrap()).unwrap();
        git(&shared_records, &["add", "."]);
        git(
            &shared_records,
            &["commit", "-qm", &format!("init detached {prefix}")],
        );
        checkout
    };

    let alpha = init_detached("alpha", "alp");
    let beta = init_detached("beta", "bet");
    let gamma = init_detached("gamma", "gam");

    let focal = env.json(&alpha, &["add", "Alpha focal"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_bet_clean = env.json(&beta, &["add", "Beta fresh holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();
    let h_gam_verified = env.json(&gamma, &["add", "Gamma verified holder"])["id"]
        .as_str()
        .unwrap()
        .to_string();

    env.json(
        &gamma,
        &[
            "note",
            &h_gam_verified,
            &format!(
                "retained: {}",
                kept_worktree("host-a", "/work/gamma-item", &format!("task:{focal}"))
            ),
        ],
    );

    // Dirty uncommitted handoff in `beta` (`records/bet/tasks/<h_bet_clean>.md`) only!
    let bet_md = shared_records.join(format!("bet/tasks/{h_bet_clean}.md"));
    let mut bet_raw = std::fs::read_to_string(&bet_md).unwrap();
    bet_raw.push_str(&format!(
        "- 2026-11-18T00:00:01Z (fixture): retained: {}\n",
        kept_worktree("host-a", "/work/beta-dirty", &format!("task:{focal}"))
    ));
    std::fs::write(&bet_md, bet_raw).unwrap();

    let st = env.json(&alpha, &["retained", "state", &focal, "--all-projects"]);
    assert_eq!(
        st["complete"], true,
        "alpha top-level state must stay complete: {st}"
    );
    assert_eq!(st["incoming"]["complete"], false, "{st}");
    assert_eq!(st["incoming"]["items"].as_array().unwrap().len(), 1, "{st}");
    assert_eq!(st["incoming"]["items"][0]["holder"], h_gam_verified);

    let diags = st["incoming"]["diagnostics"].as_array().unwrap();
    assert!(
        diags
            .iter()
            .all(|d| d["project"] == "bet" && d["code"] == "retained_stale_basis"),
        "only beta should report stale diagnostics: {st}"
    );
    assert!(diags.iter().any(|d| d["holder"] == h_bet_clean), "{st}");
}
