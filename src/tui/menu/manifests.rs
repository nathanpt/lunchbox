use crate::config::Config;
use crate::tui::menu::{chrome, Action};
use anyhow::Result;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

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
        workers: Vec<(String, Vec<String>, Option<Vec<String>>)>,
        tokens: u64,
    },
    Broken(String),
}

pub fn discover(skills: &BTreeMap<String, u64>) -> Vec<DiscoveredManifest> {
    let dirs = crate::manifests::discovery_dirs().unwrap_or_default();
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
                            .map(|worker| {
                                (
                                    worker.name.clone(),
                                    worker.pack.clone(),
                                    worker.tools.clone(),
                                )
                            })
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

pub struct ManifestsState {
    pub manifests: Vec<DiscoveredManifest>,
    pub cursor: usize,
}

impl ManifestsState {
    pub fn build(cfg: &Config, libraries: &[std::path::PathBuf]) -> Result<Self> {
        let sections = super::pantry::build_sections(cfg, libraries)?;
        let union = super::pantry::skill_union(&sections);
        let skills: BTreeMap<String, u64> =
            union.values().map(|skill| (skill.name.clone(), skill.tokens)).collect();
        Ok(ManifestsState {
            manifests: discover(&skills),
            cursor: 0,
        })
    }
}

pub struct ManifestDetailState {
    pub manifest: DiscoveredManifest,
    pub tokens: std::collections::BTreeMap<String, u64>,
}

impl ManifestDetailState {
    pub fn new(manifest: DiscoveredManifest, cfg: &Config, libraries: &[std::path::PathBuf]) -> Result<Self> {
        let sections = super::pantry::build_sections(cfg, libraries)?;
        let tokens = super::pantry::skill_union(&sections)
            .values()
            .map(|skill| (skill.name.clone(), skill.tokens))
            .collect();
        Ok(ManifestDetailState { manifest, tokens })
    }
}

pub fn render(state: &mut ManifestsState, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (index, manifest) in state.manifests.iter().enumerate() {
        let marker = if index == state.cursor { "▸ " } else { "  " };
        let line = match &manifest.state {
            ManifestState::Ok {
                task,
                workers,
                tokens,
            } => {
                let text = format!(
                    "{:<20} {:<24} {} workers  ~{} tokens",
                    manifest.name,
                    task,
                    workers.len(),
                    tokens
                );
                if index == state.cursor {
                    Line::from(vec![Span::raw(marker), chrome::bold(text)])
                } else {
                    Line::from(vec![Span::raw(marker), Span::raw(text)])
                }
            }
            ManifestState::Broken(error) => Line::from(vec![
                Span::raw(marker),
                chrome::bad(format!("{:<20} error: {}", manifest.name, error)),
            ]),
        };
        lines.push(line);
    }
    if state.manifests.is_empty() {
        lines.push(Line::from(chrome::bold("No manifests found".to_string())));
        lines.push(Line::from(chrome::dim(
            "looked in ./lunchbox/manifests and ~/.lunchbox/manifests".to_string(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            chrome::good("n".to_string()),
            Span::raw("  new manifest here"),
        ]));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut ManifestsState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    if state.manifests.is_empty() {
        return match (key.code, key.modifiers) {
            (KeyCode::Char('q'), _) => Action::Quit,
            (KeyCode::Esc, _) => Action::Pop,
            (KeyCode::Char('n'), KeyModifiers::NONE) => Action::NewEditor,
            _ => Action::Continue,
        };
    }
    let max = state.manifests.len() - 1;
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Action::Quit,
        (KeyCode::Esc, _) => Action::Pop,
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.cursor = state.cursor.saturating_sub(1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.cursor = (state.cursor + 1).min(max);
            Action::Continue
        }
        (KeyCode::Enter, _) => Action::Detail(state.manifests[state.cursor].clone()),
        (KeyCode::Char('e'), KeyModifiers::NONE) => {
            Action::EditManifest(state.manifests[state.cursor].clone())
        }
        (KeyCode::Char('n'), KeyModifiers::NONE) => Action::NewEditor,
        _ => Action::Continue,
    }
}

pub fn render_detail(state: &mut ManifestDetailState, frame: &mut Frame, area: Rect) {
    let manifest = &state.manifest;
    let label = |text: &str| chrome::dim(format!("{text:<15} "));
    let mut lines = vec![
        Line::from(vec![label("manifest"), Span::raw(manifest.name.clone())]),
        Line::from(vec![
            label("path"),
            Span::raw(manifest.path.display().to_string()),
        ]),
    ];
    match &manifest.state {
        ManifestState::Ok {
            task,
            workers,
            tokens,
            ..
        } => {
            lines.push(Line::from(vec![label("task"), Span::raw(task.clone())]));
            lines.push(Line::from(vec![
                label("menu_tokens"),
                Span::raw(format!("~{tokens}")),
            ]));
            lines.push(Line::from(""));
            for (name, pack, tools) in workers {
                lines.push(Line::from(chrome::bold(format!("worker {name}"))));
                for pin in pack {
                    let base = pin.split_once('@').map_or(pin.as_str(), |(base, _)| base);
                    let tokens = state.tokens.get(base).cloned().unwrap_or(0);
                    let unknown = state.tokens.get(base).is_none();
                    let line = if unknown {
                        format!("  - {pin}  ?")
                    } else {
                        format!("  - {pin}  {tokens}")
                    };
                    lines.push(Line::from(chrome::dim(line)));
                }
                if let Some(tools) = tools {
                    if !tools.is_empty() {
                        lines.push(Line::from(chrome::dim(format!(
                            "  tools {}",
                            tools.join(", ")
                        ))));
                    }
                }
            }
        }
        ManifestState::Broken(error) => {
            lines.push(Line::from(""));
            lines.push(Line::from(chrome::bad(format!("error: {error}"))));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_detail_event(_state: &mut ManifestDetailState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Action::Quit,
        (KeyCode::Esc, _) => Action::Pop,
        _ => Action::Continue,
    }
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
        let ManifestState::Ok { task, workers, tokens, .. } = &found[0].state else {
            panic!("expected Ok state");
        };
        assert_eq!(task, "project");
        assert_eq!(
            workers,
            &vec![(
                "w".to_string(),
                vec!["demo-review".to_string()],
                None
            )]
        );
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

    #[test]
    fn detail_lists_per_pin_tokens() {
        let manifest = DiscoveredManifest {
            name: "rt".to_string(),
            path: std::path::PathBuf::from("lunchbox/manifests/rt.toml"),
            state: ManifestState::Ok {
                task: "t".to_string(),
                workers: vec![(
                    "w".to_string(),
                    vec!["demo-review".to_string()],
                    Some(vec!["read".to_string(), "bash".to_string()]),
                )],
                tokens: 20,
            },
        };
        let mut state = ManifestDetailState {
            manifest,
            tokens: [("demo-review".to_string(), 20u64)].into_iter().collect(),
        };
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(64, 10)).unwrap();
        let frame = terminal
            .draw(|frame| render_detail(&mut state, frame, frame.area()))
            .unwrap();
        let rendered = crate::tui::snap::frame_to_string(frame.buffer, frame.area);
        assert!(rendered.contains("demo-review  20"), "{rendered}");
    }

    #[test]
    fn detail_lists_worker_tools_when_set() {
        let manifest = DiscoveredManifest {
            name: "rt".to_string(),
            path: std::path::PathBuf::from("lunchbox/manifests/rt.toml"),
            state: ManifestState::Ok {
                task: "t".to_string(),
                workers: vec![(
                    "w".to_string(),
                    vec!["demo-review".to_string()],
                    Some(vec!["read".to_string(), "bash".to_string()]),
                )],
                tokens: 20,
            },
        };
        let mut state = ManifestDetailState {
            manifest,
            tokens: [("demo-review".to_string(), 20u64)].into_iter().collect(),
        };
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(64, 12)).unwrap();
        let frame = terminal
            .draw(|frame| render_detail(&mut state, frame, frame.area()))
            .unwrap();
        let rendered = crate::tui::snap::frame_to_string(frame.buffer, frame.area);
        assert!(rendered.contains("tools read, bash"), "{rendered}");
    }
}
