use super::Adapter;
use crate::config::Config;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub struct NoneAdapter;

impl Adapter for NoneAdapter {
    fn name(&self) -> &'static str {
        "none"
    }

    fn detect(&self) -> Result<Option<String>> {
        Ok(Some("builtin".to_string()))
    }

    fn skill_dirs(&self, cfg: &Config) -> Vec<PathBuf> {
        cfg.library_paths.clone()
    }

    fn isolation_argv(
        &self,
        _run_dir: &Path,
        _workdir: &Path,
        _skills: &[String],
        _user_argv: &[String],
    ) -> Result<Vec<String>> {
        bail!("adapter none never spawns; the workdir is mounted, not spawned")
    }

    fn agent_dir_hint(&self) -> Option<PathBuf> {
        None
    }

    fn write_run_agents(&self, _run_dir: &Path) -> Result<()> {
        bail!("Path B not implemented in this build")
    }

    fn selftest(&self) -> Result<super::SelftestOutcome> {
        Ok(super::SelftestOutcome::Ok)
    }

    fn explain(&self) -> String {
        "none: mounts the sealed workdir and prints it, never spawns a process. \
Any argv after -- is stored in the manifest only. Useful for tests and inspection."
            .to_string()
    }
}
