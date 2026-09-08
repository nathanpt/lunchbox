use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
#[derive(Clone, Debug)]
pub struct DiscoveredManifest {
    pub name: String,
    pub path: PathBuf,
    pub state: ManifestState,
}
#[derive(Clone, Debug)]
pub enum ManifestState {
    Ok {
        task: String,
        workers: Vec<(String, Vec<String>)>,
        tokens: u64,
    },
    Broken(String),
}

pub fn discovery_dirs() -> Result<Vec<PathBuf>> {
    Ok(vec![
        PathBuf::from("lunchbox").join("manifests"),
        crate::config::lunchbox_home()?.join("manifests"),
    ])
}

pub fn discover(skills: &BTreeMap<String, u64>) -> Vec<DiscoveredManifest> {
    let dirs = discovery_dirs().unwrap_or_default();
    discover_in(&dirs, skills)
}

fn discover_in(dirs: &[PathBuf], skills: &BTreeMap<String, u64>) -> Vec<DiscoveredManifest> {
    let mut found: Vec<DiscoveredManifest> = Vec::new();
    for dir in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file() && path.extension().is_some_and(|ext| ext == "toml")
            })
            .collect();
        paths.sort();
        for path in paths {
            let Some(name) = path.file_stem().map(|stem| stem.to_string_lossy().into_owned())
            else {
                continue;
            };
            if found.iter().any(|known| known.name == name) {
                continue;
            }
            let state = match crate::run::read_manifest_input(&path) {
                Ok(input) => match input.validate() {
                    Ok(_) => ManifestState::Ok {
                        task: input.task.clone(),
                        workers: input
                            .workers
                            .iter()
                            .map(|worker| (worker.name.clone(), worker.pack.clone()))
                            .collect(),
                        tokens: manifest_tokens(&input, skills),
                    },
                    Err(error) => ManifestState::Broken(format!("{error:#}")),
                },
                Err(error) => ManifestState::Broken(format!("{error:#}")),
            };
            found.push(DiscoveredManifest { name, path, state });
        }
    }
    found
}

fn manifest_tokens(input: &crate::run::ManifestInput, skills: &BTreeMap<String, u64>) -> u64 {
    let mut names: Vec<String> = Vec::new();
    for worker in &input.workers {
        for pin in &worker.pack {
            let name = pin
                .split_once('@')
                .map_or(pin.clone(), |(base, _)| base.to_string());
            if !names.iter().any(|known| known == &name) {
                names.push(name);
            }
        }
    }
    names.iter().filter_map(|name| skills.get(name)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skills_map() -> BTreeMap<String, u64> {
        [("demo-review".to_string(), 20u64), ("demo-scan".to_string(), 7u64)]
            .into_iter()
            .collect()
    }

    fn write_manifest(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{name}.toml"));
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn missing_dirs_yield_no_manifests() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dirs = vec![tmp.path().join("lunchbox").join("manifests")];
        assert!(discover_in(&dirs, &skills_map()).is_empty());
    }

    #[test]
    fn broken_toml_is_reported_not_fatal() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_manifest(tmp.path(), "broken", "schema = 1\ntask = 1\n");
        let found = discover_in(&[tmp.path().to_path_buf()], &skills_map());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "broken");
        assert!(matches!(&found[0].state, ManifestState::Broken(error) if !error.is_empty()));
    }

    #[test]
    fn project_dir_wins_on_stem_collision() {
        let project = tempfile::TempDir::new().unwrap();
        let home = tempfile::TempDir::new().unwrap();
        write_manifest(
            project.path(),
            "run",
            "schema = 1\ntask = \"project\"\nadapter = \"none\"\n[[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n",
        );
        write_manifest(
            home.path(),
            "run",
            "schema = 1\ntask = \"home\"\nadapter = \"none\"\n[[workers]]\nname = \"w\"\npack = [\"demo-scan\"]\n",
        );
        let found = discover_in(
            &[project.path().to_path_buf(), home.path().to_path_buf()],
            &skills_map(),
        );
        assert_eq!(found.len(), 1, "collision must not list both: {found:?}");
        assert_eq!(found[0].name, "run");
        assert_eq!(found[0].path, project.path().join("run.toml"));
        let ManifestState::Ok { task, workers, tokens } = &found[0].state else {
            panic!("expected Ok state");
        };
        assert_eq!(task, "project");
        assert_eq!(workers, &vec![("w".to_string(), vec!["demo-review".to_string()])]);
        assert_eq!(*tokens, 20);
    }

    #[test]
    fn tokens_union_pins_strip_hashes_and_skip_unknown_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_manifest(
            tmp.path(),
            "multi",
            "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\
             [[workers]]\nname = \"a\"\npack = [\"demo-review\", \"demo-review@sha256:aaaa\", \"unknown\"]\n\
             [[workers]]\nname = \"b\"\npack = [\"demo-scan\"]\n",
        );
        let found = discover_in(&[tmp.path().to_path_buf()], &skills_map());
        let ManifestState::Ok { tokens, .. } = &found[0].state else {
            panic!("expected Ok state");
        };
        assert_eq!(*tokens, 27, "union by name: 20 + 7, unknown contributes 0");
    }

    #[test]
    fn non_toml_files_and_subdirs_are_skipped() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_manifest(tmp.path(), "real", "schema = 1\ntask = \"t\"\nadapter = \"none\"\n[[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n");
        fs::write(tmp.path().join("notes.txt"), "x").unwrap();
        fs::create_dir_all(tmp.path().join("subdir.toml")).unwrap();
        let found = discover_in(&[tmp.path().to_path_buf()], &skills_map());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "real");
    }
}
