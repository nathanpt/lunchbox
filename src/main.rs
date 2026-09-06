mod adapter;
mod config;
mod hash;
mod library;
mod mount;
mod resolve;
mod run;
mod tokens;

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
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::from(1)
        }
    }
}

fn cmd_doctor(adapter_override: Option<&str>, json: bool) -> Result<ExitCode> {
    let cfg = Config::load()?;
    let adapter_name = adapter_override.unwrap_or(&cfg.default_adapter);
    let adapter = adapter::resolve_adapter(adapter_name)?;
    let version = adapter.detect()?;
    let reports = adapter::scan_dirs(adapter.as_ref(), &cfg);
    let (without_tokens, union) = adapter::union_from(&reports);
    let mut fattest = union.clone();
    fattest.sort_by(|a, b| b.tokens.cmp(&a.tokens).then(a.name.cmp(&b.name)));
    fattest.truncate(3);
    let duplicates = adapter::duplicates(&union);
    if json {
        let output = json!({
            "adapter": adapter_name,
            "adapter_version": version,
            "skill_dirs": reports.iter().map(|r| json!({
                "dir": r.dir,
                "exists": r.exists,
                "skills": r.skills.len(),
            })).collect::<Vec<_>>(),
            "menu_tokens": without_tokens,
            "fattest": fattest.iter().map(|s| json!({"name": s.name, "tokens": s.tokens})).collect::<Vec<_>>(),
            "duplicates": duplicates.iter().map(|group| group.iter().map(|s| s.name.clone()).collect::<Vec<_>>()).collect::<Vec<_>>(),
        });
        println!("{output}");
    } else {
        match &version {
            Some(version) => println!("adapter        {adapter_name} {version}"),
            None => println!("adapter        {adapter_name} (not detected)"),
        }
        println!("skill dirs:");
        for report in &reports {
            let state = if report.exists {
                format!("{} skills", report.skills.len())
            } else {
                "absent".to_string()
            };
            println!("  {:<40} {}", report.dir.display(), state);
        }
        println!("menu_tokens    {without_tokens}  ({} skills union)", union.len());
        if union.is_empty() {
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
    let locked = resolve::resolve(&args.skills, &cfg, &libraries)?;
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let run_id = run::new_run_id()?;
    let run_dir = cfg.runs_dir.join(&run_id);
    let outcome = start_run(
        &args,
        &cfg,
        adapter.as_ref(),
        &run_id,
        &run_dir,
        &harness_argv,
        &locked,
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

fn start_run(
    args: &StartArgs,
    cfg: &Config,
    adapter: &dyn Adapter,
    run_id: &str,
    run_dir: &Path,
    harness_argv: &[String],
    locked: &[Locked],
    signals: &mut Signals,
) -> Result<ExitCode> {
    let adapter_name = adapter.name();
    let workdir = run_dir.join("workdir");
    let workdir = workdir.as_path();
    std::fs::create_dir_all(run_dir)
        .with_context(|| format!("failed to create run dir {}", run_dir.display()))?;
    let skill_names: Vec<String> = locked.iter().map(|s| s.name.clone()).collect();
    let manifest = run::build_manifest(
        run_id,
        args.task.as_deref().unwrap_or(""),
        adapter_name,
        harness_argv,
        cfg.max_menu_tokens,
        &skill_names,
    );
    run::write_manifest(run_dir, &manifest)?;
    let mount_mode = mount::mount(locked, workdir, cfg.mount_mode)?;
    let lock = run::build_lock(run_id, mount_mode.as_str(), workdir, locked);
    run::write_lock(run_dir, &lock)?;
    let (without_tokens, without_skills) = adapter::union_menu(adapter, cfg);
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
        if without_skills.is_empty() {
            println!("without        {without_tokens}  (no skills found in {adapter_name} skill dirs)");
        } else {
            println!("without        ~{without_tokens}  ({} skills on {adapter_name} global+project)", without_skills.len());
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
        run_id,
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
