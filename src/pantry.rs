use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub struct ManagedPantry {
    pub name: String,
    pub repo: PathBuf,
    pub root: PathBuf,
    pub path_override: Option<String>,
}

pub struct Added {
    pub name: String,
    pub root: PathBuf,
    pub skills: usize,
}

pub fn pantry_home() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .context("HOME is not set; cannot locate the managed pantry")?;
    Ok(PathBuf::from(home).join(".lunchbox").join("pantry"))
}

fn override_file(pantry_home: &Path, name: &str) -> PathBuf {
    pantry_home.join(format!("{name}.path"))
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

pub fn name_from_url(url: &str) -> Result<String> {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed.rsplit(['/', ':']).next().unwrap_or(trimmed);
    let name = last.strip_suffix(".git").unwrap_or(last);
    if !is_plain_component(name) {
        bail!(
            "cannot derive a pantry name from '{url}' (got '{name}'); \
             the URL's last path component must be a plain directory name"
        );
    }
    Ok(name.to_string())
}

fn is_plain_component(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn validate_subdir(sub: &str) -> Result<()> {
    let path = Path::new(sub);
    if path.is_absolute()
        || path.components().count() == 0
        || !path
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        bail!("--path must be a relative subdirectory without '.' or '..' components");
    }
    Ok(())
}

fn dir_has_skill_children(dir: &Path) -> Result<bool> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).context(format!("failed to list {}", dir.display())),
    };
    for entry in entries {
        let path = entry?.path();
        if !path.is_dir() || is_hidden(&path) {
            continue;
        }
        if path.join("SKILL.md").is_file() {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn detect_pantry_root(
    name: &str,
    repo: &Path,
    path_override: Option<&str>,
) -> Result<PathBuf> {
    if let Some(sub) = path_override {
        validate_subdir(sub)
            .with_context(|| format!("managed pantry '{name}' has an invalid recorded path"))?;
        let root = repo.join(sub);
        if !dir_has_skill_children(&root)? {
            bail!(
                "managed pantry '{name}': no Skill packages under '{sub}' in {}",
                repo.display()
            );
        }
        return Ok(root);
    }
    let mut candidates = Vec::new();
    if dir_has_skill_children(repo)? {
        candidates.push(repo.to_path_buf());
    }
    for entry in
        fs::read_dir(repo).with_context(|| format!("failed to list {}", repo.display()))?
    {
        let path = entry?.path();
        if !path.is_dir() || is_hidden(&path) {
            continue;
        }
        if dir_has_skill_children(&path)? {
            candidates.push(path);
        }
    }
    match candidates.as_slice() {
        [only] => Ok(only.clone()),
        [] => bail!(
            "managed pantry '{name}': no Skill packages found in {} (expected \
             <name>/SKILL.md packages at the repository root or in a single \
             subdirectory); re-add with --path <subdir> or remove the directory",
            repo.display()
        ),
        many => {
            let list = many
                .iter()
                .map(|c| c.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "managed pantry '{name}': multiple candidate skill roots in {}: \
                 {list}; re-add with --path <subdir>",
                repo.display()
            );
        }
    }
}

fn read_override(pantry_home: &Path, name: &str) -> Result<Option<String>> {
    let file = override_file(pantry_home, name);
    match fs::read_to_string(&file) {
        Ok(text) => Ok(Some(text.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context(format!("failed to read {}", file.display())),
    }
}

fn raw_pantries() -> Result<Vec<(String, PathBuf, Option<String>)>> {
    let home = pantry_home()?;
    let entries = match fs::read_dir(&home) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context(format!("failed to list {}", home.display())),
    };
    let mut raw = Vec::new();
    for entry in entries {
        let repo = entry?.path();
        if !repo.is_dir() || is_hidden(&repo) {
            continue;
        }
        let Some(name) = repo.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let name = name.to_string();
        let path_override = read_override(&home, &name)?;
        raw.push((name, repo, path_override));
    }
    raw.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(raw)
}

pub fn managed_pantries() -> Result<Vec<ManagedPantry>> {
    raw_pantries()?
        .into_iter()
        .map(|(name, repo, path_override)| {
            let root = detect_pantry_root(&name, &repo, path_override.as_deref())?;
            Ok(ManagedPantry {
                name,
                repo,
                root,
                path_override,
            })
        })
        .collect()
}

pub fn resolve_roots() -> Result<Vec<PathBuf>> {
    Ok(managed_pantries()?
        .into_iter()
        .map(|pantry| pantry.root)
        .collect())
}

pub struct PantryStatus {
    pub name: String,
    pub repo: PathBuf,
    pub root: Option<PathBuf>,
    pub skills: usize,
    pub error: Option<String>,
}

pub fn pantry_statuses() -> Vec<PantryStatus> {
    let raw = match raw_pantries() {
        Ok(raw) => raw,
        Err(_) => return Vec::new(),
    };
    raw.into_iter()
        .map(|(name, repo, path_override)| match detect_pantry_root(
            &name,
            &repo,
            path_override.as_deref(),
        ) {
            Ok(root) => {
                let skills = crate::library::scan_root(&root)
                    .map(|skills| skills.len())
                    .unwrap_or(0);
                PantryStatus {
                    name,
                    repo,
                    root: Some(root),
                    skills,
                    error: None,
                }
            }
            Err(error) => PantryStatus {
                name,
                repo,
                root: None,
                skills: 0,
                error: Some(format!("{error:#}")),
            },
        })
        .collect()
}

pub fn add(url: &str, path: Option<&str>) -> Result<Added> {
    let name = name_from_url(url)?;
    if let Some(sub) = path {
        validate_subdir(sub)?;
    }
    let home = pantry_home()?;
    let repo = home.join(&name);
    if repo.exists() {
        bail!(
            "managed pantry '{name}' already exists at {}",
            repo.display()
        );
    }
    fs::create_dir_all(&home).with_context(|| format!("failed to create {}", home.display()))?;
    let output = Command::new("git")
        .arg("clone")
        .arg(url)
        .arg(&repo)
        .output()
        .map_err(|e| anyhow::anyhow!("failed to run git clone: {e}"))?;
    if !output.status.success() {
        let _ = fs::remove_dir_all(&repo);
        bail!(
            "failed to clone '{url}' into {} (exit {}){}",
            repo.display(),
            output.status.code().unwrap_or(-1),
            crate::resolve::stderr_excerpt(&output.stderr)
        );
    }
    let root = match detect_pantry_root(&name, &repo, path) {
        Ok(root) => root,
        Err(error) => {
            let _ = fs::remove_dir_all(&repo);
            return Err(error);
        }
    };
    if let Some(sub) = path {
        fs::write(override_file(&home, &name), format!("{sub}\n"))
            .with_context(|| format!("failed to record --path for pantry '{name}'"))?;
    }
    let skills = crate::library::scan_root(&root)?.len();
    Ok(Added { name, root, skills })
}

pub fn update(name: Option<&str>) -> Result<Vec<String>> {
    let pantries = managed_pantries()?;
    let selected: Vec<&ManagedPantry> = match name {
        Some(want) => {
            let found = pantries.iter().find(|pantry| pantry.name == want);
            match found {
                Some(pantry) => vec![pantry],
                None => bail!("no managed pantry named '{want}'"),
            }
        }
        None => pantries.iter().collect(),
    };
    let mut updated = Vec::new();
    for pantry in selected {
        let output = Command::new("git")
            .arg("-C")
            .arg(&pantry.repo)
            .arg("pull")
            .arg("--ff-only")
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git pull: {e}"))?;
        if !output.status.success() {
            bail!(
                "failed to update managed pantry '{}' (exit {}){}",
                pantry.name,
                output.status.code().unwrap_or(-1),
                crate::resolve::stderr_excerpt(&output.stderr)
            );
        }
        detect_pantry_root(
            &pantry.name,
            &pantry.repo,
            pantry.path_override.as_deref(),
        )?;
        updated.push(pantry.name.clone());
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::with_home;

    fn write_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir.join(name)).unwrap();
        fs::write(
            dir.join(name).join("SKILL.md"),
            format!("---\nname: {name}\ndescription: d\n---\n"),
        )
        .unwrap();
    }

    fn repo_home() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn names_derive_from_urls() {
        assert_eq!(
            name_from_url("https://github.com/nathanpt/agent-skills").unwrap(),
            "agent-skills"
        );
        assert_eq!(
            name_from_url("https://github.com/nathanpt/agent-skills.git").unwrap(),
            "agent-skills"
        );
        assert_eq!(
            name_from_url("https://github.com/nathanpt/agent-skills/").unwrap(),
            "agent-skills"
        );
        assert_eq!(name_from_url("git@host:skills").unwrap(), "skills");
    }

    #[test]
    fn names_reject_non_plain_components() {
        for url in [
            "https://host/..",
            "https://host/.",
            "https://host/.hidden",
            "https://host/.hidden.git",
        ] {
            assert!(name_from_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn detect_prefers_root_shaped_repos() {
        let dir = repo_home();
        write_skill(dir.path(), "alpha");
        assert_eq!(
            detect_pantry_root("r", dir.path(), None).unwrap(),
            dir.path()
        );
    }

    #[test]
    fn detect_finds_single_subdir() {
        let dir = repo_home();
        write_skill(&dir.path().join("skills"), "alpha");
        assert_eq!(
            detect_pantry_root("r", dir.path(), None).unwrap(),
            dir.path().join("skills")
        );
    }

    #[test]
    fn detect_ignores_dot_dirs() {
        let dir = repo_home();
        write_skill(&dir.path().join("skills"), "alpha");
        write_skill(&dir.path().join(".git"), "packed");
        assert_eq!(
            detect_pantry_root("r", dir.path(), None).unwrap(),
            dir.path().join("skills")
        );
    }

    #[test]
    fn detect_rejects_ambiguity_and_emptiness() {
        let dir = repo_home();
        write_skill(dir.path(), "alpha");
        write_skill(&dir.path().join("skills"), "beta");
        let err = detect_pantry_root("r", dir.path(), None).unwrap_err().to_string();
        assert!(err.contains("multiple candidate skill roots"), "{err}");
        assert!(err.contains("skills"), "{err}");

        let empty = repo_home();
        let err = detect_pantry_root("r", empty.path(), None).unwrap_err().to_string();
        assert!(err.contains("no Skill packages found"), "{err}");
    }

    #[test]
    fn path_override_wins_and_is_validated() {
        let dir = repo_home();
        write_skill(&dir.path().join("skills"), "alpha");
        fs::create_dir_all(dir.path().join("other")).unwrap();
        assert_eq!(
            detect_pantry_root("r", dir.path(), Some("skills")).unwrap(),
            dir.path().join("skills")
        );
        let err = detect_pantry_root("r", dir.path(), Some("missing")).unwrap_err().to_string();
        assert!(err.contains("no Skill packages under 'missing'"), "{err}");
        for bad in ["..", "/abs", "a/../b", ""] {
            assert!(detect_pantry_root("r", dir.path(), Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn resolve_roots_list_managed_after_creation() {
        let home = repo_home();
        with_home(home.path(), || {
            let pantry = home.path().join(".lunchbox").join("pantry");
            write_skill(&pantry.join("agent-skills").join("skills"), "alpha");
            let roots = resolve_roots().unwrap();
            assert_eq!(
                roots,
                vec![pantry.join("agent-skills").join("skills")]
            );
        });
    }

    #[test]
    fn broken_pantry_fails_closed_but_reports() {
        let home = repo_home();
        with_home(home.path(), || {
            let pantry = home.path().join(".lunchbox").join("pantry");
            fs::create_dir_all(pantry.join("junk").join("stuff")).unwrap();
            assert!(resolve_roots().is_err());
            let statuses = pantry_statuses();
            assert_eq!(statuses.len(), 1);
            assert_eq!(statuses[0].name, "junk");
            assert!(statuses[0].root.is_none());
            assert!(
                statuses[0].error.as_deref().unwrap().contains("no Skill packages"),
                "{:?}",
                statuses[0].error
            );
        });
    }
}
