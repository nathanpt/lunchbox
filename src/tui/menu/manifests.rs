use crate::config::Config;
use crate::manifests::{DiscoveredManifest, ManifestState};
use crate::tui::menu::Action;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct ManifestsState {
    pub manifests: Vec<DiscoveredManifest>,
    pub cursor: usize,
}

impl ManifestsState {
    pub fn build(cfg: &Config, libraries: &[std::path::PathBuf]) -> Result<Self> {
        let sections = super::pantry::build_sections(cfg, libraries)?;
        let skills = super::pantry::skill_union(&sections);
        Ok(ManifestsState {
            manifests: crate::manifests::discover(&skills),
            cursor: 0,
        })
    }
}

pub struct ManifestDetailState {
    pub manifest: DiscoveredManifest,
}

pub fn render(state: &mut ManifestsState, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (index, manifest) in state.manifests.iter().enumerate() {
        let cursor = if index == state.cursor { "▸ " } else { "  " };
        let line = match &manifest.state {
            ManifestState::Ok {
                task,
                workers,
                tokens,
            } => format!(
                "{cursor}{:<20} {:<24} {} workers  ~{} tokens",
                manifest.name,
                task,
                workers.len(),
                tokens
            ),
            ManifestState::Broken(error) => format!("{cursor}{:<20} error: {}", manifest.name, error),
        };
        lines.push(Line::from(line));
    }
    if state.manifests.is_empty() {
        lines.push(Line::from(
            "no manifests found (looked in ./lunchbox/manifests and ~/.lunchbox/manifests)",
        ));
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
    let mut lines = vec![
        Line::from(format!("manifest       {}", manifest.name)),
        Line::from(format!("path           {}", manifest.path.display())),
    ];
    match &manifest.state {
        ManifestState::Ok {
            task,
            workers,
            tokens,
        } => {
            lines.push(Line::from(format!("task           {task}")));
            lines.push(Line::from(format!("menu_tokens    ~{tokens}")));
            lines.push(Line::from(""));
            for (name, pack) in workers {
                lines.push(Line::from(format!("worker         {name}")));
                for pin in pack {
                    lines.push(Line::from(format!("  - {pin}")));
                }
            }
        }
        ManifestState::Broken(error) => {
            lines.push(Line::from(""));
            lines.push(Line::from(format!("error: {error}")));
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
