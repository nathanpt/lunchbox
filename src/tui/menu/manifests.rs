use crate::config::Config;
use crate::manifests::{DiscoveredManifest, ManifestState};
use crate::tui::menu::{chrome, Action};
use anyhow::Result;
use std::collections::BTreeMap;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

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
            manifests: crate::manifests::discover(&skills),
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
        } => {
            lines.push(Line::from(vec![label("task"), Span::raw(task.clone())]));
            lines.push(Line::from(vec![
                label("menu_tokens"),
                Span::raw(format!("~{tokens}")),
            ]));
            lines.push(Line::from(""));
            for (name, pack) in workers {
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

    #[test]
    fn detail_lists_per_pin_tokens() {
        let manifest = DiscoveredManifest {
            name: "rt".to_string(),
            path: std::path::PathBuf::from("lunchbox/manifests/rt.toml"),
            state: ManifestState::Ok {
                task: "t".to_string(),
                workers: vec![("w".to_string(), vec!["demo-review".to_string()])],
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
}
