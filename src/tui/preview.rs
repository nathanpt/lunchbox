use crate::library::ListedSkill;
use crate::tui::terminal::{self, Restore};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub enum Action {
    Continue,
    Quit,
}

pub struct PreviewState {
    pub library: Vec<ListedSkill>,
    pub selected: Vec<bool>,
    pub cursor: usize,
    pub without_menu_tokens: u64,
    pub max_menu_tokens: u64,
}

impl PreviewState {
    pub fn new(
        library: Vec<ListedSkill>,
        without_menu_tokens: u64,
        max_menu_tokens: u64,
    ) -> Self {
        let selected = vec![false; library.len()];
        PreviewState {
            library,
            selected,
            cursor: 0,
            without_menu_tokens,
            max_menu_tokens,
        }
    }

    pub fn selected_tokens(&self) -> u64 {
        self.library
            .iter()
            .zip(&self.selected)
            .filter(|(_, selected)| **selected)
            .map(|(skill, _)| skill.tokens)
            .sum()
    }
    pub fn preselect(&mut self, names: &[String]) {
        for (skill, selected) in self.library.iter_mut().zip(&mut self.selected) {
            *selected = names.iter().any(|name| name == &skill.name);
        }
    }

    fn over_budget(&self) -> bool {
        self.selected_tokens() > self.max_menu_tokens
    }

    fn move_cursor(&mut self, delta: i64) {
        if self.library.is_empty() {
            return;
        }
        let max = self.library.len() - 1;
        let next = self.cursor as i64 + delta;
        self.cursor = next.clamp(0, max as i64) as usize;
    }
}

pub fn render(state: &mut PreviewState, frame: &mut Frame, area: Rect) {
    let [library_area, stats_area] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .areas(area);
    let mut lines = Vec::new();
    for (index, skill) in state.library.iter().enumerate() {
        let cursor = if index == state.cursor { "▸ " } else { "  " };
        let check = if state.selected[index] { "[x]" } else { "[ ]" };
        lines.push(Line::from(format!(
            "{cursor}{check} {:<16} {}",
            skill.name, skill.tokens
        )));
    }
    if state.library.is_empty() {
        lines.push(Line::from("no skills found in library roots"));
    }
    frame.render_widget(Paragraph::new(lines), library_area);

    let this_run = state.selected_tokens();
    let without = state.without_menu_tokens;
    let delta = without as i64 - this_run as i64;
    let budget = if state.over_budget() { "OVER" } else { "ok" };
    let stats = vec![
        Line::from(format!("this run  {this_run}")),
        Line::from(format!("without   {without}")),
        Line::from(format!("delta     {delta}")),
        Line::from(format!("budget    max {}  {budget}", state.max_menu_tokens)),
    ];
    frame.render_widget(
        Paragraph::new(stats).style(Style::new()),
        stats_area,
    );
}

pub fn handle_event(state: &mut PreviewState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.move_cursor(-1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.move_cursor(1);
            Action::Continue
        }
        (KeyCode::Char(' '), _) => {
            if let Some(flag) = state.selected.get_mut(state.cursor) {
                *flag = !*flag;
            }
            Action::Continue
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            for flag in &mut state.selected {
                *flag = true;
            }
            Action::Continue
        }
        (KeyCode::Char('n'), KeyModifiers::NONE) => {
            for flag in &mut state.selected {
                *flag = false;
            }
            Action::Continue
        }
        _ => Action::Continue,
    }
}

pub fn run(mut state: PreviewState) -> Result<()> {
    let mut terminal = terminal::install()?;
    let _restore = Restore;
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            render(&mut state, frame, area);
            let footer = Rect {
                y: area.bottom().saturating_sub(1),
                height: 1,
                ..area
            };
            frame.render_widget(
                Paragraph::new(Line::from(
                    "↑/↓ move · Space toggle · a all · n none · q quit",
                )),
                footer,
            );
        })?;
        let event = terminal::next_event()?;
        if let Action::Quit = handle_event(&mut state, &event) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library;
    use crate::tui::snap::frame_to_string;
    use crate::tokens;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn listed(name: &str, tokens: u64) -> ListedSkill {
        ListedSkill {
            name: name.to_string(),
            source: PathBuf::from("/home/w/.agents/skills"),
            tokens,
        }
    }

    fn two_skill_state() -> PreviewState {
        let mut state = PreviewState::new(vec![listed("demo-review", 14), listed("demo-scan", 13)], 27, 2000);
        for flag in &mut state.selected {
            *flag = true;
        }
        state
    }

    #[test]
    fn preview_screen_selected_pack() {
        let mut state = two_skill_state();
        let mut terminal = Terminal::new(TestBackend::new(64, 8)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }

    #[test]
    fn selected_tokens_equal_cli_preview() {
        let home = crate::tui::testkit::demo_tree_home();
        crate::config::with_home(home.path(), || {
            let cfg = crate::config::Config::load().unwrap();
            let listing = library::scan_roots(&cfg.search_roots(&[])).unwrap();
            let pins = vec![
                "demo-review".to_string(),
                "demo-scan".to_string(),
            ];
            let mut state = PreviewState::new(listing, 27, 2000);
            state.preselect(&pins);
            assert_eq!(
                state.selected_tokens(),
                tokens::preview(&cfg, &[], &pins).unwrap().menu_tokens
            );
        });
    }

    #[test]
    fn preselect_marks_only_pinned_skills() {
        let mut state = PreviewState::new(
            vec![listed("demo-review", 14), listed("demo-scan", 13)],
            27,
            2000,
        );
        state.preselect(&["demo-review".to_string()]);
        assert_eq!(state.selected, vec![true, false]);
        assert_eq!(state.selected_tokens(), 14);
    }

    #[test]
    fn budget_flips_to_over() {
        let state = PreviewState::new(vec![listed("demo-review", 14)], 0, 10);
        assert!(!state.over_budget());
        let mut state = state;
        state.selected[0] = true;
        assert!(state.over_budget());
    }

    #[test]
    fn space_toggles_and_cursor_clamps() {
        let mut state = PreviewState::new(vec![listed("demo-review", 14), listed("demo-scan", 13)], 0, 2000);
        let press = |code: KeyCode| {
            Event::Key(crossterm::event::KeyEvent::from(code))
        };
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        assert!(state.selected[0]);
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Down));
        assert_eq!(state.cursor, 1, "cursor clamps at last entry");
        handle_event(&mut state, &press(KeyCode::Up));
        handle_event(&mut state, &press(KeyCode::Up));
        handle_event(&mut state, &press(KeyCode::Up));
        assert_eq!(state.cursor, 0, "cursor clamps at first entry");
        handle_event(&mut state, &press(KeyCode::Char('n')));
        assert!(state.selected.iter().all(|f| !f));
        handle_event(&mut state, &press(KeyCode::Char('a')));
        assert!(state.selected.iter().all(|f| *f));
        assert_eq!(state.selected_tokens(), 27);
    }
}
