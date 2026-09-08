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

fn probe_screen(args: &[&str]) -> bool {
    let home = scratch();
    let cwd = scratch();
    let output = lbx()
        .args(args)
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .output()
        .unwrap_or_else(|error| panic!("failed to run lunchbox {:?} probe: {error}", args));
    !String::from_utf8_lossy(&output.stderr).contains("no TUI screens")
}

fn doctor_screens_available() -> bool {
    static AVAILABLE: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| probe_screen(&["tui", "doctor", "--json"]));
    *AVAILABLE
}

fn menu_screens_available() -> bool {
    static AVAILABLE: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| probe_screen(&["tui", "policy", "--json"]));
    *AVAILABLE
}


#[test]
fn tui_doctor_json_twin_matches_doctor_json() {
    let home = fake_tree_home();
    if !doctor_screens_available() { return; }
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
    if !doctor_screens_available() { return; }
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
    if !doctor_screens_available() { return; }
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
    if !menu_screens_available() { return; }
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
    if !menu_screens_available() { return; }
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
    if !menu_screens_available() { return; }
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
fn feature_016_pinless_start_fails_closed() {
    for args in [
        vec!["start"],
        vec!["start", "--json"],
        vec!["start", "--dry-run"],
        vec!["start", "--adapter", "none"],
    ] {
        let home = scratch();
        let cwd = scratch();
        lbx()
            .args(&args)
            .env("HOME", home.path())
            .current_dir(cwd.path())
            .assert()
            .failure()
            .stderr(predicates::str::contains("no skills pinned"));
        assert!(
            !home.path().join(".lunchbox").exists(),
            "{args:?} must not create a run dir"
        );
    }
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
    assert_eq!(
        report["workers"],
        serde_json::json!([{"name": "default", "menu_tokens": 27}])
    );
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

fn project_scan_config(cwd: &Path, scan_command: &str) {
    fs::write(
        cwd.join("lunchbox.toml"),
        format!("scan_command = '''{scan_command}'''\n"),
    )
    .unwrap();
}

#[test]
fn scan_pass_invokes_scanner_and_locks() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = cwd.path().join("scan-record.txt");
    project_scan_config(cwd.path(), &format!("printf '%s' > {}", record.display()));
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
    let run_id = report["run_id"].as_str().unwrap().to_string();
    let recorded = fs::read_to_string(&record).unwrap();
    assert_eq!(
        recorded,
        skills.join("demo-review").display().to_string(),
        "scanner must receive the pantry source path as its final argument"
    );
    let run = only_run(home.path());
    let lock: Value = toml::from_str(&fs::read_to_string(run.join("lunchbox.lock")).unwrap()).unwrap();
    assert_eq!(lock["skills"][0]["scan"], serde_json::json!("pass"));
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    let scan: Value = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .find(|event: &Value| event["event"] == "scan")
        .expect("audit must contain a scan event");
    assert!(scan["command"].as_str().unwrap().contains("printf '%s'"));
    assert_eq!(
        scan["skills"],
        serde_json::json!([{"name": "demo-review", "scan": "pass"}])
    );
    assert_eq!(scan["override"], serde_json::json!(false));
    lbx()
        .args(["finish", &run_id])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
}

#[test]
fn scan_fail_fails_closed() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    project_scan_config(cwd.path(), "echo findings >&2; exit 3");
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
        .failure()
        .stderr(predicates::str::contains("demo-review"))
        .stderr(predicates::str::contains(
            "scan_command 'echo findings >&2; exit 3'",
        ))
        .stderr(predicates::str::contains("exit 3"))
        .stderr(predicates::str::contains("findings"));
    let runs: Vec<_> = fs::read_dir(runs_dir(home.path()))
        .map(|entries| entries.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    assert!(runs.is_empty(), "failed scan must leave no run dir: {runs:?}");
}

#[test]
fn scan_override_proceeds_and_audits() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    project_scan_config(cwd.path(), "echo findings >&2; exit 3");
    let output = lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "none",
            "--override-scan",
            "--json",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("warning: skill 'demo-review' failed scan_command"),
        "{stderr}"
    );
    assert!(stderr.contains("overridden by --override-scan"), "{stderr}");
    let run = only_run(home.path());
    let lock: Value = toml::from_str(&fs::read_to_string(run.join("lunchbox.lock")).unwrap()).unwrap();
    assert_eq!(lock["skills"][0]["scan"], serde_json::json!("overridden"));
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    let scan: Value = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .find(|event: &Value| event["event"] == "scan")
        .expect("audit must contain a scan event");
    assert_eq!(
        scan["skills"],
        serde_json::json!([{"name": "demo-review", "scan": "overridden"}])
    );
    assert_eq!(scan["override"], serde_json::json!(true));
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!run.join("workdir").exists());
}

#[test]
fn scan_json_stays_pure() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    project_scan_config(cwd.path(), "echo junk");
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
    let report: Value = serde_json::from_slice(&output)
        .expect("start --json stdout must be exactly one parseable JSON object");
    assert!(report.get("run_id").is_some());
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
}

fn two_worker_manifest(dir: &Path, adapter: &str, reviewer_description: bool) -> PathBuf {
    let path = dir.join("m.toml");
    let description = if reviewer_description {
        "description = \"Review specialist\"\n"
    } else {
        ""
    };
    fs::write(
        &path,
        format!(
            "schema = 1\ntask = \"path b check\"\nadapter = \"{adapter}\"\n\n[[workers]]\nname = \"parent\"\npack = [\"demo-review\"]\n\n[[workers]]\nname = \"reviewer\"\n{description}pack = [\"demo-scan\"]\n"
        ),
    )
    .unwrap();
    path
}

#[test]
fn from_manifest_mounts_per_worker_packs() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let manifest = two_worker_manifest(cwd.path(), "none", true);
    let output = lbx()
        .args([
            "start",
            "--from",
            manifest.to_str().unwrap(),
            "--library",
            skills.to_str().unwrap(),
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
    let run_id = report["run_id"].as_str().unwrap().to_string();
    assert_eq!(
        report["workers"],
        serde_json::json!([
            {"name": "parent", "menu_tokens": 14},
            {"name": "reviewer", "menu_tokens": 13},
        ])
    );
    let run = only_run(home.path());
    let workdir = run.join("workdir");
    assert!(workdir.join("packs").join("parent").join("demo-review").is_dir());
    assert!(workdir.join("packs").join("reviewer").join("demo-scan").is_dir());
    assert!(!workdir.join("demo-review").exists());
    assert!(!workdir.join("demo-scan").exists());
    let lock = fs::read_to_string(run.join("lunchbox.lock")).unwrap();
    let lock: Value = toml::from_str(&lock).unwrap();
    let skills_by_name: std::collections::HashMap<&str, &Value> = lock["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].as_str().unwrap(), s))
        .collect();
    assert_eq!(
        skills_by_name["demo-review"]["workers"],
        serde_json::json!(["parent"])
    );
    assert_eq!(
        skills_by_name["demo-scan"]["workers"],
        serde_json::json!(["reviewer"])
    );
    assert_eq!(lock["workdir"].as_str().unwrap(), workdir.to_str().unwrap());

    let why = lbx()
        .args(["why"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let why = String::from_utf8(why).unwrap();
    assert!(why.contains("worker parent: demo-review"), "{why}");
    assert!(why.contains("worker reviewer: demo-scan"), "{why}");

    lbx()
        .args(["finish", &run_id])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!workdir.exists());
    assert!(!run.join("agents").exists());
    let result: Value =
        serde_json::from_str(&fs::read_to_string(run.join("result.json")).unwrap()).unwrap();
    assert_eq!(result["unmounted"], serde_json::json!(true));
}

#[test]
fn from_manifest_rejects_invalid() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let unused = cwd.path().join("unused.toml").to_string_lossy().into_owned();
    let cases: Vec<(&str, String, &str)> = vec![
        (
            "schema 2",
            "schema = 2\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n".to_string(),
            "manifest schema 2 not supported (expected 1)",
        ),
        (
            "unknown key",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\nbogus = 1\n\n[[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n".to_string(),
            "failed to parse manifest",
        ),
        (
            "duplicate worker",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n\n[[workers]]\nname = \"w\"\npack = [\"demo-scan\"]\n".to_string(),
            "duplicate worker name 'w' in manifest",
        ),
        (
            "empty pack",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"w\"\npack = []\n".to_string(),
            "worker 'w' has an empty pack",
        ),
        (
            "no workers",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n".to_string(),
            "manifest has no workers",
        ),
        (
            "worker name path escape",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"../../../.pi/agent/agents/backdoor\"\npack = [\"demo-review\"]\n".to_string(),
            "worker name '../../../.pi/agent/agents/backdoor' must be a single path component",
        ),
        (
            "worker name with slash",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"a/b\"\npack = [\"demo-review\"]\n".to_string(),
            "worker name 'a/b' must be a single path component",
        ),
        (
            "worker-level unknown key",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\n[[workers]]\nname = \"w\"\ndescritpion = \"typo\"\npack = [\"demo-review\"]\n".to_string(),
            "failed to parse manifest",
        ),
    ];
    for (label, body, needle) in cases {
        let path = cwd.path().join(format!("{label}.toml"));
        fs::write(&path, body).unwrap();
        lbx()
            .args([
                "start",
                "--from",
                path.to_str().unwrap(),
                "--library",
                skills.to_str().unwrap(),
                "--adapter",
                "none",
            ])
            .env("HOME", home.path())
            .current_dir(cwd.path())
            .assert()
            .failure()
            .stderr(predicates::str::contains(needle));
    }
    lbx()
        .args([
            "start",
            "--from",
            &unused,
            "--skill",
            "demo-review",
        ])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "--skill and --from are mutually exclusive",
        ));
    let runs: Vec<_> = fs::read_dir(runs_dir(home.path()))
        .map(|entries| entries.filter_map(|e| e.ok()).collect())
        .unwrap_or_default();
    assert!(runs.is_empty(), "invalid manifests must leave no run dir: {runs:?}");
}

#[test]
fn from_manifest_spawn_uses_parent_pack() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_pi(cwd.path());
    let path_env = path_with_mock_bin(cwd.path());
    let manifest = two_worker_manifest(cwd.path(), "pi", false);
    lbx()
        .args([
            "start",
            "--from",
            manifest.to_str().unwrap(),
            "--library",
            skills.to_str().unwrap(),
            "--adapter",
            "pi",
            "--no-wait",
            "--",
            "pi",
            "-p",
            "hi",
        ])
        .env("HOME", home.path())
        .env("PATH", &path_env)
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
    let argv: Vec<&str> = spawn["argv"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let expected_skill = run.join("workdir").join("packs").join("parent").join("demo-review");
    assert_eq!(
        argv,
        vec![
            "pi",
            "--no-skills",
            "--skill",
            expected_skill.to_str().unwrap(),
            "pi",
            "-p",
            "hi",
        ],
        "{argv:?}"
    );
    let recorded = read_spawn_record(&record);
    assert!(recorded.contains("--no-skills"), "{}", recorded);

    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!run.join("workdir").exists());
}

#[test]
fn from_manifest_pi_prints_agents() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_pi(cwd.path());
    let path_env = path_with_mock_bin(cwd.path());
    let manifest = two_worker_manifest(cwd.path(), "pi", true);
    let output = lbx()
        .args([
            "start",
            "--from",
            manifest.to_str().unwrap(),
            "--library",
            skills.to_str().unwrap(),
            "--adapter",
            "pi",
            "--dry-run",
            "--",
            "pi",
            "-p",
            "hi",
        ])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("agents         1 run-local (printed; not auto-loaded)"), "{text}");
    assert!(text.contains("note           pi-subagents discovers agents only from ~/.pi/agent/agents"), "{text}");
    assert!(text.contains("--no-skills"), "{text}");
    assert!(!record.exists(), "dry-run must not spawn");

    let run = only_run(home.path());
    let run_id = run.file_name().unwrap().to_string_lossy().into_owned();
    let agent = fs::read_to_string(run.join("agents").join("reviewer.md")).unwrap();
    let expected_pack = run.join("workdir").join("packs").join("reviewer");
    assert!(agent.starts_with("---\nname: reviewer\ndescription: Review specialist\ninheritSkills: false\n"), "{agent}");
    assert!(agent.contains(&format!("skillPath: {}", expected_pack.display())), "{agent}");
    assert!(agent.contains("skills: demo-scan\n"), "{agent}");
    assert!(agent.contains("tools: read, grep, find, bash"), "{agent}");
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    let agents_event: Value = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .find(|event: &Value| event["event"] == "agents")
        .expect("agents event present");
    assert_eq!(agents_event["adapter"], serde_json::json!("pi"));
    assert_eq!(agents_event["files"], serde_json::json!(["reviewer.md"]));
    assert_eq!(agents_event["loaded"], serde_json::json!(false));

    lbx()
        .args(["finish", &run_id])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!run.join("agents").exists());
    assert!(!run.join("workdir").exists());
}

#[test]
fn omp_manifest_overlay_and_agents() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_omp(cwd.path());
    let path_env = path_with_mock_bin(cwd.path());
    let manifest = two_worker_manifest(cwd.path(), "omp", false);
    let output = lbx()
        .args([
            "start",
            "--from",
            manifest.to_str().unwrap(),
            "--library",
            skills.to_str().unwrap(),
            "--adapter",
            "omp",
            "--no-wait",
            "--",
            "omp",
            "-p",
            "hello",
        ])
        .env("HOME", home.path())
        .env("PATH", &path_env)
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("agents         1 run-local (printed; not auto-loaded)"), "{text}");
    assert!(text.contains("note           omp discovers task agents only from ~/.omp/agent/agents"), "{text}");

    let run = only_run(home.path());
    let overlay = fs::read_to_string(run.join("omp-config.yml")).unwrap();
    assert!(overlay.contains("customDirectories"), "{overlay}");
    assert!(
        overlay.contains(run.join("workdir").join("packs").join("parent").to_str().unwrap()),
        "{overlay}"
    );
    assert!(!overlay.contains("agents:"), "print mode keeps the overlay skills-only: {overlay}");
    let agent = fs::read_to_string(run.join("agents").join("reviewer.md")).unwrap();
    assert!(agent.starts_with("---\nname: reviewer\ndescription: Lunchbox run-local agent for worker reviewer\n"), "{agent}");
    assert!(agent.contains("tools:"), "{agent}");
    assert!(
        agent.contains(run.join("workdir").join("packs").join("reviewer").to_str().unwrap()),
        "{agent}"
    );
    let audit = fs::read_to_string(run.join("audit.jsonl")).unwrap();
    let agents_event: Value = audit
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .find(|event: &Value| event["event"] == "agents")
        .expect("agents event present");
    assert_eq!(agents_event["adapter"], serde_json::json!("omp"));
    assert_eq!(agents_event["loaded"], serde_json::json!(false));
    let recorded = read_spawn_record(&record);
    assert!(recorded.contains("--config"), "{}", recorded);

    lbx()
        .args(["abort"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
    assert!(!run.join("workdir").exists());
    assert!(!run.join("agents").exists());
}

#[test]
fn omp_spawn_records_audit_and_overlay() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_omp(cwd.path());
    let path_env = path_with_mock_bin(cwd.path());

    lbx()
        .args([
            "start",
            "--library",
            skills.to_str().unwrap(),
            "--skill",
            "demo-review",
            "--adapter",
            "omp",
            "--no-wait",
            "--",
            "omp",
            "-p",
            "hello",
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
    let overlay_path = run.join("omp-config.yml").to_string_lossy().into_owned();
    assert_eq!(
        argv.iter().map(|v| v.as_str().unwrap()).collect::<Vec<_>>(),
        vec!["omp", "--config", overlay_path.as_str(), "omp", "-p", "hello"],
        "{argv:?}"
    );
    let overlay = fs::read_to_string(run.join("omp-config.yml")).unwrap();
    assert!(overlay.contains("customDirectories"), "{overlay}");
    assert!(
        overlay.contains(run.join("workdir").to_str().unwrap()),
        "{overlay}"
    );
    let recorded = read_spawn_record(&record);
    assert!(recorded.contains("--config"), "{}", recorded);

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
}

#[test]
fn omp_selftest_skips_when_binary_absent() {
    let home = scratch();
    let cwd = scratch();
    let bare = std::env::join_paths([
        cwd.path().join("empty-bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
    ])
    .unwrap();
    fs::create_dir_all(cwd.path().join("empty-bin")).unwrap();
    lbx()
        .args(["adapters"])
        .env("HOME", home.path())
        .env("PATH", bare)
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("omp"))
        .stdout(predicates::str::contains("skipped"));
}

fn path_with_mock_bin(cwd: &Path) -> std::ffi::OsString {
    std::env::join_paths(
        std::iter::once(cwd.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
    )
    .unwrap()
}

fn read_spawn_record(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(text) = fs::read_to_string(path) {
            if !text.is_empty() {
                return text;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    fs::read_to_string(path).expect("spawn record was never written")
}

fn mock_harness(scratch_dir: &Path, binary: &str) -> PathBuf {
    let bin = scratch_dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = scratch_dir.join("argv-record");
    fs::write(
        bin.join(binary),
        format!(
            "#!/bin/sh\nexec >/dev/null 2>&1 </dev/null\nprintf '%s\\n' \"$@\" > {}\nexec sleep 60\n",
            script.display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(bin.join(binary), fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn mock_pi(scratch_dir: &Path) -> PathBuf {
    mock_harness(scratch_dir, "pi")
}

fn mock_omp(scratch_dir: &Path) -> PathBuf {
    mock_harness(scratch_dir, "omp")
}

#[test]
fn pi_spawn_records_audit_and_aborts() {
    let home = scratch();
    let skills = demo_skills();
    let cwd = scratch();
    let record = mock_pi(cwd.path());
    let path_env = path_with_mock_bin(cwd.path());

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
    let recorded = read_spawn_record(&record);
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
    let path_env = path_with_mock_bin(cwd.path());
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
    let path_env = path_with_mock_bin(cwd.path());
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
    let path_env = path_with_mock_bin(cwd.path());
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

fn mock_git(scratch_dir: &Path) {
    let bin = scratch_dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join("git"),
        r#"#!/bin/sh
case "$1" in
  --version)
    echo "git version 2.99.0-mock"
    exit 0
    ;;
  clone)
    url="$2"; dest="$3"
    case "$url" in
      *clonefail*)
        echo "remote: repository not found" >&2
        exit 1
        ;;
      *nested*|*agent-skills*)
        mkdir -p "$dest/skills/alpha" "$dest/skills/beta" "$dest/.git"
        printf -- '---\nname: alpha\ndescription: a\n---\n' > "$dest/skills/alpha/SKILL.md"
        printf -- '---\nname: beta\ndescription: b\n---\n' > "$dest/skills/beta/SKILL.md"
        ;;
      *rootshaped*)
        mkdir -p "$dest/alpha" "$dest/.git"
        printf -- '---\nname: alpha\ndescription: a\n---\n' > "$dest/alpha/SKILL.md"
        ;;
      *ambiguous*)
        mkdir -p "$dest/alpha" "$dest/skills/beta" "$dest/.git"
        printf -- '---\nname: alpha\ndescription: a\n---\n' > "$dest/alpha/SKILL.md"
        printf -- '---\nname: beta\ndescription: b\n---\n' > "$dest/skills/beta/SKILL.md"
        ;;
      *dupname*)
        mkdir -p "$dest/skills/alpha" "$dest/skills/beta" "$dest/.git"
        printf -- '---\nname: same\ndescription: a\n---\n' > "$dest/skills/alpha/SKILL.md"
        printf -- '---\nname: same\ndescription: b\n---\n' > "$dest/skills/beta/SKILL.md"
        ;;
    esac
    exit 0
    ;;
  -C)
    [ "$3" = pull ] && [ "$4" = --ff-only ] || {
      echo "unexpected git invocation: $*" >&2
      exit 1
    }
    case "$2" in
      *divergent*)
        echo "fatal: Not possible to fast-forward" >&2
        exit 128
        ;;
    esac
    exit 0
    ;;
esac
echo "unexpected git invocation: $*" >&2
exit 1
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(bin.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
}

fn pantry_dir(home: &Path, name: &str) -> PathBuf {
    home.join(".lunchbox").join("pantry").join(name)
}

#[test]
fn add_registers_pantry_and_start_resolves() {
    let home = scratch();
    let cwd = scratch();
    mock_git(cwd.path());
    lbx()
        .args(["add", "https://example.com/you/agent-skills"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("added          agent-skills"))
        .stdout(predicates::str::contains("pantry root"))
        .stdout(predicates::str::contains(
            pantry_dir(home.path(), "agent-skills")
                .join("skills")
                .display()
                .to_string(),
        ))
        .stdout(predicates::str::contains("skills         2"));

    let output = lbx()
        .args(["doctor", "--json"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["git"], serde_json::json!("2.99.0"));
    let pantries = report["pantries"].as_array().unwrap();
    assert_eq!(pantries.len(), 1);
    assert_eq!(pantries[0]["name"], serde_json::json!("agent-skills"));
    assert_eq!(pantries[0]["skills"], serde_json::json!(2));
    assert_eq!(
        report["menu_tokens"],
        serde_json::json!(0),
        "managed pantries must not enter the without estimate"
    );

    let output = lbx()
        .args(["start", "--skill", "alpha", "--adapter", "none", "--json"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(scratch().path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let run: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(
        run["without_menu_tokens"],
        serde_json::json!(0),
        "adapter none sees no standing dirs"
    );
    assert_eq!(run["skills"].as_array().unwrap().len(), 1);
    assert_eq!(run["skills"][0]["name"], serde_json::json!("alpha"));
    lbx()
        .args(["finish"])
        .env("HOME", home.path())
        .current_dir(cwd.path())
        .assert()
        .success();
}

#[test]
fn add_root_shaped_and_path_override() {
    let home = scratch();
    let cwd = scratch();
    mock_git(cwd.path());
    lbx()
        .args(["add", "https://example.com/you/rootshaped"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            pantry_dir(home.path(), "rootshaped").display().to_string(),
        ));

    lbx()
        .args(["add", "https://example.com/you/ambiguous", "--path", "skills"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            pantry_dir(home.path(), "ambiguous")
                .join("skills")
                .display()
                .to_string(),
        ));
    assert_eq!(
        fs::read_to_string(
            home.path()
                .join(".lunchbox")
                .join("pantry")
                .join("ambiguous.path")
        )
        .unwrap()
        .trim(),
        "skills"
    );

    lbx()
        .args(["update"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("updated        ambiguous"))
        .stdout(predicates::str::contains("updated        rootshaped"));
}

#[test]
fn add_failures_fail_closed() {
    let home = scratch();
    let cwd = scratch();
    mock_git(cwd.path());
    lbx()
        .args(["add", "https://example.com/you/ambiguous"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("multiple candidate skill roots"))
        .stderr(predicates::str::contains("skills"));
    assert!(
        !pantry_dir(home.path(), "ambiguous").exists(),
        "failed add must remove its clone"
    );
    assert!(!home.path().join(".lunchbox").join("pantry").join("ambiguous.path").exists());

    lbx()
        .args(["add", "https://example.com/you/clonefail"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("failed to clone"))
        .stderr(predicates::str::contains("exit 1"))
        .stderr(predicates::str::contains("repository not found"));
    assert!(!pantry_dir(home.path(), "clonefail").exists());

    lbx()
        .args(["add", "https://example.com/you/dupname"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "two packages with the name 'same'",
        ));
    assert!(
        !pantry_dir(home.path(), "dupname").exists(),
        "a scan failure after a successful clone must still remove the clone"
    );

    lbx()
        .args(["add", "https://example.com/you/nested"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success();
    lbx()
        .args(["add", "https://example.com/other/nested"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "managed pantry 'nested' already exists",
        ));
    assert!(pantry_dir(home.path(), "nested").join("skills").is_dir());
}

#[test]
fn update_reports_and_fails_loudly() {
    let home = scratch();
    let cwd = scratch();
    mock_git(cwd.path());
    lbx()
        .args(["add", "https://example.com/you/nested"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success();
    lbx()
        .args(["update", "nested"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("updated        nested"));

    fs::create_dir_all(
        home.path()
            .join(".lunchbox")
            .join("pantry")
            .join("junk")
            .join("stuff"),
    )
    .unwrap();
    lbx()
        .args(["update", "nested"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("updated        nested"));
    lbx()
        .args(["update", "ghost"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("no managed pantry named 'ghost'"));

    let diverged = scratch();
    let divergent_pantry = diverged.path().join(".lunchbox").join("pantry").join("divergent");
    fs::create_dir_all(divergent_pantry.join("skills").join("alpha")).unwrap();
    fs::write(
        divergent_pantry.join("skills").join("alpha").join("SKILL.md"),
        "---\nname: alpha\ndescription: a\n---\n",
    )
    .unwrap();
    lbx()
        .args(["update", "divergent"])
        .env("HOME", diverged.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "failed to update managed pantry 'divergent'",
        ))
        .stderr(predicates::str::contains("exit 128"))
        .stderr(predicates::str::contains("Not possible to fast-forward"));
}

#[test]
fn missing_git_fails_closed() {
    let home = scratch();
    let cwd = scratch();
    fs::create_dir_all(cwd.path().join("emptybin")).unwrap();
    let empty_path =
        std::env::join_paths([cwd.path().join("emptybin")]).unwrap();
    lbx()
        .args(["add", "https://example.com/you/nested"])
        .env("HOME", home.path())
        .env("PATH", &empty_path)
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("failed to run git clone"));
    assert!(!pantry_dir(home.path(), "nested").exists());

    lbx()
        .args(["doctor"])
        .env("HOME", home.path())
        .env("PATH", &empty_path)
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("git            (not found)"));
}

#[test]
fn broken_pantry_fails_start_closed_and_doctor_reports() {
    let home = scratch();
    let cwd = scratch();
    let junk = home.path().join(".lunchbox").join("pantry").join("junk");
    fs::create_dir_all(junk.join("stuff")).unwrap();
    lbx()
        .args(["start", "--skill", "alpha", "--adapter", "none"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("managed pantry 'junk'"))
        .stderr(predicates::str::contains("no Skill packages found"));
    assert!(!runs_dir(home.path()).exists(), "no run dir left behind");

    lbx()
        .args(["doctor"])
        .env("HOME", home.path())
        .env("PATH", &path_with_mock_bin(cwd.path()))
        .current_dir(cwd.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("pantries:"))
        .stdout(predicates::str::contains("junk"))
        .stdout(predicates::str::contains("error:"));
}
