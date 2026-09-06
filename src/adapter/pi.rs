use super::Adapter;
use crate::config::Config;
use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};

pub struct PiAdapter;

impl Adapter for PiAdapter {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn detect(&self) -> Result<Option<String>> {
        super::detect_version("pi")
    }

    fn skill_dirs(&self, _cfg: &Config) -> Vec<PathBuf> {
        let mut dirs = super::pantry_base_dirs();
        if let Some(home) = env::var_os("HOME") {
            let pi_dir = PathBuf::from(home)
                .join(".pi")
                .join("agent")
                .join("skills");
            if pi_dir.is_dir() {
                dirs.push(pi_dir);
            }
        }
        dirs
    }

    fn isolation_argv(
        &self,
        _run_dir: &Path,
        workdir: &Path,
        skills: &[String],
        user_argv: &[String],
    ) -> Result<Vec<String>> {
        let mut argv = vec!["pi".to_string(), "--no-skills".to_string()];
        for skill in skills {
            argv.push("--skill".to_string());
            argv.push(workdir.join(skill).to_string_lossy().into_owned());
        }
        argv.extend(user_argv.iter().cloned());
        Ok(argv)
    }

    fn isolation_summary(&self) -> &'static str {
        "path A flags applied: --no-skills --skill <workdir>"
    }

    fn agent_dir_hint(&self) -> Option<PathBuf> {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".pi").join("agent"))
    }

    fn write_run_agents(
        &self,
        run_dir: &Path,
        agents: &[super::AgentSpec],
    ) -> Result<super::AgentFiles> {
        let files = super::write_agent_files(run_dir, agents, agent_md)?;
        Ok(super::AgentFiles {
            loaded: false,
            files,
            include_hint: Some(
                "pi-subagents discovers agents only from ~/.pi/agent/agents and the project's \
                 .pi/agents; lunchbox never writes those — copy runs/<run_id>/agents/*.md into \
                 one of them for this session; they must not outlive the run"
                    .to_string(),
            ),
        })
    }

    fn selftest(&self, version: Option<&str>) -> Result<super::SelftestOutcome> {
        let Some(version) = version else {
            return Ok(super::SelftestOutcome::Skipped);
        };
        super::help_flag_selftest("pi", version, &["--no-skills", "--skill"])
    }

    fn explain(&self) -> String {
        "pi Path A: spawns `pi --no-skills --skill <workdir>/<skill> ...` with one --skill \
flag per locked package, followed by the user argv. Discovery of ~/.claude/skills, \
~/.agents/skills, and settings overlays is disabled by --no-skills; only the sealed \
workdir is passed. Flags only, no settings overlay. Belief verified against pi --help \
0.84.4 on 2026-09-06; selftest re-verifies. Path B: writes \
runs/<id>/agents/<worker>.md with inheritSkills false, skillPath -> the worker's \
pack, explicit skills; pi-subagents (probed 2026-09-06 on 0.84.4) has no \
per-invocation agent-dir override, so the files are printed, not auto-loaded."
            .to_string()
    }
}

fn agent_md(spec: &super::AgentSpec) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ninheritSkills: false\nskillPath: {}\nskills: {}\ntools: \
         read, grep, find, bash\n---\nWork only with the Skills in your skillPath. Do not \
         search ~/.agents/skills or any global skill directory.\n",
        spec.name,
        spec.description,
        spec.pack_dir.display(),
        spec.skills.join(", ")
    )
}
