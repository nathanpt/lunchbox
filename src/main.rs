mod adapter;
mod config;
mod hash;
mod library;
mod mount;
mod pantry;
mod manifests;
mod resolve;
mod run;
mod tokens;
#[cfg(any(feature = "tui-doctor", feature = "tui-menu"))]
mod tui;

use adapter::{Adapter, SelftestOutcome};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use config::Config;
use run::{Manifest, Outcome, PreparedRun};
use serde_json::json;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::path::PathBuf;
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
    Add {
        url: String,
        #[arg(long, value_name = "SUBDIR")]
        path: Option<String>,
    },
    Update {
        name: Option<String>,
    },
    Menu {
        #[arg(long = "library", value_name = "PATH")]
        libraries: Vec<String>,
    },
}

#[derive(Args)]
struct StartArgs {
    #[arg(long)]
    task: Option<String>,
    #[arg(long = "skill", value_name = "PIN")]
    skills: Vec<String>,
    #[arg(long = "tool", value_name = "TOOL")]
    tools: Vec<String>,
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
    #[arg(long = "from", value_name = "MANIFEST")]
    from: Option<String>,
    #[arg(long)]
    override_scan: bool,
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
        CliCommand::Add { url, path } => cmd_add(&url, path.as_deref()),
        CliCommand::Update { name } => cmd_update(name.as_deref()),
        CliCommand::Menu { libraries } => cmd_menu(libraries),
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
        match &report.git_version {
            Some(version) => println!("git            {version}"),
            None => println!("git            (not found)"),
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
        if !report.pantries.is_empty() {
            println!("pantries:");
            for pantry in &report.pantries {
                println!("{}", pantry.summary_line());
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_add(url: &str, path: Option<&str>) -> Result<ExitCode> {
    let added = pantry::add(url, path)?;
    println!("added          {}", added.name);
    println!("pantry root    {}", added.root.display());
    println!("skills         {}", added.skills);
    Ok(ExitCode::SUCCESS)
}

fn cmd_update(name: Option<&str>) -> Result<ExitCode> {
    for pantry in pantry::update(name)? {
        println!("updated        {pantry}");
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
fn cmd_menu(libraries: Vec<String>) -> Result<ExitCode> {
    #[cfg(not(all(feature = "tui-doctor", feature = "tui-menu")))]
    {
        let _ = libraries;
        bail!(tui_feature_missing(
            "tui-doctor,tui-menu",
            "lunchbox doctor / lunchbox start"
        ));
    }
    #[cfg(all(feature = "tui-doctor", feature = "tui-menu"))]
    {
        use std::io::IsTerminal;
        if !std::io::stdout().is_terminal() {
            bail!("menu requires a terminal; stdout is not a TTY");
        }
        let cfg = Config::load()?;
        let libraries: Vec<PathBuf> = libraries.iter().map(PathBuf::from).collect();
        tui::menu::pantry::build_sections(&cfg, &libraries)?;
        tui::menu::run(tui::menu::MenuState::new(cfg, libraries))?;
        Ok(ExitCode::SUCCESS)
    }
}

fn cmd_start(args: StartArgs) -> Result<ExitCode> {
    if args.skills.is_empty() && args.from.is_none() {
        bail!("no skills pinned: pass --skill <name>[@sha256:<64 hex>], --from <manifest>, or use lunchbox menu");
    }
    if !args.tools.is_empty() && args.from.is_some() {
        bail!("cannot combine --tool with --from (set tools per worker in the manifest)");
    }
    let cfg = Config::load()?;
    let mut harness_argv = args.harness_argv.clone();
    if harness_argv.first().map(String::as_str) == Some("--") {
        harness_argv.remove(0);
    }
    let libraries: Vec<PathBuf> = args.libraries.iter().map(PathBuf::from).collect();
    let (adapter_name, task, max_menu_tokens, workers, use_packs) = match &args.from {
        Some(from) => {
            if !args.skills.is_empty() {
                bail!("--skill and --from are mutually exclusive; put pins in the manifest workers");
            }
            let parsed = run::read_manifest_input(&manifests::resolve_from(from)?)?;
            let input = parsed.validate()?;
            (
                args.adapter
                    .clone()
                    .unwrap_or_else(|| input.adapter.clone()),
                args.task.clone().unwrap_or_else(|| input.task.clone()),
                input
                    .budget
                    .as_ref()
                    .map(|b| b.max_menu_tokens)
                    .unwrap_or(cfg.max_menu_tokens),
                input.workers.clone(),
                true,
            )
        }
        None => {
            let mut worker = run::Worker::default_pack(args.skills.clone());
            worker.tools = (!args.tools.is_empty()).then(|| args.tools.clone());
            (
                args.adapter
                    .clone()
                    .unwrap_or_else(|| cfg.default_adapter.clone()),
                args.task.clone().unwrap_or_default(),
                cfg.max_menu_tokens,
                vec![worker],
                false,
            )
        }
    };
    let adapter = adapter::resolve_adapter(&adapter_name)?;
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let prepared = run::prepare_run(
        &cfg,
        &adapter_name,
        &task,
        workers,
        max_menu_tokens,
        &libraries,
        &harness_argv,
        use_packs,
        args.override_scan,
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

fn agent_file_names(files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect()
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
    let run::PreparedRun {
        run_id,
        run_dir,
        workdir,
        locked,
        mount_mode,
        menu_tokens,
        without_tokens,
        without_skills,
        workers,
        worker_tokens,
        scan_root,
    } = prepared;
    let run_dir = run_dir.as_path();
    let workdir = workdir.as_path();
    let scan_root = scan_root.as_path();
    let parent_skills: Vec<String> = workers[0].pack.clone();
    let tools: Vec<String> = workers[0].tools.clone().unwrap_or_default();
    let tool_estimate =
        (!tools.is_empty()).then(|| adapter::tools::estimate(&adapter_name, &tools));
    if tool_estimate.is_some() {
        let known = adapter::tools::table(&adapter_name);
        for tool in tools.iter().filter(|tool| {
            known.is_some_and(|table| !table.iter().any(|entry| entry.name == tool.as_str()))
        }) {
            eprintln!(
                "warning: no token estimate for tool '{tool}' on {adapter_name} (not a builtin?)"
            );
        }
    }

    if let Some(signal) = signals.pending().next() {
        let _ = std::fs::remove_dir_all(run_dir);
        return Ok(ExitCode::from(signal_exit_code(signal)));
    }

    let agent_files = if workers.len() > 1 {
        let specs: Vec<adapter::AgentSpec> = workers[1..]
            .iter()
            .map(|worker| adapter::AgentSpec {
                name: worker.name.clone(),
                description: worker.description.clone().unwrap_or_else(|| {
                    format!("Lunchbox run-local agent for worker {}", worker.name)
                }),
                pack_dir: run::pack_dir(workdir, &worker.name),
                skills: worker.pack.clone(),
                tools: worker.tools.clone().unwrap_or_default(),
            })
            .collect();
        let files = adapter.write_run_agents(run_dir, &specs)?;
        if !files.files.is_empty() {
            run::append_audit(
                run_dir,
                &run_id,
                json!({
                    "event": "agents",
                    "adapter": adapter_name,
                    "files": agent_file_names(&files.files),
                    "loaded": files.loaded,
                }),
            )?;
        }
        Some(files)
    } else {
        None
    };

    let pins: Vec<String> = locked
        .iter()
        .map(|s| format!("{}@{}", s.name, s.hash))
        .collect();
    if args.json {
        let mut output = json!({
            "run_id": run_id,
            "workdir": workdir,
            "skills": locked.iter().map(|s| json!({"name": s.name, "hash": s.hash})).collect::<Vec<_>>(),
            "menu_tokens": menu_tokens,
            "without_menu_tokens": without_tokens,
            "adapter": adapter_name,
            "mount_mode": mount_mode.as_str(),
            "workers": worker_tokens.iter().map(|w| json!({"name": w.name, "menu_tokens": w.menu_tokens})).collect::<Vec<_>>(),
        });
        if let Some((tokens, unestimated)) = &tool_estimate {
            output["tools"] = json!(tools);
            output["tool_tokens"] = json!(tokens);
            output["unestimated_tools"] = json!(unestimated);
        }
        if let Some(files) = &agent_files {
            output["agents"] = json!({
                "files": agent_file_names(&files.files),
                "loaded": files.loaded,
            });
        }
        println!("{output}");
    } else {
        println!("run            {run_id}");
        println!("workdir        {}", workdir.display());
        println!("skills         {}", pins.join("  "));
        println!("menu_tokens    this run: {menu_tokens}");
        if let Some((tokens, unestimated)) = &tool_estimate {
            println!("tools          {} ({})", tools.join(", "), tools.len());
            if *unestimated > 0 {
                println!("tool_tokens    this run: {tokens} (+{unestimated} unestimated)");
            } else {
                println!("tool_tokens    this run: {tokens}");
            }
        }
        if without_skills == 0 {
            println!("without        {without_tokens}  (no skills found in {adapter_name} skill dirs)");
        } else {
            println!("without        ~{without_tokens}  ({without_skills} skills on {adapter_name} global+project)");
        }
        println!("isolation      {}", adapter.isolation_summary());
        if let Some(files) = &agent_files {
            if !files.files.is_empty() {
                let state = if files.loaded {
                    format!(", loaded by {adapter_name}")
                } else {
                    " (printed; not auto-loaded)".to_string()
                };
                println!("agents         {} run-local{}", files.files.len(), state);
                if let Some(hint) = &files.include_hint {
                    println!("note           {hint}");
                }
            }
        }
        println!("unmount        run lunchbox finish {run_id}   (auto on --wait exit)");
    }

    if adapter_name == "none" {
        return Ok(ExitCode::SUCCESS);
    }

    let argv = adapter.isolation_argv(
        run_dir,
        scan_root,
        &parent_skills,
        workers[0].tools.as_deref().unwrap_or(&[]),
        harness_argv,
    )?;
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
    let menu_tokens: u64 =
        crate::tokens::with_preamble(lock.skills.iter().map(|s| s.description_tokens).sum());
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
    for name in adapter::ADAPTERS {
        let adapter = adapter::resolve_adapter(name)?;
        let version = adapter.detect()?;
        let selftest = adapter.selftest(version.as_deref());
        let version_text = match &version {
            Some(version) => version.clone(),
            None => "not found".to_string(),
        };
        let selftest = selftest?;
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
mod menu_tests {
    #[allow(unused_imports)]
    use super::*;

    #[cfg(not(all(feature = "tui-doctor", feature = "tui-menu")))]
    #[test]
    fn menu_fails_closed_without_tui_features() {
        let error = cmd_menu(vec![]).unwrap_err().to_string();
        assert!(error.contains("this build has no TUI screens"), "{error}");
    }
}
