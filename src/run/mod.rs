use anyhow::{Context, Result, bail};
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use time::OffsetDateTime;
use time::macros::format_description;

const ISO8601: &[time::format_description::FormatItem] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]Z");
const COMPACT_DATE: &[time::format_description::FormatItem] =
    format_description!("[year][month][day]");
const COMPACT_TIME: &[time::format_description::FormatItem] =
    format_description!("[hour][minute][second]");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub run_id: String,
    pub task: String,
    pub adapter: String,
    pub created_at: String,
    pub harness_argv: Vec<String>,
    pub budget: Budget,
    pub workers: Vec<Worker>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub max_menu_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    pub name: String,
    pub pack: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lock {
    pub schema: u32,
    pub run_id: String,
    pub resolved_at: String,
    pub mount_mode: String,
    pub workdir: PathBuf,
    pub skills: Vec<LockSkill>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockSkill {
    pub name: String,
    pub source: PathBuf,
    pub hash: String,
    pub scan: String,
    pub description_tokens: u64,
    pub workers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Aborted,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Aborted => "aborted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Mounted,
    Running,
    Finished,
    Aborted,
    Leaked,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Mounted => "mounted",
            RunStatus::Running => "running",
            RunStatus::Finished => "finished",
            RunStatus::Aborted => "aborted",
            RunStatus::Leaked => "leaked",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub run_id: String,
    pub outcome: String,
    pub menu_tokens: u64,
    pub without_menu_tokens: u64,
    pub skills: Vec<ResultSkill>,
    pub unmounted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultSkill {
    pub name: String,
    pub hash: String,
}

pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&ISO8601)
        .expect("static format description is valid")
}

pub fn new_run_id() -> Result<String> {
    let now = OffsetDateTime::now_utc();
    let date = now
        .format(&COMPACT_DATE)
        .expect("static format description is valid");
    let time = now
        .format(&COMPACT_TIME)
        .expect("static format description is valid");
    let mut bytes = [0u8; 2];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .context("failed to read /dev/urandom")?;
    Ok(format!("lbx_{}_{}_{}", date, time, hex::encode(bytes)))
}

pub fn build_manifest(
    run_id: &str,
    task: &str,
    adapter: &str,
    harness_argv: &[String],
    max_menu_tokens: u64,
    skill_names: &[String],
) -> Manifest {
    Manifest {
        schema: 1,
        run_id: run_id.to_string(),
        task: task.to_string(),
        adapter: adapter.to_string(),
        created_at: now_iso(),
        harness_argv: harness_argv.to_vec(),
        budget: Budget { max_menu_tokens },
        workers: vec![Worker {
            name: "default".to_string(),
            pack: skill_names.to_vec(),
        }],
    }
}

pub fn build_lock(
    run_id: &str,
    mount_mode: &str,
    workdir: &Path,
    locked: &[crate::resolve::Locked],
) -> Lock {
    Lock {
        schema: 1,
        run_id: run_id.to_string(),
        resolved_at: now_iso(),
        mount_mode: mount_mode.to_string(),
        workdir: workdir.to_path_buf(),
        skills: locked
            .iter()
            .map(|skill| LockSkill {
                name: skill.name.clone(),
                source: skill.source.clone(),
                hash: skill.hash.clone(),
                scan: "pass".to_string(),
                description_tokens: skill.description_tokens,
                workers: vec!["default".to_string()],
            })
            .collect(),
    }
}

pub fn write_manifest(run_dir: &Path, manifest: &Manifest) -> Result<()> {
    write_toml(&run_dir.join("manifest.toml"), manifest)
}

pub fn read_manifest(run_dir: &Path) -> Result<Manifest> {
    read_toml(&run_dir.join("manifest.toml"))
}

pub fn write_lock(run_dir: &Path, lock: &Lock) -> Result<()> {
    write_toml(&run_dir.join("lunchbox.lock"), lock)
}

pub fn read_lock(run_dir: &Path) -> Result<Lock> {
    read_toml(&run_dir.join("lunchbox.lock"))
}

fn write_toml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = toml::to_string(value)
        .with_context(|| format!("failed to serialize {}", path.display()))?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn append_audit(run_dir: &Path, run_id: &str, mut event: Value) -> Result<()> {
    if let Value::Object(map) = &mut event {
        map.insert("ts".to_string(), json!(now_iso()));
        map.insert("run_id".to_string(), json!(run_id));
    }
    let path = run_dir.join("audit.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{event}").with_context(|| format!("failed to append {}", path.display()))
}

pub fn write_pid(run_dir: &Path, pid: u32) -> Result<()> {
    fs::write(run_dir.join("pid"), pid.to_string())
        .with_context(|| format!("failed to write pid for run dir {}", run_dir.display()))
}

pub fn read_pid(run_dir: &Path) -> Option<u32> {
    fs::read_to_string(run_dir.join("pid"))
        .ok()
        .and_then(|text| text.trim().parse().ok())
}

pub fn pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn signal_pid(pid: u32, signal: &str) -> bool {
    Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn wait_pid_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !pid_alive(pid)
}

pub fn read_result(run_dir: &Path) -> Result<Option<RunResult>> {
    let path = run_dir.join("result.json");
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(Some(
        serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {}", path.display()))?,
    ))
}

pub fn run_status(run_dir: &Path) -> RunStatus {
    if let Ok(Some(result)) = read_result(run_dir) {
        return match result.outcome.as_str() {
            "aborted" => RunStatus::Aborted,
            _ => RunStatus::Finished,
        };
    }
    if let Some(pid) = read_pid(run_dir) {
        return if pid_alive(pid) {
            RunStatus::Running
        } else {
            RunStatus::Leaked
        };
    }
    if run_dir.join("workdir").exists() {
        return RunStatus::Mounted;
    }
    RunStatus::Leaked
}

pub fn run_dirs(runs_dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(runs_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .map(|n| n.to_string_lossy().starts_with("lbx_"))
                            .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

pub fn latest_run(runs_dir: &Path) -> Option<PathBuf> {
    run_dirs(runs_dir).pop()
}

pub fn resolve_run_arg(runs_dir: &Path, run_id: Option<&str>) -> Result<PathBuf> {
    match run_id {
        Some(id) => {
            let dir = runs_dir.join(id);
            if dir.is_dir() {
                Ok(dir)
            } else {
                bail!("run '{}' not found under {}", id, runs_dir.display())
            }
        }
        None => latest_run(runs_dir).ok_or_else(|| {
            anyhow::anyhow!("no runs found under {}", runs_dir.display())
        }),
    }
}

pub fn teardown(run_dir: &Path, outcome: Outcome, cfg: &crate::config::Config) -> Result<()> {
    let lock_path = run_dir.join(".lock");
    let guard = File::create(&lock_path)
        .with_context(|| format!("failed to open {}", lock_path.display()))?;
    guard
        .try_lock_exclusive()
        .with_context(|| "another lunchbox command is operating on this run".to_string())?;
    if run_dir.join("result.json").exists() {
        return Ok(());
    }
    let lock = read_lock(run_dir).ok();
    let manifest = read_manifest(run_dir).ok();
    let run_id = manifest
        .as_ref()
        .map(|m| m.run_id.clone())
        .or_else(|| {
            run_dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let workdir = run_dir.join("workdir");
    if workdir.exists() {
        fs::remove_dir_all(&workdir)
            .with_context(|| format!("failed to remove {}", workdir.display()))?;
    }
    let agents = run_dir.join("agents");
    if agents.exists() {
        fs::remove_dir_all(&agents)
            .with_context(|| format!("failed to remove {}", agents.display()))?;
    }
    let reason = match outcome {
        Outcome::Ok => "finish",
        Outcome::Aborted => "abort",
    };
    append_audit(
        run_dir,
        &run_id,
        json!({"event": "unmounted", "reason": reason}),
    )?;
    let menu_tokens = lock
        .as_ref()
        .map(|l| l.skills.iter().map(|s| s.description_tokens).sum())
        .unwrap_or(0);
    let without_menu_tokens = manifest
        .as_ref()
        .map(|m| crate::adapter::without_estimate(&m.adapter, cfg))
        .unwrap_or(0);
    let result = RunResult {
        run_id,
        outcome: outcome.as_str().to_string(),
        menu_tokens,
        without_menu_tokens,
        skills: lock
            .map(|l| {
                l.skills
                    .into_iter()
                    .map(|s| ResultSkill {
                        name: s.name,
                        hash: s.hash,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        unmounted: true,
    };
    let path = run_dir.join("result.json");
    fs::write(&path, serde_json::to_string_pretty(&result)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn gc(runs_dir: &Path, older_than: Duration) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    for dir in run_dirs(runs_dir) {
        let status = run_status(&dir);
        if status == RunStatus::Running {
            continue;
        }
        let stale = fs::metadata(&dir)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| mtime.elapsed().ok())
            .map(|age| age >= older_than)
            .unwrap_or(false);
        if status == RunStatus::Leaked || stale {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("failed to remove {}", dir.display()))?;
            removed.push(dir);
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::Locked;
    use tempfile::TempDir;

    fn locked_pair(dir: &TempDir) -> Vec<Locked> {
        let mk = |name: &str, tokens: u64| {
            let package = dir.path().join(name);
            fs::create_dir_all(&package).unwrap();
            fs::write(
                package.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: d\n---\n"),
            )
            .unwrap();
            Locked {
                name: name.to_string(),
                source: package,
                hash: format!("sha256:{name}"),
                description_tokens: tokens,
            }
        };
        vec![mk("alpha", 14), mk("beta", 13)]
    }

    #[test]
    fn run_id_shape() {
        let id = new_run_id().unwrap();
        assert!(id.starts_with("lbx_"), "{id}");
        let parts: Vec<&str> = id.split('_').collect();
        assert_eq!(parts.len(), 4, "{id}");
        assert_eq!(parts[1].len(), 8, "{id}");
        assert_eq!(parts[2].len(), 6, "{id}");
        assert_eq!(parts[3].len(), 4, "{id}");
        assert!(parts[3].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{id}");
    }

    #[test]
    fn iso_timestamp_shape() {
        let ts = now_iso();
        assert_eq!(ts.len(), 20, "{ts}");
        assert!(ts.ends_with('Z'), "{ts}");
    }

    #[test]
    fn manifest_roundtrip_and_schema() {
        let dir = TempDir::new().unwrap();
        let manifest = build_manifest(
            "lbx_x",
            "task text",
            "none",
            &["pi".to_string(), "-p".to_string()],
            2000,
            &["alpha".to_string(), "beta".to_string()],
        );
        write_manifest(dir.path(), &manifest).unwrap();
        let text = fs::read_to_string(dir.path().join("manifest.toml")).unwrap();
        assert!(text.contains("schema = 1"));
        assert!(text.contains("[[workers]]"));
        assert!(text.contains("name = \"default\""));
        assert!(text.contains("[budget]"));
        let back = read_manifest(dir.path()).unwrap();
        assert_eq!(back, manifest);
        assert_eq!(back.workers[0].pack, vec!["alpha", "beta"]);
    }

    #[test]
    fn lock_roundtrip_and_schema() {
        let dir = TempDir::new().unwrap();
        let locked = locked_pair(&dir);
        let workdir = dir.path().join("run").join("workdir");
        let lock = build_lock("lbx_x", "symlink", &workdir, &locked);
        let run_dir = dir.path().join("run");
        fs::create_dir_all(&run_dir).unwrap();
        write_lock(&run_dir, &lock).unwrap();
        let text = fs::read_to_string(run_dir.join("lunchbox.lock")).unwrap();
        assert!(text.contains("[[skills]]"));
        assert!(text.contains("scan = \"pass\""));
        assert!(text.contains("workers = [\"default\"]"));
        let back = read_lock(&run_dir).unwrap();
        assert_eq!(back, lock);
    }

    #[test]
    fn teardown_removes_workdir_writes_result_and_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let locked = locked_pair(&dir);
        let run_dir = dir.path().join("run");
        let workdir = run_dir.join("workdir");
        fs::create_dir_all(&workdir).unwrap();
        fs::write(workdir.join("alpha"), "x").unwrap();
        write_manifest(
            &run_dir,
            &build_manifest("lbx_x", "task", "none", &[], 2000, &["alpha".to_string()]),
        )
        .unwrap();
        write_lock(&run_dir, &build_lock("lbx_x", "symlink", &workdir, &locked)).unwrap();

        teardown(&run_dir, Outcome::Ok, &crate::config::Config::default()).unwrap();
        assert!(!workdir.exists());
        let result = read_result(&run_dir).unwrap().unwrap();
        assert_eq!(result.outcome, "ok");
        assert_eq!(result.menu_tokens, 27);
        assert!(result.unmounted);
        assert_eq!(result.skills.len(), 2);
        let audit = fs::read_to_string(run_dir.join("audit.jsonl")).unwrap();
        assert!(audit.contains("\"unmounted\""));
        assert!(audit.contains("\"reason\":\"finish\""));

        teardown(&run_dir, Outcome::Ok, &crate::config::Config::default()).unwrap();
        assert_eq!(read_result(&run_dir).unwrap().unwrap().outcome, "ok");
    }

    #[test]
    fn teardown_abort_outcome_recorded() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("run");
        fs::create_dir_all(run_dir.join("workdir")).unwrap();
        teardown(&run_dir, Outcome::Aborted, &crate::config::Config::default()).unwrap();
        let result = read_result(&run_dir).unwrap().unwrap();
        assert_eq!(result.outcome, "aborted");
    }

    #[test]
    fn status_transitions() {
        let dir = TempDir::new().unwrap();
        let run_dir = dir.path().join("lbx_a");
        fs::create_dir_all(run_dir.join("workdir")).unwrap();
        assert_eq!(run_status(&run_dir), RunStatus::Mounted);

        write_pid(&run_dir, std::process::id()).unwrap();
        assert_eq!(run_status(&run_dir), RunStatus::Running);

        let dead_pid = find_dead_pid();
        fs::write(run_dir.join("pid"), dead_pid.to_string()).unwrap();
        assert_eq!(run_status(&run_dir), RunStatus::Leaked);

        fs::remove_file(run_dir.join("pid")).unwrap();
        fs::remove_dir_all(run_dir.join("workdir")).unwrap();
        teardown(&run_dir, Outcome::Ok, &crate::config::Config::default()).unwrap();
        assert_eq!(run_status(&run_dir), RunStatus::Finished);
        teardown(&run_dir, Outcome::Aborted, &crate::config::Config::default()).unwrap();
    }

    fn find_dead_pid() -> u32 {
        let me = std::process::id();
        let mut candidate = me + 1000;
        while pid_alive(candidate) {
            candidate += 1;
        }
        candidate
    }

    #[test]
    fn gc_removes_stale_and_leaked_keeps_running() {
        let runs = TempDir::new().unwrap();
        let stale = runs.path().join("lbx_old");
        fs::create_dir_all(stale.join("workdir")).unwrap();
        filetime_back(&stale, Duration::from_secs(48 * 3600));
        let leaked = runs.path().join("lbx_leak");
        fs::create_dir_all(leaked.join("workdir")).unwrap();
        fs::write(leaked.join("pid"), find_dead_pid().to_string()).unwrap();
        let running = runs.path().join("lbx_live");
        fs::create_dir_all(running.join("workdir")).unwrap();
        write_pid(&running, std::process::id()).unwrap();

        let removed = gc(runs.path(), Duration::from_secs(24 * 3600)).unwrap();
        assert!(removed.contains(&stale), "{removed:?}");
        assert!(removed.contains(&leaked), "{removed:?}");
        assert!(!removed.contains(&running), "{removed:?}");
        assert!(!leaked.exists());
        assert!(running.exists());
        assert!(!stale.exists(), "stale dir should be removed");
    }

    fn filetime_back(path: &Path, age: Duration) {
        let seconds = (std::time::SystemTime::now() - age)
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let status = Command::new("touch")
            .arg("-d")
            .arg(format!("@{seconds}"))
            .arg(path)
            .status();
        assert!(status.unwrap().success());
    }

    #[test]
    fn latest_run_is_lexicographically_greatest() {
        let runs = TempDir::new().unwrap();
        for id in ["lbx_20260101_000000_aa", "lbx_20260102_000000_bb"] {
            fs::create_dir_all(runs.path().join(id)).unwrap();
        }
        assert_eq!(
            latest_run(runs.path()).unwrap(),
            runs.path().join("lbx_20260102_000000_bb")
        );
        assert!(resolve_run_arg(runs.path(), None).is_ok());
        assert!(resolve_run_arg(runs.path(), Some("lbx_missing")).is_err());
    }
}
