use super::Adapter;
use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct OmpAdapter;

fn overlay_yaml(workdir: &Path) -> String {
    format!(
        "skills:\n  enabled: true\n  customDirectories:\n    - {}\n  enableAgentsUser: false\n  enableAgentsProject: false\n  enableClaudeUser: false\n  enableClaudeProject: false\n  enableCodexUser: false\n  enablePiUser: false\n  enablePiProject: false\ndisabledProviders: [native, claude, codex, gemini, github, opencode, cursor, agents-md]\n",
        workdir.display()
    )
}

impl Adapter for OmpAdapter {
    fn name(&self) -> &'static str {
        "omp"
    }

    fn detect(&self) -> Result<Option<String>> {
        super::detect_version("omp")
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
            let skills_dir = home.join(".omp").join("agent").join("skills");
            if skills_dir.is_dir() {
                dirs.push(skills_dir);
            }
            let managed_dir = home.join(".omp").join("agent").join("managed-skills");
            if managed_dir.is_dir() {
                dirs.push(managed_dir);
            }
        }
        dirs
    }

    fn isolation_argv(
        &self,
        run_dir: &Path,
        workdir: &Path,
        _skills: &[String],
        user_argv: &[String],
    ) -> Result<Vec<String>> {
        let overlay_path = run_dir.join("omp-config.yml");
        std::fs::write(&overlay_path, overlay_yaml(workdir))
            .with_context(|| format!("failed to write {}", overlay_path.display()))?;
        Ok(["omp".to_string(), "--config".to_string()]
            .into_iter()
            .chain([overlay_path.to_string_lossy().into_owned()])
            .chain(user_argv.iter().cloned())
            .collect())
    }

    fn agent_dir_hint(&self) -> Option<PathBuf> {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".omp").join("agent"))
    }

    fn write_run_agents(&self, _run_dir: &Path) -> Result<()> {
        bail!("Path B not implemented in this build")
    }

    fn selftest(&self) -> Result<super::SelftestOutcome> {
        let Some(version) = self.detect()? else {
            return Ok(super::SelftestOutcome::Skipped);
        };
        let output = Command::new("omp")
            .arg("--help")
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run 'omp --help': {e}"))?;
        if !output.status.success() {
            bail!("'omp --help' exited with {}", output.status);
        }
        let help = String::from_utf8_lossy(&output.stdout).into_owned();
        let has_config = help.contains("--config");
        if has_config {
            Ok(super::SelftestOutcome::Ok)
        } else {
            Ok(super::SelftestOutcome::Failed(format!(
                "omp isolation flags drifted (omp {version}: --config={has_config})"
            )))
        }
    }

    fn explain(&self) -> String {
        "omp Path A: spawns `omp --config <run_dir>/omp-config.yml` followed by the user \
argv. The per-run overlay sets skills.customDirectories to the sealed workdir and turns \
every discovery source off (enableAgentsUser/Project, Claude, Codex, Pi user/project), so \
the harness sees only the mounted packages. --no-skills is insufficient (probed: it also \
disables customDirectories — with --no-skills plus the overlay the probe listed no skills); \
--skills only filters already-discovered skills; OMP_PROFILE/--profile isolates auth and \
session state, not skill discovery. Belief verified against omp 18.1.11 on 2026-09-06 by a \
headless `omp -p` probe: the reply listed exactly the mounted marker skill, the five \
foreign pantry skills appeared without the overlay and were absent with it. Selftest \
re-verifies the deterministic part only (omp --help still offers --config); the live \
probe is not repeated per run."
            .to_string()
    }
}
