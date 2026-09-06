use super::Adapter;
use crate::config::Config;
use anyhow::{Result, bail};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct PiAdapter;

impl Adapter for PiAdapter {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn detect(&self) -> Result<Option<String>> {
        super::detect_version("pi")
    }

    fn skill_dirs(&self, _cfg: &Config) -> Vec<PathBuf> {
        let mut dirs = vec![
            env::current_dir()
                .map(|cwd| cwd.join(".agents").join("skills"))
                .unwrap_or_else(|_| PathBuf::from(".agents/skills")),
        ];
        if let Some(home) = env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".agents").join("skills"));
            let pi_dir = home.join(".pi").join("agent").join("skills");
            if pi_dir.is_dir() {
                dirs.push(pi_dir);
            }
        }
        dirs
    }

    fn isolation_argv(
        &self,
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

    fn agent_dir_hint(&self) -> Option<PathBuf> {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".pi").join("agent"))
    }

    fn write_run_agents(&self, _run_dir: &Path) -> Result<()> {
        bail!("Path B not implemented in this build")
    }

    fn selftest(&self) -> Result<super::SelftestOutcome> {
        let Some(version) = self.detect()? else {
            return Ok(super::SelftestOutcome::Skipped);
        };
        let output = Command::new("pi")
            .arg("--help")
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run 'pi --help': {e}"))?;
        if !output.status.success() {
            bail!("'pi --help' exited with {}", output.status);
        }
        let help = String::from_utf8_lossy(&output.stdout).into_owned();
        let has_no_skills = help.contains("--no-skills");
        let has_skill = help.contains("--skill");
        if has_no_skills && has_skill {
            Ok(super::SelftestOutcome::Ok)
        } else {
            Ok(super::SelftestOutcome::Failed(format!(
                "pi isolation flags drifted (pi {version}: --no-skills={has_no_skills}, --skill={has_skill})"
            )))
        }
    }

    fn explain(&self) -> String {
        "pi Path A: spawns `pi --no-skills --skill <workdir>/<skill> ...` with one --skill \
flag per locked package, followed by the user argv. Discovery of ~/.claude/skills, \
~/.agents/skills, and settings overlays is disabled by --no-skills; only the sealed \
workdir is passed. Flags only, no settings overlay. Belief verified against pi --help \
0.84.4 on 2026-09-06; selftest re-verifies."
            .to_string()
    }
}
