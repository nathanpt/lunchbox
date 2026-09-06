mod adapter;
mod config;
mod hash;
mod library;
mod mount;
mod resolve;
mod run;
mod tokens;
#[cfg(any(feature = "tui-doctor", feature = "tui-menu"))]
mod tui;

use adapter::{Adapter, SelftestOutcome};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use config::Config;
use resolve::Locked;
use run::{Manifest, Outcome};
use serde_json::json;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "lunchbox",
    version,
    about = "Hand an agent only the Skill packages needed for this job, then take them back."
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    Doctor {
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Start(StartArgs),
    Status {
        run_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Finish {
        run_id: Option<String>,
    },
    Abort {
        run_id: Option<String>,
    },
    Gc,
    Why {
        run_id: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Adapters {
        #[arg(long)]
        explain: bool,
    },
    Tui {
        #[command(subcommand)]
        screen: TuiScreen,
    },
}

#[derive(Subcommand)]
enum TuiScreen {
    Doctor {
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Preview {
        #[arg(long = "skill", value_name = "PIN")]
        skills: Vec<String>,
        #[arg(long = "library", value_name = "PATH")]
        libraries: Vec<String>,
        #[arg(long)]
        adapter: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Picker {
        #[arg(long = "library", value_name = "PATH")]
        libraries: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    Policy {
        #[arg(long)]
        json: bool,
    },
}
#[derive(Args)]
struct StartArgs {
    #[arg(long)]
    task: Option<String>,
    #[arg(long = "skill", value_name = "PIN")]
    skills: Vec<String>,
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long = "library", value_name = "PATH")]
    libraries: Vec<String>,
    #[arg(long)]
    wait: bool,
    #[arg(long)]
    no_wait: bool,
    #[arg(long)]
    keep: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long = "from", value_name = "PATH")]
    from: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    harness_argv: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        CliCommand::Doctor { adapter, json } => cmd_doctor(adapter.as_deref(), json),
        CliCommand::Start(args) => cmd_start(args),
        CliCommand::Status { run_id, json } => cmd_status(run_id.as_deref(), json),
        CliCommand::Finish { run_id } => cmd_finish(run_id.as_deref()),
        CliCommand::Abort { run_id } => cmd_abort(run_id.as_deref()),
        CliCommand::Gc => cmd_gc(),
        CliCommand::Why { run_id, json } => cmd_why(run_id.as_deref(), json),
        CliCommand::Adapters { explain } => cmd_adapters(explain),
        CliCommand::Tui { screen } => cmd_tui(screen),
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn doctor_data(adapter_override: Option<&str>) -> Result<adapter::DoctorReport> {
    let cfg = Config::load()?;
    let adapter_name = adapter_override.unwrap_or(&cfg.default_adapter);
    let adapter = adapter::resolve_adapter(adapter_name)?;
    adapter::doctor_report(adapter.as_ref(), &cfg)
}

fn cmd_doctor(adapter_override: Option<&str>, json: bool) -> Result<ExitCode> {
    let report = doctor_data(adapter_override)?;
    if json {
        println!("{}", report.to_json());
    } else {
        let fattest = report.fattest();
        let duplicates = adapter::duplicates(&report.union);
        match &report.adapter_version {
            Some(version) => println!("adapter        {} {version}", report.adapter),
            None => println!("adapter        {} (not detected)", report.adapter),
        }
        println!("skill dirs:");
        for dir_report in &report.dirs {
            let state = if dir_report.exists {
                format!("{} skills", dir_report.skills.len())
            } else {
                "absent".to_string()
            };
            println!("  {:<40} {}", dir_report.dir.display(), state);
        }
        println!(
            "menu_tokens    {}  ({} skills union)",
            report.menu_tokens,
            report.union.len()
        );
        if report.union.is_empty() {
            println!("               no skills found in adapter skill dirs");
        }
        println!("fattest:");
        for skill in &fattest {
            println!("  {:<16} {}", skill.name, skill.tokens);
        }
        if duplicates.is_empty() {
            println!("duplicates     none");
        } else {
            println!("duplicates:");
            for group in &duplicates {
                let dirs: Vec<String> =
                    group.iter().map(|s| s.dir.display().to_string()).collect();
                let name = group.first().map(|s| s.name.as_str()).unwrap_or_default();
                println!("  {name}  in {}", dirs.join(", "));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(not(all(feature = "tui-doctor", feature = "tui-menu")))]
fn tui_feature_missing(feature: &str, plain_equivalent: &str) -> String {
    if cfg!(any(feature = "tui-doctor", feature = "tui-menu")) {
        format!(
            "this build has no TUI screens for feature '{feature}'; rebuild with default \
             features or --features {feature} (plain CLI equivalent: {plain_equivalent})"
        )
    } else {
        "this build has no TUI screens; rebuild with default features or \
         --features tui-doctor,tui-menu (plain CLI equivalents: lunchbox doctor, lunchbox start)"
            .to_string()
    }
}
fn cmd_tui(screen: TuiScreen) -> Result<ExitCode> {
    match screen {
        TuiScreen::Doctor { adapter, json } => tui_doctor(adapter.as_deref(), json),
        TuiScreen::Preview {
            skills,
            libraries,
            adapter,
            json,
        } => tui_preview(skills, libraries, adapter, json),
        TuiScreen::Picker { libraries, json } => tui_picker(libraries, json),
        TuiScreen::Policy { json } => tui_policy(json),
    }
}

fn tui_doctor(adapter_override: Option<&str>, json: bool) -> Result<ExitCode> {
    #[cfg(not(feature = "tui-doctor"))]
    {
        let _ = (adapter_override, json);
        bail!(tui_feature_missing("tui-doctor", "lunchbox doctor"));
    }
    #[cfg(feature = "tui-doctor")]
    {
        let report = doctor_data(adapter_override)?;
        if json {
            println!("{}", report.to_json());
            Ok(ExitCode::SUCCESS)
        } else {
            crate::tui::doctor::run(report)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn tui_preview(
    skills: Vec<String>,
    libraries: Vec<String>,
    adapter: Option<String>,
    json: bool,
) -> Result<ExitCode> {
    #[cfg(not(feature = "tui-doctor"))]
    {
        let _ = (skills, libraries, adapter, json);
        bail!(tui_feature_missing("tui-doctor", "lunchbox doctor"));
    }
    #[cfg(feature = "tui-doctor")]
    {
        let mut cfg = Config::load()?;
        if let Some(adapter) = adapter {
            cfg.default_adapter = adapter;
        }
        let libraries: Vec<PathBuf> = libraries.iter().map(PathBuf::from).collect();
        let preview = tokens::preview(&cfg, &libraries, &skills)?;
        if json {
            let output = json!({
                "skills": preview.skills.iter().map(|s| json!({"name": s.name, "tokens": s.tokens})).collect::<Vec<_>>(),
                "menu_tokens": preview.menu_tokens,
                "without_menu_tokens": preview.without_menu_tokens,
                "max_menu_tokens": preview.max_menu_tokens,
                "over_budget": preview.over_budget,
            });
            println!("{output}");
            Ok(ExitCode::SUCCESS)
        } else {
            let listing = library::scan_roots(&cfg.search_roots(&libraries))?;
            let mut state = tui::preview::PreviewState::new(
                listing,
                preview.without_menu_tokens,
                preview.max_menu_tokens,
            );
            let pinned: Vec<String> = preview.skills.iter().map(|s| s.name.clone()).collect();
            state.preselect(&pinned);
            tui::preview::run(state)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn tui_picker(libraries: Vec<String>, json: bool) -> Result<ExitCode> {
    #[cfg(not(feature = "tui-menu"))]
    {
        let _ = (libraries, json);
        bail!(tui_feature_missing("tui-menu", "lunchbox start"));
    }
    #[cfg(feature = "tui-menu")]
    {
        let cfg = Config::load()?;
        let libraries: Vec<PathBuf> = libraries.iter().map(PathBuf::from).collect();
        let listing = library::scan_roots(&cfg.search_roots(&libraries))?;
        if json {
            let output = json!({
                "library": listing.iter().map(|s| json!({
                    "name": s.name,
                    "tokens": s.tokens,
                    "source": s.source,
                })).collect::<Vec<_>>(),
            });
            println!("{output}");
            Ok(ExitCode::SUCCESS)
        } else {
            let state = tui::picker::PickerState::new(cfg, libraries, listing);
            tui::picker::run(state)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn tui_policy(json: bool) -> Result<ExitCode> {
    #[cfg(not(feature = "tui-menu"))]
    {
        let _ = json;
        bail!(tui_feature_missing("tui-menu", "lunchbox policy"));
    }
    #[cfg(feature = "tui-menu")]
    {
        if json {
            let global = config::layer_report(&config::global_path()?)?;
            let project = config::layer_report(&config::project_path())?;
            let cfg = Config::load()?;
            let output = json!({
                "global": {"path": global.path, "exists": global.exists, "allow": global.allow, "deny": global.deny},
                "project": {"path": project.path, "exists": project.exists, "allow": project.allow, "deny": project.deny},
                "effective": {"allow": cfg.allow, "deny": cfg.deny},
            });
            println!("{output}");
        } else {
            let state = tui::policy::PolicyState::load()?;
            tui::policy::run(state)?;
        }
        Ok(ExitCode::SUCCESS)
    }
}

fn cmd_start(args: StartArgs) -> Result<ExitCode> {
    if let Some(_from) = &args.from {
        bail!("manifest-driven multi-worker runs arrive with Path B");
    }
    let cfg = Config::load()?;
    let adapter_name = args
        .adapter
        .clone()
        .unwrap_or_else(|| cfg.default_adapter.clone());
    let adapter = adapter::resolve_adapter(&adapter_name)?;
    let mut harness_argv = args.harness_argv.clone();
    if harness_argv.first().map(String::as_str) == Some("--") {
        harness_argv.remove(0);
    }
    let libraries: Vec<PathBuf> = args.libraries.iter().map(PathBuf::from).collect();
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let prepared = prepare_run(
        &cfg,
        &adapter_name,
        args.task.as_deref().unwrap_or(""),
        &args.skills,
        &libraries,
        &harness_argv,
    )?;
    let run_dir = prepared.run_dir.clone();
    let outcome = start_run(
        &args,
        &cfg,
        adapter.as_ref(),
        prepared,
        &harness_argv,
        &mut signals,
    );
    match outcome {
        Ok(code) => Ok(code),
        Err(error) => {
            if run_dir.exists() {
                let _ = std::fs::remove_dir_all(&run_dir);
            }
            Err(error)
        }
    }
}

pub struct PreparedRun {
    pub run_id: String,
    pub run_dir: PathBuf,
    pub workdir: PathBuf,
    pub locked: Vec<Locked>,
    pub mount_mode: config::MountMode,
    pub menu_tokens: u64,
    pub without_tokens: u64,
    pub without_skills: usize,
}

pub fn prepare_run(
    cfg: &Config,
    adapter_name: &str,
    task: &str,
    pins: &[String],
    libraries: &[PathBuf],
    harness_argv: &[String],
) -> Result<PreparedRun> {
    let locked = resolve::resolve(pins, cfg, libraries)?;
    let run_id = run::new_run_id()?;
    let run_dir = cfg.runs_dir.join(&run_id);
    let prepared = mount_run(cfg, adapter_name, task, &locked, &run_id, &run_dir, harness_argv);
    match prepared {
        Ok(prepared) => Ok(prepared),
        Err(error) => {
            if run_dir.exists() {
                let _ = std::fs::remove_dir_all(&run_dir);
            }
            Err(error)
        }
    }
}

fn mount_run(
    cfg: &Config,
    adapter_name: &str,
    task: &str,
    locked: &[Locked],
    run_id: &str,
    run_dir: &Path,
    harness_argv: &[String],
) -> Result<PreparedRun> {
    let workdir = run_dir.join("workdir");
    std::fs::create_dir_all(run_dir)
        .with_context(|| format!("failed to create run dir {}", run_dir.display()))?;
    let skill_names: Vec<String> = locked.iter().map(|s| s.name.clone()).collect();
    let manifest = run::build_manifest(
        run_id,
        task,
        adapter_name,
        harness_argv,
        cfg.max_menu_tokens,
        &skill_names,
    );
    run::write_manifest(run_dir, &manifest)?;
    let mount_mode = mount::mount(locked, &workdir, cfg.mount_mode)?;
    let lock = run::build_lock(run_id, mount_mode.as_str(), &workdir, locked);
    run::write_lock(run_dir, &lock)?;
    let adapter = adapter::resolve_adapter(adapter_name)?;
    let (without_tokens, without_skills) = adapter::union_menu(adapter.as_ref(), cfg);
    let menu_tokens: u64 = locked.iter().map(|s| s.description_tokens).sum();
    run::append_audit(
        run_dir,
        run_id,
        json!({
            "event": "resolved",
            "skills": locked.iter().map(|s| json!({"name": s.name, "hash": s.hash})).collect::<Vec<_>>(),
        }),
    )?;
    run::append_audit(
        run_dir,
        run_id,
        json!({"event": "mounted", "mode": mount_mode.as_str(), "workdir": workdir}),
    )?;
    Ok(PreparedRun {
        run_id: run_id.to_string(),
        run_dir: run_dir.to_path_buf(),
        workdir,
        locked: locked.to_vec(),
        mount_mode,
        menu_tokens,
        without_tokens,
        without_skills: without_skills.len(),
    })
}

fn start_run(
    args: &StartArgs,
    cfg: &Config,
    adapter: &dyn Adapter,
    prepared: PreparedRun,
    harness_argv: &[String],
    signals: &mut Signals,
) -> Result<ExitCode> {
    let adapter_name = adapter.name();
    let PreparedRun {
        run_id,
        run_dir,
        workdir,
        locked,
        mount_mode,
        menu_tokens,
        without_tokens,
        without_skills,
    } = prepared;
    let run_dir = run_dir.as_path();
    let workdir = workdir.as_path();
    let skill_names: Vec<String> = locked.iter().map(|s| s.name.clone()).collect();

    if let Some(signal) = signals.pending().next() {
        let _ = std::fs::remove_dir_all(run_dir);
        return Ok(ExitCode::from(signal_exit_code(signal)));
    }

    let pins: Vec<String> = locked
        .iter()
        .map(|s| format!("{}@{}", s.name, s.hash))
        .collect();
    if args.json {
        let output = json!({
            "run_id": run_id,
            "workdir": workdir,
            "skills": locked.iter().map(|s| json!({"name": s.name, "hash": s.hash})).collect::<Vec<_>>(),
            "menu_tokens": menu_tokens,
            "without_menu_tokens": without_tokens,
            "adapter": adapter_name,
            "mount_mode": mount_mode.as_str(),
        });
        println!("{output}");
    } else {
        println!("run            {run_id}");
        println!("workdir        {}", workdir.display());
        println!("skills         {}", pins.join("  "));
        println!("menu_tokens    this run: {menu_tokens}");
        if without_skills == 0 {
            println!("without        {without_tokens}  (no skills found in {adapter_name} skill dirs)");
        } else {
            println!("without        ~{without_tokens}  ({without_skills} skills on {adapter_name} global+project)");
        }
        if adapter_name == "none" {
            println!("isolation      adapter none — mounted, not spawned");
        } else {
            println!("isolation      path A flags applied: --no-skills --skill <workdir>");
        }
        println!("unmount        run lunchbox finish {run_id}   (auto on --wait exit)");
    }

    if adapter_name == "none" {
        return Ok(ExitCode::SUCCESS);
    }

    let argv = adapter.isolation_argv(workdir, &skill_names, harness_argv)?;
    if args.dry_run {
        for token in &argv {
            println!("{}", shell_quote(token));
        }
        println!("run lunchbox finish {run_id}");
        return Ok(ExitCode::SUCCESS);
    }

    run::append_audit(
        run_dir,
        &run_id,
        json!({"event": "spawn", "adapter": adapter_name, "argv": argv}),
    )?;
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .spawn()
        .with_context(|| format!("failed to spawn {}", argv[0]))?;
    let child_pid = child.id();
    run::write_pid(run_dir, child_pid)?;

    let wait = args.wait || (!args.no_wait && !harness_argv.is_empty());
    if !wait {
        println!("pid            {child_pid}");
        println!("unmount        run lunchbox finish {run_id}");
        return Ok(ExitCode::SUCCESS);
    }

    let exit = loop {
        if let Some(signal) = signals.pending().next() {
            break WaitOutcome::Signaled(signal);
        }
        match child.try_wait()? {
            Some(status) => break WaitOutcome::Exited(status.code()),
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    match exit {
        WaitOutcome::Exited(code) => {
            if !args.keep {
                run::teardown(run_dir, Outcome::Ok, cfg)?;
            }
            Ok(ExitCode::from(code.unwrap_or(1).clamp(0, 255) as u8))
        }
        WaitOutcome::Signaled(signal) => {
            run::signal_pid(child_pid, "TERM");
            let _ = child.wait();
            if !args.keep {
                run::teardown(run_dir, Outcome::Aborted, cfg)?;
            }
            Ok(ExitCode::from(signal_exit_code(signal)))
        }
    }
}

fn signal_exit_code(signal: i32) -> u8 {
    if signal == SIGINT {
        130
    } else {
        143
    }
}

enum WaitOutcome {
    Exited(Option<i32>),
    Signaled(i32),
}

fn shell_quote(token: &str) -> String {
    if token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c))
        && !token.is_empty()
    {
        token.to_string()
    } else {
        format!("'{}'", token.replace('\'', "'\\''"))
    }
}

fn cmd_status(run_id: Option<&str>, json: bool) -> Result<ExitCode> {
    let cfg = Config::load()?;
    let run_dir = run::resolve_run_arg(&cfg.runs_dir, run_id)?;
    let status = run::run_status(&run_dir);
    let id = run_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if json {
        println!("{}", json!({"run_id": id, "status": status.as_str()}));
    } else {
        println!("{id}  {}", status.as_str());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_finish(run_id: Option<&str>) -> Result<ExitCode> {
    let cfg = Config::load()?;
    let run_dir = run::resolve_run_arg(&cfg.runs_dir, run_id)?;
    if run::read_result(&run_dir)?.is_none() {
        if let Some(pid) = run::read_pid(&run_dir) {
            if run::pid_alive(pid) {
                bail!("run is still running; abort first");
            }
        }
    }
    run::teardown(&run_dir, Outcome::Ok, &cfg)?;
    println!(
        "finished {}  (workdir removed, result.json written)",
        run_dir.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn cmd_abort(run_id: Option<&str>) -> Result<ExitCode> {
    let cfg = Config::load()?;
    let run_dir = run::resolve_run_arg(&cfg.runs_dir, run_id)?;
    if run::read_result(&run_dir)?.is_none() {
        if let Some(pid) = run::read_pid(&run_dir) {
            if run::pid_alive(pid) {
                run::signal_pid(pid, "TERM");
                if !run::wait_pid_exit(pid, Duration::from_secs(5)) {
                    run::signal_pid(pid, "KILL");
                    let _ = run::wait_pid_exit(pid, Duration::from_secs(5));
                }
            }
        }
    }
    run::teardown(&run_dir, Outcome::Aborted, &cfg)?;
    println!("aborted {}  (workdir removed, result.json written)", run_dir.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_gc() -> Result<ExitCode> {
    let cfg = Config::load()?;
    let removed = run::gc(&cfg.runs_dir, Duration::from_secs(24 * 3600))?;
    if removed.is_empty() {
        println!("nothing to collect");
    } else {
        for dir in removed {
            println!("removed {}", dir.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_why(run_id: Option<&str>, json: bool) -> Result<ExitCode> {
    let cfg = Config::load()?;
    let run_dir = run::resolve_run_arg(&cfg.runs_dir, run_id)?;
    let manifest: Manifest = run::read_manifest(&run_dir)?;
    let lock = run::read_lock(&run_dir)?;
    let result = run::read_result(&run_dir)?;
    let menu_tokens: u64 = lock.skills.iter().map(|s| s.description_tokens).sum();
    let adapter = adapter::resolve_adapter(&manifest.adapter)?;
    let (without_tokens, _) = adapter::union_menu(adapter.as_ref(), &cfg);
    let unmounted = !run_dir.join("workdir").exists() && result.is_some();
    let outcome = result
        .as_ref()
        .map(|r| r.outcome.clone())
        .unwrap_or_else(|| "in flight".to_string());
    let packs: Vec<String> = manifest
        .workers
        .iter()
        .map(|w| format!("{}: {}", w.name, w.pack.join(", ")))
        .collect();
    if json {
        println!(
            "{}",
            json!({
                "run_id": manifest.run_id,
                "task": manifest.task,
                "packs": packs,
                "menu_tokens": menu_tokens,
                "without_menu_tokens": without_tokens,
                "unmounted": unmounted,
                "outcome": outcome,
            })
        );
    } else {
        println!("run       {}  task \"{}\"", manifest.run_id, manifest.task);
        for pack in &packs {
            println!("packs     worker {pack}");
        }
        println!(
            "tokens    this run {menu_tokens} vs without ~{without_tokens} (delta {})",
            without_tokens as i64 - menu_tokens as i64
        );
        println!("unmounted {}", if unmounted { "yes" } else { "no" });
        println!("outcome   {outcome}");
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_adapters(explain: bool) -> Result<ExitCode> {
    let mut failure = None;
    for name in ["none", "pi"] {
        let adapter = adapter::resolve_adapter(name)?;
        let version = adapter.detect()?;
        let selftest = adapter.selftest()?;
        let version_text = match &version {
            Some(version) => version.clone(),
            None => "not found".to_string(),
        };
        let selftest_text = match &selftest {
            SelftestOutcome::Ok => "selftest: ok".to_string(),
            SelftestOutcome::Skipped => format!("{name} not found; selftest skipped"),
            SelftestOutcome::Failed(reason) => {
                failure = Some(reason.clone());
                format!("selftest: FAILED — {reason}")
            }
        };
        println!("{name:<8} {version_text:<12} {selftest_text}");
        if explain {
            println!("{}", adapter.explain());
            if let Some(hint) = adapter.agent_dir_hint() {
                println!("standing agent dir (never written): {}", hint.display());
            }
            println!();
        }
    }
    if let Some(reason) = failure {
        bail!("{reason}");
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tui_tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(not(feature = "tui-doctor"))]
    #[test]
    fn doctor_and_preview_fail_closed_without_tui_doctor() {
        let error = cmd_tui(TuiScreen::Doctor {
            adapter: None,
            json: true,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("this build has no TUI screens"), "{error}");
        let error = cmd_tui(TuiScreen::Preview {
            skills: vec![],
            libraries: vec![],
            adapter: None,
            json: true,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("this build has no TUI screens"), "{error}");
    }

    #[cfg(not(feature = "tui-menu"))]
    #[test]
    fn picker_and_policy_fail_closed_without_tui_menu() {
        let error = cmd_tui(TuiScreen::Picker {
            libraries: vec![],
            json: true,
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("this build has no TUI screens"), "{error}");
        let error = cmd_tui(TuiScreen::Policy { json: true })
            .unwrap_err()
            .to_string();
        assert!(error.contains("this build has no TUI screens"), "{error}");
    }
}
