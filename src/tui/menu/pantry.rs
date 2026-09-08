use crate::config::Config;
use crate::library::ListedSkill;
use crate::tui::menu::Action;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct RootSection {
    pub label: String,
    pub root: PathBuf,
    pub skills: Vec<ListedSkill>,
    pub error: Option<String>,
}

pub struct PantryState {
    pub sections: Vec<RootSection>,
    pub section: usize,
    pub cursor: usize,
    pub selected: Vec<String>,
    pub status: String,
}

pub fn build_sections(cfg: &Config, libraries: &[PathBuf]) -> Result<Vec<RootSection>> {
    let mut sections = Vec::new();
    for root in libraries {
        sections.push(scan_section(root.clone(), "--library".to_string()));
    }
    for root in &cfg.library_paths {
        sections.push(scan_section(root.clone(), "library_paths".to_string()));
    }
    for status in crate::pantry::pantry_statuses()? {
        let label = format!("pantry:{}", status.name);
        match status.state {
            crate::pantry::PantryState::Healthy { root, .. } => {
                sections.push(scan_section(root, label))
            }
            crate::pantry::PantryState::Broken(error) => sections.push(RootSection {
                label,
                root: status.repo,
                skills: Vec::new(),
                error: Some(error),
            }),
        }
    }
    for root in crate::adapter::pantry_base_dirs() {
        sections.push(scan_section(root, "default".to_string()));
    }
    Ok(sections)
}

fn scan_section(root: PathBuf, label: String) -> RootSection {
    match crate::library::scan_roots(std::slice::from_ref(&root)) {
        Ok(skills) => RootSection {
            label,
            root,
            skills,
            error: None,
        },
        Err(error) => RootSection {
            label,
            root,
            skills: Vec::new(),
            error: Some(format!("{error:#}")),
        },
    }
}

pub fn skill_union(sections: &[RootSection]) -> BTreeMap<String, u64> {
    let mut union = BTreeMap::new();
    for section in sections {
        for skill in &section.skills {
            union.entry(skill.name.clone()).or_insert(skill.tokens);
        }
    }
    union
}

impl PantryState {
    pub fn build(cfg: &Config, libraries: &[PathBuf]) -> Result<Self> {
        let sections = build_sections(cfg, libraries)?;
        let section = sections
            .iter()
            .position(|section| section.error.is_none() && !section.skills.is_empty())
            .unwrap_or(0);
        Ok(PantryState {
            sections,
            section,
            cursor: 0,
            selected: Vec::new(),
            status: "compose a pack, then press s".to_string(),
        })
    }

    fn cycle_section(&mut self, delta: i64) {
        if self.sections.is_empty() {
            return;
        }
        let max = self.sections.len() - 1;
        let next = self.section as i64 + delta;
        self.section = next.clamp(0, max as i64) as usize;
        self.cursor = 0;
    }

    fn move_cursor(&mut self, delta: i64) {
        let Some(section) = self.sections.get(self.section) else {
            return;
        };
        if section.skills.is_empty() {
            self.cursor = 0;
            return;
        }
        let max = section.skills.len() - 1;
        let next = self.cursor as i64 + delta;
        self.cursor = next.clamp(0, max as i64) as usize;
    }

    fn toggle(&mut self) {
        let Some(skill) = self
            .sections
            .get(self.section)
            .and_then(|section| section.skills.get(self.cursor))
        else {
            return;
        };
        if let Some(at) = self.selected.iter().position(|name| name == &skill.name) {
            self.selected.remove(at);
        } else {
            self.selected.push(skill.name.clone());
        }
    }
}

pub fn render(state: &mut PantryState, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (index, section) in state.sections.iter().enumerate() {
        let cursor = if index == state.section { "▸ " } else { "  " };
        let line = match &section.error {
            Some(error) => format!("{cursor}{:<28} error: {}", section.label, error),
            None => format!(
                "{cursor}{:<28} {}  {} skills",
                section.label,
                section.root.display(),
                section.skills.len()
            ),
        };
        lines.push(Line::from(line));
    }
    lines.push(Line::from(""));
    match state.sections.get(state.section) {
        None => lines.push(Line::from("no skill roots found")),
        Some(section) => match &section.error {
            Some(error) => lines.push(Line::from(format!("error: {error}"))),
            None => {
                for (index, skill) in section.skills.iter().enumerate() {
                    let cursor = if index == state.cursor { "▸ " } else { "  " };
                    let check = if state.selected.contains(&skill.name) {
                        "[x]"
                    } else {
                        "[ ]"
                    };
                    lines.push(Line::from(format!(
                        "{cursor}{check} {:<16} {}",
                        skill.name, skill.tokens
                    )));
                }
                if section.skills.is_empty() {
                    lines.push(Line::from("no skills in this root"));
                }
            }
        },
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut PantryState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Action::Quit,
        (KeyCode::Esc, _) => Action::Pop,
        (KeyCode::Left, _)
        | (KeyCode::Char('h'), KeyModifiers::NONE)
        | (KeyCode::BackTab, _) => {
            state.cycle_section(-1);
            Action::Continue
        }
        (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Tab, _) => {
            state.cycle_section(1);
            Action::Continue
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.move_cursor(-1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.move_cursor(1);
            Action::Continue
        }
        (KeyCode::Char(' '), _) => {
            state.toggle();
            Action::Continue
        }
        (KeyCode::Char('s'), KeyModifiers::NONE) => {
            if state.selected.is_empty() {
                state.status = "select at least one skill first".to_string();
                Action::Continue
            } else {
                Action::ConfirmFlat(state.selected.clone())
            }
        }
        _ => Action::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::snap::frame_to_string;
    use crate::tui::testkit::demo_tree_home;
    use crossterm::event::KeyEvent;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn state_for(home: &std::path::Path) -> PantryState {
        crate::config::with_home(home, || {
            let cfg = Config::load().unwrap();
            let libraries = vec![home.join(".agents").join("skills")];
            PantryState::build(&cfg, &libraries).unwrap()
        })
    }

    #[test]
    fn pantry_opens_on_first_section_with_skills() {
        let state = state_for(demo_tree_home().path());
        let focused = &state.sections[state.section];
        assert!(!focused.skills.is_empty(), "focused section must show skills");
        assert!(focused.skills.iter().any(|s| s.name == "demo-review"));
    }

    #[test]
    fn toggle_selects_by_name_and_cycles_sections() {
        let mut state = state_for(demo_tree_home().path());
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        assert_eq!(state.selected, vec!["demo-review", "demo-scan"]);
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        assert_eq!(state.selected, vec!["demo-review"], "second toggle removes");

        let before = state.section;
        handle_event(&mut state, &press(KeyCode::Right));
        assert_ne!(state.section, before, "Right must move to another section");
        assert_eq!(state.cursor, 0, "cursor resets on section change");
        handle_event(&mut state, &press(KeyCode::Left));
        assert_eq!(state.section, before);

        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('s'))),
            Action::ConfirmFlat(names) if names == vec!["demo-review"]
        ));
    }

    #[test]
    fn start_without_selection_reports_status() {
        let mut state = state_for(demo_tree_home().path());
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('s'))),
            Action::Continue
        ));
        assert_eq!(state.status, "select at least one skill first");
    }

    #[test]
    fn pantry_screen_two_selected() {
        let mut state = PantryState {
            sections: vec![
                RootSection {
                    label: "--library".to_string(),
                    root: PathBuf::from("/home/user/demo/.agents/skills"),
                    skills: vec![
                        ListedSkill {
                            name: "demo-review".to_string(),
                            tokens: 14,
                        },
                        ListedSkill {
                            name: "demo-scan".to_string(),
                            tokens: 13,
                        },
                    ],
                    error: None,
                },
                RootSection {
                    label: "default".to_string(),
                    root: PathBuf::from("/home/user/demo/project/.agents/skills"),
                    skills: Vec::new(),
                    error: None,
                },
            ],
            section: 0,
            cursor: 1,
            selected: vec!["demo-review".to_string(), "demo-scan".to_string()],
            status: "compose a pack, then press s".to_string(),
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }
}
