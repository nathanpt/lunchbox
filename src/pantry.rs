use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;


pub struct Added {
    pub name: String,
    pub root: PathBuf,
    pub skills: usize,
}

pub fn pantry_home() -> Result<PathBuf> {
    Ok(crate::config::lunchbox_home()?.join("pantry"))
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

fn is_normal_relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn is_plain_component(name: &str) -> bool {
    is_normal_relative(name) && !name.starts_with('.')
}

fn validate_subdir(sub: &str) -> Result<()> {
    if !is_normal_relative(sub) {
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

fn raw_pantries(home: &Path) -> Result<Vec<(String, PathBuf, Option<String>)>> {
    let entries = match fs::read_dir(home) {
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

pub fn resolve_roots() -> Result<Vec<PathBuf>> {
    roots_at(&pantry_home()?)
}

fn roots_at(home: &Path) -> Result<Vec<PathBuf>> {
    raw_pantries(home)?
        .into_iter()
        .map(|(name, repo, path_override)| {
            detect_pantry_root(&name, &repo, path_override.as_deref())
        })
        .collect()
}

pub struct PantryStatus {
    pub name: String,
    pub repo: PathBuf,
    pub state: PantryState,
}

pub enum PantryState {
    Healthy { root: PathBuf, skills: usize },
    Broken(String),
}

impl PantryStatus {
    pub fn summary_line(&self) -> String {
        match &self.state {
            PantryState::Healthy { root, skills } => format!(
                "  {:<16} {}  {} skills",
                self.name,
                root.display(),
                skills
            ),
            PantryState::Broken(error) => format!("  {:<16} error: {}", self.name, error),
        }
    }
}

pub fn pantry_statuses() -> Result<Vec<PantryStatus>> {
    statuses_at(&pantry_home()?)
}

fn statuses_at(home: &Path) -> Result<Vec<PantryStatus>> {
    Ok(raw_pantries(home)?
        .into_iter()
        .map(|(name, repo, path_override)| {
            let state = match detect_pantry_root(&name, &repo, path_override.as_deref()) {
                Ok(root) => match crate::library::scan_root(&root) {
                    Ok(skills) => PantryState::Healthy {
                        root,
                        skills: skills.len(),
                    },
                    Err(error) => PantryState::Broken(format!("{error:#}")),
                },
                Err(error) => PantryState::Broken(format!("{error:#}")),
            };
            PantryStatus { name, repo, state }
        })
        .collect())
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
        Err(error) => return Err(discard_clone(&repo, error)),
    };
    let skills = match crate::library::scan_root(&root) {
        Ok(skills) => skills.len(),
        Err(error) => return Err(discard_clone(&repo, error)),
    };
    if let Some(sub) = path {
        if let Err(error) = fs::write(override_file(&home, &name), format!("{sub}\n")) {
            let _ = fs::remove_dir_all(&repo);
            return Err(error).context(format!("failed to record --path for pantry '{name}'"));
        }
    }
    Ok(Added { name, root, skills })
}

fn discard_clone(repo: &Path, error: anyhow::Error) -> anyhow::Error {
    let _ = fs::remove_dir_all(repo);
    error
}

pub fn update(name: Option<&str>) -> Result<Vec<String>> {
    let raw = raw_pantries(&pantry_home()?)?;
    let selected: Vec<&(String, PathBuf, Option<String>)> = match name {
        Some(want) => {
            let found = raw.iter().find(|(pantry, _, _)| pantry == want);
            match found {
                Some(entry) => vec![entry],
                None => bail!("no managed pantry named '{want}'"),
            }
        }
        None => raw.iter().collect(),
    };
    let mut updated = Vec::new();
    for (pantry, repo, path_override) in selected {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("pull")
            .arg("--ff-only")
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run git pull: {e}"))?;
        if !output.status.success() {
            bail!(
                "failed to update managed pantry '{}' (exit {}){}",
                pantry,
                output.status.code().unwrap_or(-1),
                crate::resolve::stderr_excerpt(&output.stderr)
            );
        }
        detect_pantry_root(pantry, repo, path_override.as_deref())?;
        updated.push(pantry.clone());
    }
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn roots_at_lists_managed_and_skips_hidden() {
        let dir = repo_home();
        let pantry = dir.path().join("pantry");
        write_skill(&pantry.join("agent-skills").join("skills"), "alpha");
        fs::create_dir_all(pantry.join(".hidden").join("skills")).unwrap();
        let roots = roots_at(&pantry).unwrap();
        assert_eq!(
            roots,
            vec![pantry.join("agent-skills").join("skills")]
        );
    }

    #[test]
    fn broken_pantry_fails_closed_but_reports() {
        let dir = repo_home();
        let pantry = dir.path().join("pantry");
        fs::create_dir_all(pantry.join("junk").join("stuff")).unwrap();
        assert!(roots_at(&pantry).is_err());
        let statuses = statuses_at(&pantry).unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "junk");
        match &statuses[0].state {
            PantryState::Healthy { .. } => panic!("junk pantry must report broken"),
            PantryState::Broken(error) => {
                assert!(error.contains("no Skill packages"), "{error}")
            }
        }
    }

    #[test]
    fn dup_name_pantry_scans_broken_not_healthy() {
        let dir = repo_home();
        let pantry = dir.path().join("pantry");
        write_skill(&pantry.join("dup").join("skills"), "same");
        write_skill(&pantry.join("dup").join("skills"), "beta");
        let path = pantry.join("dup").join("skills").join("beta").join("SKILL.md");
        fs::write(&path, "---\nname: same\ndescription: b\n---\n").unwrap();
        let statuses = statuses_at(&pantry).unwrap();
        match &statuses[0].state {
            PantryState::Healthy { .. } => {
                panic!("duplicate frontmatter names must report broken")
            }
            PantryState::Broken(error) => {
                assert!(error.contains("two packages with the name 'same'"), "{error}")
            }
        }
    }
}
