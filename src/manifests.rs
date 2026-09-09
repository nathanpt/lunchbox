use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub fn discovery_dirs() -> Result<Vec<PathBuf>> {
    Ok(vec![
        PathBuf::from("lunchbox").join("manifests"),
        crate::config::lunchbox_home()?.join("manifests"),
    ])
}

pub fn resolve_from(from: &str) -> Result<PathBuf> {
    resolve_in(from, &discovery_dirs()?)
}

fn resolve_in(from: &str, dirs: &[PathBuf]) -> Result<PathBuf> {
    let candidate = PathBuf::from(from);
    if candidate.exists() {
        return Ok(candidate);
    }
    let mut available: Vec<String> = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut stems: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file() && path.extension().is_some_and(|ext| ext == "toml")
            })
            .filter_map(|path| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .collect();
        stems.sort();
        if stems.iter().any(|stem| stem == from) {
            return Ok(dir.join(format!("{from}.toml")));
        }
        available.extend(stems);
    }
    let searched = dirs
        .iter()
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if available.is_empty() {
        available.push("none".to_string());
    }
    anyhow::bail!(
        "manifest '{from}' not found in any discovery dir (searched: {searched}); available: {}",
        available.join(", ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn resolve_from_prefers_existing_paths_unchanged() {
        let tmp = tempfile::TempDir::new().unwrap();
        let direct = tmp.path().join("explicit.toml");
        fs::write(&direct, "schema = 1\n").unwrap();
        let other = tempfile::TempDir::new().unwrap();
        write_manifest(other.path(), "explicit", "schema = 1\n");
        let resolved = resolve_in(
            direct.to_str().unwrap(),
            &[other.path().to_path_buf()],
        )
        .unwrap();
        assert_eq!(resolved, direct, "an existing path must pass through");
    }

    #[test]
    fn resolve_from_project_dir_wins_over_home() {
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        write_manifest(project.path(), "run", "schema = 1\n");
        write_manifest(home.path(), "run", "schema = 1\n");
        write_manifest(home.path(), "other", "schema = 1\n");
        let resolved = resolve_in(
            "run",
            &[project.path().to_path_buf(), home.path().to_path_buf()],
        )
        .unwrap();
        assert_eq!(resolved, project.path().join("run.toml"));
        let resolved = resolve_in(
            "other",
            &[project.path().to_path_buf(), home.path().to_path_buf()],
        )
        .unwrap();
        assert_eq!(resolved, home.path().join("other.toml"));
    }

    #[test]
    fn resolve_from_miss_lists_searched_dirs_and_candidates() {
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        write_manifest(home.path(), "e2e", "schema = 1\n");
        let error = resolve_in(
            "missing",
            &[project.path().to_path_buf(), home.path().to_path_buf()],
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("manifest 'missing' not found"), "{error}");
        assert!(error.contains("available: e2e"), "{error}");
        let empty = tempfile::TempDir::new().unwrap();
        let error = resolve_in("missing", &[empty.path().to_path_buf()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("available: none"), "{error}");
    }
}
