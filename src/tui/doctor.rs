use crate::adapter::DoctorReport;
use crate::tui::terminal::{self, Restore};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

const UNION_WINDOW: usize = 10;

pub enum Action {
    Continue,
    Quit,
}

pub struct DoctorState {
    pub report: DoctorReport,
    pub scroll: usize,
}

impl DoctorState {
    pub fn new(report: DoctorReport) -> Self {
        DoctorState { report, scroll: 0 }
    }

    fn max_scroll(&self) -> usize {
        self.report.union.len().saturating_sub(UNION_WINDOW)
    }
}

pub fn render(state: &mut DoctorState, frame: &mut Frame, area: Rect) {
    let report = &state.report;
    let mut lines = Vec::new();
    match &report.adapter_version {
        Some(version) => lines.push(Line::from(format!(
            "lunchbox doctor — {} [{}]",
            report.adapter, version
        ))),
        None => lines.push(Line::from(format!(
            "lunchbox doctor — {} (not detected)",
            report.adapter
        ))),
    }
    match &report.git_version {
        Some(version) => lines.push(Line::from(format!("git [{version}]"))),
        None => lines.push(Line::from("git (not found)")),
    }
    for dir in &report.dirs {
        let state = if dir.exists {
            format!("{} skills", dir.skills.len())
        } else {
            "absent".to_string()
        };
        lines.push(Line::from(format!("{}  {}", dir.dir.display(), state)));
    }
    if !report.pantries.is_empty() {
        lines.push(Line::from("pantries:"));
        for pantry in &report.pantries {
            lines.push(Line::from(pantry.summary_line()));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!(
        "menu_tokens  {}  ({} skills union)",
        report.menu_tokens,
        report.union.len()
    )));
    let fattest = report.fattest();
    let window: Vec<String> = report
        .union
        .iter()
        .skip(state.scroll)
        .take(UNION_WINDOW)
        .map(|skill| {
            let marker = if fattest.iter().any(|f| f.name == skill.name) {
                "* "
            } else {
                "  "
            };
            format!("{}{:<16} {}", marker, skill.name, skill.tokens)
        })
        .collect();
    lines.extend(window.iter().map(|line| {
        Line::from(Span::styled(
            line.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ))
    }));
    if report.union.is_empty() {
        lines.push(Line::from("               no skills found in adapter skill dirs"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("↑/↓ scroll · Home/End jump · q quit"));
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut DoctorState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.scroll = state.scroll.saturating_sub(1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.scroll = (state.scroll + 1).min(state.max_scroll());
            Action::Continue
        }
        (KeyCode::Home, _) => {
            state.scroll = 0;
            Action::Continue
        }
        (KeyCode::End, _) => {
            state.scroll = state.max_scroll();
            Action::Continue
        }
        _ => Action::Continue,
    }
}

pub fn run(report: DoctorReport) -> Result<()> {
    let mut terminal = terminal::install()?;
    let _restore = Restore;
    let mut state = DoctorState::new(report);
    loop {
        terminal.draw(|frame| render(&mut state, frame, frame.area()))?;
        let event = terminal::next_event()?;
        if let Action::Quit = handle_event(&mut state, &event) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{DirReport, FoundDirSkill};
    use crate::tui::snap::frame_to_string;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use serde_json::json;
    use std::path::PathBuf;

    fn skill(name: &str, tokens: u64) -> FoundDirSkill {
        FoundDirSkill {
            name: name.to_string(),
            dir: PathBuf::from("/home/w/.agents/skills"),
            tokens,
        }
    }

    fn two_skill_report() -> DoctorReport {
        let skills = vec![skill("demo-review", 14), skill("demo-scan", 13)];
        DoctorReport {
            adapter: "pi".to_string(),
            adapter_version: Some("0.84.4".to_string()),
            git_version: Some("2.99.0".to_string()),
            dirs: vec![DirReport {
                dir: PathBuf::from("/home/w/.agents/skills"),
                exists: true,
                skills: skills.clone(),
            }],
            pantries: Vec::new(),
            menu_tokens: 27,
            union: skills,
        }
    }
    #[test]
    fn doctor_screen_two_skills() {
        let mut state = DoctorState::new(two_skill_report());
        let mut terminal = Terminal::new(TestBackend::new(64, 10)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }

    #[test]
    fn displayed_tokens_equal_doctor_json() {
        let state = DoctorState::new(two_skill_report());
        let twin = state.report.to_json();
        assert_eq!(twin["menu_tokens"], json!(27));
        assert_eq!(twin["fattest"][0]["name"], json!("demo-review"));
        assert_eq!(twin["fattest"][0]["tokens"], json!(14));
        assert_eq!(twin["skill_dirs"][0]["skills"], json!(2));
    }

    #[test]
    fn scroll_clamps_at_union_window() {
        let mut state = DoctorState::new(two_skill_report());
        for _ in 0..5 {
            handle_event(
                &mut state,
                &Event::Key(crossterm::event::KeyEvent::from(KeyCode::Down)),
            );
        }
        assert_eq!(state.scroll, 0, "2 skills fit one window; scroll must clamp");
        handle_event(
            &mut state,
            &Event::Key(crossterm::event::KeyEvent::from(KeyCode::End)),
        );
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn quit_keys_quit_and_others_continue() {
        let mut state = DoctorState::new(two_skill_report());
        assert!(matches!(
            handle_event(
                &mut state,
                &Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('q')))
            ),
            Action::Quit
        ));
        assert!(matches!(
            handle_event(
                &mut state,
                &Event::Key(crossterm::event::KeyEvent::from(KeyCode::Esc))
            ),
            Action::Quit
        ));
        assert!(matches!(
            handle_event(&mut state, &Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab))),
            Action::Continue
        ));
    }
}
