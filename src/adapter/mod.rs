use crate::config::Config;
use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod none;
pub mod pi;

pub use none::NoneAdapter;
pub use pi::PiAdapter;

pub trait Adapter {
    fn name(&self) -> &'static str;

    fn detect(&self) -> Result<Option<String>>;

    fn skill_dirs(&self, cfg: &Config) -> Vec<PathBuf>;

    fn isolation_argv(
        &self,
        workdir: &Path,
        skills: &[String],
        user_argv: &[String],
    ) -> Result<Vec<String>>;

    fn agent_dir_hint(&self) -> Option<PathBuf>;

    #[allow(dead_code)]
    fn write_run_agents(&self, run_dir: &Path) -> Result<()>;

    fn selftest(&self) -> Result<SelftestOutcome>;

    fn explain(&self) -> String;
}

#[derive(Debug, PartialEq)]
pub enum SelftestOutcome {
    Ok,
    Skipped,
    Failed(String),
}

pub fn resolve_adapter(name: &str) -> Result<Box<dyn Adapter>> {
    match name {
        "none" => Ok(Box::new(NoneAdapter)),
        "pi" => Ok(Box::new(PiAdapter)),
        "omp" => bail!("omp adapter arrives after Phase 1"),
        other => bail!("unknown adapter '{other}' (available: none, pi)"),
    }
}

pub fn without_estimate(adapter_name: &str, cfg: &Config) -> u64 {
    let adapter = resolve_adapter(adapter_name).ok();
    let Some(adapter) = adapter else {
        return 0;
    };
    union_menu(adapter.as_ref(), cfg).0
}

pub struct DirReport {
    pub dir: PathBuf,
    pub exists: bool,
    pub skills: Vec<FoundDirSkill>,
}

pub fn scan_dirs(adapter: &dyn Adapter, cfg: &Config) -> Vec<DirReport> {
    adapter
        .skill_dirs(cfg)
        .into_iter()
        .map(|dir| {
            let exists = dir.is_dir();
            let skills = crate::library::scan_root(&dir)
                .unwrap_or_default()
                .into_iter()
                .map(|skill| FoundDirSkill {
                    tokens: crate::tokens::estimate(&skill.name, &skill.description),
                    name: skill.name,
                    dir: dir.clone(),
                })
                .collect();
            DirReport { dir, exists, skills }
        })
        .collect()
}

pub fn union_from(reports: &[DirReport]) -> (u64, Vec<FoundDirSkill>) {
    let mut by_name: BTreeMap<String, FoundDirSkill> = BTreeMap::new();
    for report in reports {
        for skill in &report.skills {
            by_name.entry(skill.name.clone()).or_insert_with(|| FoundDirSkill {
                name: skill.name.clone(),
                dir: skill.dir.clone(),
                tokens: skill.tokens,
            });
        }
    }
    let total = by_name.values().map(|s| s.tokens).sum();
    (total, by_name.into_values().collect())
}

pub fn union_menu(adapter: &dyn Adapter, cfg: &Config) -> (u64, Vec<FoundDirSkill>) {
    union_from(&scan_dirs(adapter, cfg))
}

#[derive(Debug, Clone)]
pub struct FoundDirSkill {
    pub name: String,
    pub dir: PathBuf,
    pub tokens: u64,
}

pub fn duplicates(skills: &[FoundDirSkill]) -> Vec<Vec<&FoundDirSkill>> {
    let mut by_name: BTreeMap<&str, Vec<&FoundDirSkill>> = BTreeMap::new();
    for skill in skills {
        by_name.entry(skill.name.as_str()).or_default().push(skill);
    }
    by_name
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

pub fn detect_version(binary: &str) -> Result<Option<String>> {
    let output = match Command::new(binary).arg("--version").output() {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    if !output.status.success() {
        bail!("'{binary} --version' exited with {}", output.status);
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let version = text
        .lines()
        .find_map(|line| extract_semver(line))
        .ok_or_else(|| anyhow::anyhow!("could not parse a semver from '{binary} --version'"))?;
    Ok(Some(version))
}

fn extract_semver(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut start = None;
    let mut end = None;
    for (index, byte) in bytes.iter().enumerate() {
        let is_version_char = byte.is_ascii_digit() || *byte == b'.';
        if byte.is_ascii_digit() && start.is_none() {
            start = Some(index);
        }
        if let Some(s) = start {
            if !is_version_char {
                if index > s {
                    end = Some(index);
                    break;
                }
                start = None;
            }
        }
    }
    if start.is_some() && end.is_none() {
        end = Some(bytes.len());
    }
    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            let candidate = &line[s..e];
            let dots = candidate.matches('.').count();
            if dots >= 2 && !candidate.starts_with('.') && !candidate.ends_with('.') {
                Some(candidate.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_extraction_from_version_output() {
        assert_eq!(extract_semver("pi 0.84.4"), Some("0.84.4".to_string()));
        assert_eq!(extract_semver("pi 1.2.3-beta.1"), Some("1.2.3".to_string()));
        assert_eq!(extract_semver("no version here"), None);
        assert_eq!(extract_semver("v2.0.0"), Some("2.0.0".to_string()));
        assert_eq!(extract_semver("pi 0.84"), None);
    }

    #[test]
    fn adapter_resolution() {
        assert_eq!(resolve_adapter("none").unwrap().name(), "none");
        assert_eq!(resolve_adapter("pi").unwrap().name(), "pi");
        let omp = resolve_adapter("omp").err().map(|e| e.to_string()).unwrap();
        assert_eq!(omp, "omp adapter arrives after Phase 1");
        assert!(resolve_adapter("zzz").is_err());
    }

    #[test]
    fn none_adapter_never_spawns() {
        let err = NoneAdapter
            .isolation_argv(Path::new("/w"), &["a".to_string()], &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("adapter none never spawns"), "{err}");
    }

    #[test]
    fn pi_isolation_argv_is_exact() {
        let argv = PiAdapter
            .isolation_argv(
                Path::new("/runs/lbx_x/workdir"),
                &["demo-review".to_string(), "demo-scan".to_string()],
                &["pi".to_string(), "-p".to_string(), "hello".to_string()],
            )
            .unwrap();
        assert_eq!(
            argv,
            vec![
                "pi".to_string(),
                "--no-skills".to_string(),
                "--skill".to_string(),
                "/runs/lbx_x/workdir/demo-review".to_string(),
                "--skill".to_string(),
                "/runs/lbx_x/workdir/demo-scan".to_string(),
                "pi".to_string(),
                "-p".to_string(),
                "hello".to_string(),
            ]
        );
    }

    #[test]
    fn path_b_is_refused() {
        let err = PiAdapter.write_run_agents(Path::new("/tmp/x")).unwrap_err().to_string();
        assert_eq!(err, "Path B not implemented in this build");
        assert!(NoneAdapter.write_run_agents(Path::new("/tmp/x")).is_err());
    }

    #[test]
    fn pi_skill_dirs_include_existing_pi_dir_only() {
        let home = tempfile::TempDir::new().unwrap();
        crate::config::with_home(home.path(), || {
            let cfg = Config::default();
            let dirs = PiAdapter.skill_dirs(&cfg);
            assert!(dirs.contains(&home.path().join(".agents").join("skills")));
            std::fs::create_dir_all(home.path().join(".pi").join("agent").join("skills"))
                .unwrap();
            let dirs = PiAdapter.skill_dirs(&cfg);
            assert!(dirs.contains(&home.path().join(".pi").join("agent").join("skills")));
        });
    }

    #[test]
    fn union_menu_dedupes_across_dirs_and_sums_tokens() {
        let home = tempfile::TempDir::new().unwrap();
        let global = home.path().join("global-skills");
        let project = home.path().join("project-skills");
        for (root, name, description) in [
            (&global, "shared", "Same skill in both dirs"),
            (&project, "shared", "Same skill in both dirs"),
            (&project, "extra", "Only in project"),
        ] {
            let package = root.join(name);
            std::fs::create_dir_all(&package).unwrap();
            std::fs::write(
                package.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: {description}\n---\n"),
            )
            .unwrap();
        }
        let cfg = Config {
            library_paths: vec![global, project],
            ..Config::default()
        };
        let (total, skills) = union_menu(&NoneAdapter, &cfg);
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["extra", "shared"]);
        assert_eq!(
            total,
            crate::tokens::estimate("shared", "Same skill in both dirs")
                + crate::tokens::estimate("extra", "Only in project")
        );
    }
}
