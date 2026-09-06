use super::Adapter;
use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::env;
use std::path::{Path, PathBuf};

pub struct OmpAdapter;

fn overlay_yaml(workdir: &Path) -> String {
    format!(
        r#"skills:
  enabled: true
  customDirectories:
    - {}
  enableAgentsUser: false
  enableAgentsProject: false
  enableClaudeUser: false
  enableClaudeProject: false
  enableCodexUser: false
  enablePiUser: false
  enablePiProject: false
disabledProviders: [native, claude, codex, gemini, github, opencode, cursor, agents-md]
"#,
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
        let mut dirs = super::pantry_base_dirs();
        if let Some(home) = env::var_os("HOME") {
            let agent_dir = PathBuf::from(home).join(".omp").join("agent");
            for child in ["skills", "managed-skills"] {
                let dir = agent_dir.join(child);
                if dir.is_dir() {
                    dirs.push(dir);
                }
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

    fn isolation_summary(&self) -> &'static str {
        "path A config overlay: --config <run>/omp-config.yml"
    }

    fn write_run_agents(&self, _run_dir: &Path) -> Result<()> {
        bail!("Path B not implemented in this build")
    }

    fn selftest(&self) -> Result<super::SelftestOutcome> {
        let Some(version) = self.detect()? else {
            return Ok(super::SelftestOutcome::Skipped);
        };
        super::help_flag_selftest("omp", &version, &["--config"])
    }

    fn explain(&self) -> String {
        "omp Path A: spawns `omp --config <run_dir>/omp-config.yml` followed by the user \
argv. The per-run overlay sets skills.customDirectories to the sealed workdir and turns \
every discovery source off (enableAgentsUser/Project, Claude, Codex, Pi user/project, and \
disabledProviders: native, claude, codex, gemini, github, opencode, cursor, agents-md), so \
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
