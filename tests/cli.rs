use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn lbx() -> Command {
    Command::cargo_bin("lunchbox").unwrap()
}

fn demo_skills() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/skills"))
}

fn scratch() -> TempDir {
    TempDir::new().unwrap()
}

fn runs_dir(home: &Path) -> PathBuf {
    home.join(".lunchbox").join("runs")
}

fn only_run(home: &Path) -> PathBuf {
    let mut dirs: Vec<PathBuf> = fs::read_dir(runs_dir(home))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    assert_eq!(dirs.len(), 1, "expected exactly one run dir, got {dirs:?}");
    dirs.pop().unwrap()
}

fn fake_tree_home() -> TempDir {
    let home = scratch();
    let global = home.path().join(".agents").join("skills");
    fs::create_dir_all(&global).unwrap();
    for name in ["demo-review", "demo-scan"] {
        let src = fs::read_to_string(demo_skills().join(name).join("SKILL.md")).unwrap();
        fs::create_dir_all(global.join(name)).unwrap();
        fs::write(global.join(name).join("SKILL.md"), src).unwrap();
    }
    home
}
fn twin_stdout(args: &[&str], home: &Path, cwd: &Path) -> Vec<u8> {
    lbx()
        .args(args)
        .env("HOME", home)
        .current_dir(cwd)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone()
}

fn tui_screens_available() -> bool {
    let home = scratch();
    let cwd = scratch();
    let output = lbx()
        .args(["tui", "policy", "--json"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .expect("failed to run lunchbox tui policy --json probe");
    !String::from_utf8_lossy(&output.stderr).contains("no TUI screens")
}


#[test]
fn tui_doctor_json_twin_matches_doctor_json() {
    let home = fake_tree_home();
    if !tui_screens_available() { return; }
    let cwd = scratch();
    let a = twin_stdout(&["tui", "doctor", "--json"], home.path(), cwd.path());
    let b = twin_stdout(&["doctor", "--json"], home.path(), cwd.path());
    assert_eq!(
        String::from_utf8(a).unwrap(),
        String::from_utf8(b).unwrap(),
        "tui doctor --json must be byte-identical to doctor --json"
    );
}

#[test]
fn tui_preview_json_twin_reports_estimates() {
    let home = fake_tree_home();
    if !tui_screens_available() { return; }
    let cwd = scratch();
    let output = twin_stdout(
        &[
            "tui",
            "preview",
            "--json",
            "--skill",
            "demo-review",
            "--skill",
            "demo-scan",
        ],
        home.path(),
        cwd.path(),
    );
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["menu_tokens"], serde_json::json!(27));
    assert_eq!(report["without_menu_tokens"], serde_json::json!(27));
    assert_eq!(report["max_menu_tokens"], serde_json::json!(2000));
    assert_eq!(report["over_budget"], serde_json::json!(false));
    assert_eq!(report["skills"].as_array().unwrap().len(), 2);

    let output = twin_stdout(
        &["tui", "preview", "--json", "--skill", "demo-review"],
        home.path(),
        cwd.path(),
    );
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["menu_tokens"], serde_json::json!(14));
}

#[test]
fn tui_preview_json_rejects_tag_pins_like_start() {
    let home = fake_tree_home();
    if !tui_screens_available() { return; }
    let cwd = scratch();
    lbx()
        .args(["tui", "preview", "--json", "--skill", "demo-review@latest"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("tag pins are not supported yet"));
}

#[test]
fn tui_picker_json_twin_lists_library() {
    let home = scratch();
    if !tui_screens_available() { return; }
    let cwd = scratch();
    let skills = demo_skills();
    let output = twin_stdout(
        &["tui", "picker", "--json", "--library", skills.to_str().unwrap()],
        home.path(),
        cwd.path(),
    );
    let report: Value = serde_json::from_slice(&output).unwrap();
    let library = report["library"].as_array().unwrap();
    assert_eq!(library.len(), 2);
    assert_eq!(library[0]["name"], serde_json::json!("demo-review"));
    assert_eq!(library[0]["tokens"], serde_json::json!(14));
    assert_eq!(library[1]["name"], serde_json::json!("demo-scan"));
    assert_eq!(library[1]["tokens"], serde_json::json!(13));
    let source = library[0]["source"].as_str().unwrap();
    assert!(
        source.starts_with(skills.to_str().unwrap()),
        "source should point into the library: {source}"
    );
}

#[test]
fn tui_policy_json_twin_reports_layers() {
    let home = scratch();
    if !tui_screens_available() { return; }
    let cwd = scratch();
    fs::create_dir_all(home.path().join(".lunchbox")).unwrap();
    fs::write(
        home.path().join(".lunchbox").join("config.toml"),
        "deny = [\"x\"]\n",
    )
    .unwrap();
    fs::write(cwd.path().join("lunchbox.toml"), "deny = [\"y\"]\n").unwrap();
    let output = twin_stdout(&["tui", "policy", "--json"], home.path(), cwd.path());
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["global"]["exists"], serde_json::json!(true));
    assert_eq!(report["global"]["deny"], serde_json::json!(["x"]));
    assert_eq!(report["project"]["exists"], serde_json::json!(true));
    assert_eq!(report["project"]["deny"], serde_json::json!(["y"]));
    assert_eq!(report["effective"]["deny"], serde_json::json!(["x", "y"]));
    assert_eq!(report["effective"]["allow"], serde_json::json!([]));
}

#[test]
fn feature_006_policy_edit_persists_to_project_layer_and_denies() {
    if !tui_screens_available() { return; }
    let home = scratch();
    let cwd = scratch();
    fs::write(
        cwd.path().join("lunchbox.toml"),
        "# project layer\nmount_mode = \"copy\"\n",
    )
    .unwrap();
    let before = twin_stdout(&["tui", "policy", "--json"], home.path(), cwd.path());
    let report: Value = serde_json::from_slice(&before).unwrap();
    assert_eq!(report["project"]["exists"], serde_json::json!(true));
    assert_eq!(report["effective"]["deny"], serde_json::json!([]));

    let skills = demo_skills();
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();

    config::append_deny_entry(cwd.path().join("lunchbox.toml").to_str().unwrap(), "demo-scan");

    let after = twin_stdout(&["tui", "policy", "--json"], home.path(), cwd.path());
    let report: Value = serde_json::from_slice(&after).unwrap();
    assert_eq!(report["project"]["deny"], serde_json::json!(["demo-scan"]));
    assert_eq!(report["effective"]["deny"], serde_json::json!(["demo-scan"]));

    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("denied by policy"));
    let layer = fs::read_to_string(cwd.path().join("lunchbox.toml")).unwrap();
    assert!(layer.contains("# project layer"));
    assert!(layer.contains("mount_mode = \"copy\""));
    assert!(fs::read_to_string(home.path().join(".lunchbox").join("config.toml")).is_err());
}

mod config {
    use std::fs;

    pub fn append_deny_entry(path: &str, entry: &str) {
        let text = fs::read_to_string(path).unwrap();
        let mut lines: Vec<String> = text.lines().map(String::from).collect();
        lines.push(format!("deny = [\"{entry}\"]"));
        fs::write(path, lines.join("\n") + "\n").unwrap();
    }
}

#[test]
fn feature_001_mount_unmount_loop() {
    let home = scratch();
    let skills = demo_skills();
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(scratch().path())
        .assert()
        .success()
        .stdout(predicates::str::contains("menu_tokens    this run: 27"));

    let run = only_run(home.path());
    let workdir = run.join("workdir");
    let mut mounted: Vec<String> = fs::read_dir(&workdir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    mounted.sort();
    assert_eq!(mounted, vec!["demo-review", "demo-scan"]);

    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(scratch().path())
        .assert()
        .success();
    assert!(!workdir.exists());
    let result: Value = serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap())
        .unwrap();
    assert_eq!(result["unmounted"], serde_json::json!(true));
    assert_eq!(result["outcome"], serde_json::json!("ok"));
    assert_eq!(result["menu_tokens"], serde_json::json!(27));

    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(scratch().path())
        .assert()
        .success();
}

#[test]
fn deny_gate_leaves_no_run_dir() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    fs::write(
        cwd.path().join("lunchbox.toml"),
        "deny = [\"demo-scan\"]\n",
    )
    .unwrap();
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("denied"));
    assert!(
        !runs_dir(home.path()).exists() || fs::read_dir(runs_dir(home.path())).unwrap().count() == 0
    );
}

#[test]
fn feature_002_doctor_reports_tree_without_mounting() {
    let home = scratch();
    let skills = demo_skills();
    let global = home.path().join(".agents").join("skills");
    fs::create_dir_all(&global).unwrap();
    for name in ["demo-review", "demo-scan"] {
        let src = fs::read_to_string(skills.join(name).join("SKILL.md")).unwrap();
        fs::create_dir_all(global.join(name)).unwrap();
        fs::write(global.join(name).join("SKILL.md"), src).unwrap();
    }
    let cwd = scratch();
    let output = lbx()
        .args(["doctor", "--json"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["adapter"], serde_json::json!("pi"));
    let dirs = report["skill_dirs"].as_array().unwrap();
    let home_prefix = home.path().display().to_string();
    let home_dir = dirs
        .iter()
        .find(|d| d["dir"].as_str().unwrap().starts_with(&home_prefix))
        .expect("home skills dir reported");
    assert_eq!(home_dir["skills"], serde_json::json!(2));
    assert_eq!(home_dir["exists"], serde_json::json!(true));
    assert_eq!(report["menu_tokens"], serde_json::json!(27));
    let fattest = report["fattest"].as_array().unwrap();
    assert_eq!(fattest[0]["name"], serde_json::json!("demo-review"));
    assert_eq!(fattest[0]["tokens"], serde_json::json!(14));
    assert_eq!(fattest[1]["tokens"], serde_json::json!(13));
    assert!(
        !runs_dir(home.path()).exists(),
        "doctor must not create run dirs"
    );
}

#[test]
fn start_json_reports_numbers() {
    let home = scratch();
    let skills = demo_skills();
    let output = lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
            "--json",
        ])
        .env("HOME", home.path())
        .current_dir(scratch().path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["menu_tokens"], serde_json::json!(27));
    assert_eq!(report["without_menu_tokens"], serde_json::json!(0));
    assert_eq!(report["adapter"], serde_json::json!("none"));
    assert_eq!(report["mount_mode"], serde_json::json!("symlink"));
    assert_eq!(report["skills"].as_array().unwrap().len(), 2);
}

#[test]
fn hash_pin_roundtrip_and_mismatch() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let output = lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "none",
            "--json",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();
    let hash = report["skills"][0]["hash"].as_str().unwrap().to_string();
    let run_id = report["run_id"].as_str().unwrap().to_string();

    lbx()
        .args(["finish", &run_id])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();

    let pin = format!("demo-review@{hash}");
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            &pin,
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();

    let bogus = format!("demo-review@sha256:{}", "0".repeat(64));
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            &bogus,
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("hash mismatch"));
}

#[test]
fn from_manifest_is_refused_this_phase() {
    lbx()
        .args(["start", "--from", "manifest.toml"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "manifest-driven multi-worker runs arrive with Path B",
        ));
}

#[test]
fn omp_adapter_refused_this_phase() {
    lbx()
        .args(["start", "--adapter", "omp"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("omp adapter arrives after Phase 1"));
}

fn path_with_mock_pi(cwd: &Path) -> std::ffi::OsString {
    std::env::join_paths(
        std::iter::once(cwd.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap()
}

fn mock_pi(scratch_dir: &Path) -> PathBuf {
    let bin = scratch_dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = scratch_dir.join("argv-record");
    fs::write(
        bin.join("pi"),
        format!(
            "#!/bin/sh\nexec >/dev/null 2>&1 </dev/null\nprintf '%s\\n' \"$@\" > {}\nsleep 60\n",
            script.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(bin.join("pi"), fs::Permissions::from_mode(0o755)).unwrap();
    script
}

#[test]
fn pi_spawn_records_audit_and_aborts() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_pi(cwd.path());
    let path_env = path_with_mock_pi(cwd.path());

    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "pi",
            "--no-wait",
            "--",
            "--",
            "pi",
            "--list-models",
        ])
        .env("HOME", home.path())
        .env("PATH", path_env)
        .current_dir(cwd.path())
        .assert()
        .success();

    let run = only_run(home.path());
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    let spawn: Value = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .find(|event: &Value| event["event"] == "spawn")
        .expect("spawn event present");
    let argv = spawn["argv"].as_array().unwrap();
    assert_eq!(argv[0], "pi");
    assert_eq!(argv[1], "--no-skills");
    assert_eq!(argv[2], "--skill");
    assert!(
        argv[3].as_str().unwrap().ends_with("/workdir/demo-review"),
        "{argv:?}"
    );
    assert_eq!(argv[4], "pi");
    assert_eq!(argv[5], "--list-models");
    let recorded = fs::read_to_string(&record).unwrap();
    assert!(recorded.contains("--no-skills"), "{}", recorded);
    assert!(recorded.contains("--skill"), "{}", recorded);

    let pid: u32 = fs::read_to_string(run.join("pid")).unwrap().trim().parse().unwrap();
    assert!(run::pid_alive_helper(pid));

    lbx()
        .args(["status"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("running"));

    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!run.join("workdir").exists());
    let result: Value =
        serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["outcome"], serde_json::json!("aborted"));
    assert!(!run::pid_alive_helper(pid));
}

mod run {
    pub fn pid_alive_helper(pid: u32) -> bool {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[test]
fn finish_on_live_run_errors() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let _record = mock_pi(cwd.path());
    let path_env = path_with_mock_pi(cwd.path());
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "pi",
            "--no-wait",
            "--",
            "pi",
        ])
        .env("HOME", home.path())
        .env("PATH", path_env.clone())
        .current_dir(cwd.path())
        .assert()
        .success();
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .env("PATH", path_env)
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("run is still running; abort first"));
    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
}

#[test]
fn sigint_forwards_and_aborts() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let _record = mock_pi(cwd.path());
    let path_env = path_with_mock_pi(cwd.path());
    let binary = env!("CARGO_BIN_EXE_lunchbox");
    let mut child = std::process::Command::new(binary)
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "pi",
            "--wait",
            "--",
            "pi",
        ])
        .env("HOME", home.path())
        .env("PATH", path_env)
        .current_dir(cwd.path())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(700));
    libc_free_signal(child.id() as i32);
    let status = child.wait().unwrap();
    assert_eq!(status.code(), Some(130));
    let run = only_run(home.path());
    assert!(!run.join("workdir").exists());
    let result: Value =
        serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["outcome"], serde_json::json!("aborted"));
}

fn libc_free_signal(pid: i32) {
    let kill = std::process::Command::new("kill")
        .arg("-INT")
        .arg(pid.to_string())
        .status()
        .unwrap();
    assert!(kill.success());
}

#[test]
fn leaked_run_is_collected_by_gc() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    let run = only_run(home.path());
    assert!(run.join("workdir").exists());
    let stale = std::process::Command::new("touch")
        .arg("-d")
        .arg("2 days ago")
        .arg(&run)
        .status()
        .unwrap();
    assert!(stale.success());
    lbx()
        .args(["gc"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("removed"))
        .stdout(predicates::str::contains(run.display().to_string()));
    assert!(!run.exists());
}

#[test]
fn dry_run_prints_argv_and_spawns_nothing() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_pi(cwd.path());
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "pi",
            "--dry-run",
            "--",
            "--",
            "pi",
            "--list-models",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("--no-skills"))
        .stdout(predicates::str::contains("run lunchbox finish"));
    assert!(!record.exists(), "dry-run must not spawn");
    let run = only_run(home.path());
    assert!(run.join("workdir").exists());
    assert!(!run.join("pid").exists());
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    assert!(!audit.contains("\"spawn\""));
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
}

#[test]
fn why_reports_five_line_recap() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    lbx()
        .args([
            "start",
            "--task",
            "review PR 412",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--skill",
            "demo-scan",
            "--adapter",
            "none",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    let output = lbx()
        .args(["why"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 5, "{text}");
    assert!(lines[0].contains("task \"review PR 412\""), "{text}");
    assert!(lines[1].contains("demo-review, demo-scan"), "{text}");
    assert!(lines[2].contains("this run 27"), "{text}");
    assert!(lines[3].contains("unmounted yes"), "{text}");
    assert!(lines[4].contains("outcome   ok"), "{text}");
}

#[test]
fn doctor_human_output_on_fake_tree() {
    let home = scratch();
    let skills = demo_skills();
    let global = home.path().join(".agents").join("skills");
    fs::create_dir_all(global.join("demo-review")).unwrap();
    fs::write(
        global.join("demo-review").join("SKILL.md"),
        fs::read_to_string(skills.join("demo-review").join("SKILL.md")).unwrap(),
    )
    .unwrap();
    lbx()
        .args(["doctor"])
        .env("HOME", home.path())
        .current_dir(scratch().path())
        .assert()
        .success()
        .stdout(predicates::str::contains("adapter        pi"))
        .stdout(predicates::str::contains("menu_tokens    14"))
        .stdout(predicates::str::contains("demo-review"))
        .stdout(predicates::str::contains("duplicates     none"));
}

#[test]
fn abort_on_finished_run_is_a_no_op() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let _record = mock_pi(cwd.path());
    let path_env = path_with_mock_pi(cwd.path());
    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "pi",
            "--no-wait",
            "--",
            "pi",
        ])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success();
    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success();
    let run = only_run(home.path());
    let result: Value =
        serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["outcome"], serde_json::json!("aborted"));
    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("aborted"));
    let result_again: Value =
        serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap()).unwrap();
    assert_eq!(result_again["outcome"], serde_json::json!("aborted"));
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success();
}
