use crate::config::{self, LayerReport, List};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub enum Action {
    Continue,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhichLayer {
    Global,
    Project,
}

impl WhichLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            WhichLayer::Global => "global",
            WhichLayer::Project => "project",
        }
    }
}

pub struct PolicyState {
    pub global: LayerReport,
    pub project: LayerReport,
    pub layer: WhichLayer,
    pub list: List,
    pub cursor: usize,
    pub input: String,
    pub input_mode: bool,
    pub status: String,
}

impl PolicyState {
    pub fn load() -> Result<Self> {
        Ok(Self::from_reports(
            config::layer_report(&config::global_path()?)?,
            config::layer_report(&config::project_path())?,
        ))
    }

    pub fn from_reports(global: LayerReport, project: LayerReport) -> Self {
        PolicyState {
            global,
            project,
            layer: WhichLayer::Project,
            list: List::Deny,
            cursor: 0,
            input: String::new(),
            input_mode: false,
            status: "add entries with a".to_string(),
        }
    }

    fn active_report(&self) -> &LayerReport {
        match self.layer {
            WhichLayer::Global => &self.global,
            WhichLayer::Project => &self.project,
        }
    }

    fn active_path(&self) -> &std::path::Path {
        match self.layer {
            WhichLayer::Global => &self.global.path,
            WhichLayer::Project => &self.project.path,
        }
    }

    fn active_entries(&self) -> &Vec<String> {
        let report = self.active_report();
        match self.list {
            List::Allow => &report.allow,
            List::Deny => &report.deny,
        }
    }

    fn clamp_cursor(&mut self) {
        let len = self.active_entries().len();
        self.cursor = self.cursor.min(len.saturating_sub(1));
    }

    fn delete_entry(&mut self) {
        let Some(entry) = self.active_entries().get(self.cursor).cloned() else {
            self.status = "nothing to remove".to_string();
            return;
        };
        let path = self.active_path().to_path_buf();
        match config::remove_layer_entry(&path, self.list, &entry) {
            Ok(()) => {
                self.status = format!(
                    "removed {entry} from {} {}",
                    self.layer.as_str(),
                    self.list.as_str()
                );
                if let Ok(report) = config::layer_report(&path) {
                    match self.layer {
                        WhichLayer::Global => self.global = report,
                        WhichLayer::Project => self.project = report,
                    }
                    self.clamp_cursor();
                }
            }
            Err(error) => self.status = format!("remove failed: {error:#}"),
        }
    }

    fn write_entry(&mut self) {
        let entry = self.input.trim().to_string();
        if entry.is_empty() {
            self.status = "nothing to add".to_string();
            return;
        }
        let path = self.active_path().to_path_buf();
        match config::append_layer_entry(&path, self.list, &entry) {
            Ok(()) => {
                self.status = format!("wrote {}", path.display());
                self.input.clear();
                self.input_mode = false;
                match config::layer_report(&path) {
                    Ok(report) => {
                        match self.layer {
                            WhichLayer::Global => self.global = report,
                            WhichLayer::Project => self.project = report,
                        }
                        self.clamp_cursor();
                    }
                    Err(error) => self.status = format!("reload failed: {error:#}"),
                }
            }
            Err(error) => self.status = format!("write failed: {error:#}"),
        }
    }
}

pub fn render(state: &mut PolicyState, frame: &mut Frame, area: Rect) {
    let [global_area, project_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(area);
    for (report, which, column) in [
        (&state.global, WhichLayer::Global, global_area),
        (&state.project, WhichLayer::Project, project_area),
    ] {
        let active = state.layer == which;
        let header = if active {
            Line::from(Span::styled(
                which.as_str(),
                Style::default().add_modifier(Modifier::BOLD),
            ))
        } else {
            Line::from(which.as_str())
        };
        let mut lines = vec![
            header,
            Line::from(report.path.display().to_string()),
            Line::from(format!("exists  {}", report.exists)),
        ];
        for (list, entries) in [
            (List::Allow, &report.allow),
            (List::Deny, &report.deny),
        ] {
            let label = list.as_str();
            let list_active = active && state.list == list;
            let style = if list_active {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            if entries.is_empty() {
                lines.push(Line::from(Span::styled(format!("{label}   (empty)"), style)));
            } else {
                lines.push(Line::from(Span::styled(format!("{label}:"), style)));
                for (index, entry) in entries.iter().enumerate() {
                    let cursor = if list_active && index == state.cursor {
                        "▸ "
                    } else {
                        "  "
                    };
                    lines.push(Line::from(Span::styled(
                        format!("{cursor}{entry}"),
                        style,
                    )));
                }
            }
        }
        frame.render_widget(Paragraph::new(lines), column);
    }
}

pub fn handle_event(state: &mut PolicyState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    if state.input_mode {
        match key.code {
            KeyCode::Esc => {
                state.input_mode = false;
                state.input.clear();
            }
            KeyCode::Enter => state.write_entry(),
            KeyCode::Backspace => {
                state.input.pop();
            }
            KeyCode::Char(c) => state.input.push(c),
            _ => {}
        }
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Tab, _) => {
            state.layer = match state.layer {
                WhichLayer::Global => WhichLayer::Project,
                WhichLayer::Project => WhichLayer::Global,
            };
            state.clamp_cursor();
            Action::Continue
        }
        (KeyCode::BackTab, _) => {
            state.layer = match state.layer {
                WhichLayer::Global => WhichLayer::Project,
                WhichLayer::Project => WhichLayer::Global,
            };
            state.clamp_cursor();
            Action::Continue
        }
        (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
            state.list = List::Allow;
            state.clamp_cursor();
            Action::Continue
        }
        (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
            state.list = List::Deny;
            state.clamp_cursor();
            Action::Continue
        }
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.cursor = state.cursor.saturating_sub(1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            let len = state.active_entries().len();
            state.cursor = (state.cursor + 1).min(len.saturating_sub(1));
            Action::Continue
        }
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            state.input_mode = true;
            state.input.clear();
            Action::Continue
        }
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            state.delete_entry();
            Action::Continue
        }
        _ => Action::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::snap::frame_to_string;
    use crossterm::event::KeyEvent;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::{Path, PathBuf};

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn press_char(c: char) -> Event {
        Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::NONE,
        ))
    }

    const GLOBAL_TOML: &str = "# global layer\n# managed by hand\nmax_menu_tokens = 4000\n\ndeny = [\"old-thing\"]\n";
    const PROJECT_TOML: &str = "# project layer\nmount_mode = \"copy\"\n\n# policy below\nallow = [\"demo-review\"]\n";

    fn layers(root: &Path) -> (PathBuf, PathBuf) {
        let global = root.join("config.toml");
        let project = root.join("lunchbox.toml");
        std::fs::write(&global, GLOBAL_TOML).unwrap();
        std::fs::write(&project, PROJECT_TOML).unwrap();
        (global, project)
    }

    fn state_for(root: &Path) -> PolicyState {
        let (global, project) = layers(root);
        let global = config::layer_report(&global).unwrap();
        let project = config::layer_report(&project).unwrap();
        PolicyState::from_reports(global, project)
    }

    #[test]
    fn delete_removes_focused_entry_and_preserves_comments() {
        let root = tempfile::TempDir::new().unwrap();
        let mut state = state_for(root.path());
        handle_event(&mut state, &press_char('d'));
        assert_eq!(state.status, "nothing to remove", "empty deny list must not delete");
        handle_event(&mut state, &press(KeyCode::Left));
        handle_event(&mut state, &press_char('d'));
        assert!(
            state.status.contains("removed demo-review from project allow"),
            "{}",
            state.status
        );
        assert!(state.project.allow.is_empty(), "report must reload after delete");
        let text = std::fs::read_to_string(root.path().join("lunchbox.toml")).unwrap();
        assert!(text.contains("# project layer"), "{text}");
        assert!(text.contains("mount_mode = \"copy\""), "{text}");
        assert!(!text.contains("demo-review"), "{text}");
    }

    #[test]
    fn add_deny_on_project_layer_preserves_format() {
        let root = tempfile::TempDir::new().unwrap();
        let (global_path, project_path) = layers(root.path());
        let global_before = std::fs::read_to_string(&global_path).unwrap();
        let mut state = state_for(root.path());
        assert_eq!(state.list, List::Deny);

        handle_event(&mut state, &press(KeyCode::Char('a')));
        assert!(state.input_mode);
        for c in "demo-scan".chars() {
            handle_event(&mut state, &press_char(c));
        }
        handle_event(&mut state, &press(KeyCode::Enter));

        assert!(!state.input_mode);
        assert_eq!(state.status, format!("wrote {}", project_path.display()));
        let project_after = std::fs::read_to_string(&project_path).unwrap();
        assert!(
            project_after.contains("# project layer"),
            "comments must survive: {project_after}"
        );
        assert!(
            project_after.contains("mount_mode = \"copy\""),
            "unrelated keys must survive: {project_after}"
        );
        assert!(
            project_after.contains("# policy below"),
            "inline comments must survive: {project_after}"
        );
        assert!(
            project_after.contains("deny = [\"demo-scan\"]"),
            "new deny entry must land in the project layer: {project_after}"
        );
        let global_after = std::fs::read_to_string(&global_path).unwrap();
        assert_eq!(global_before, global_after, "global layer untouched");
        assert_eq!(state.project.deny, vec!["demo-scan"]);
    }

    #[test]
    fn duplicate_entry_is_a_no_op() {
        let root = tempfile::TempDir::new().unwrap();
        let mut state = state_for(root.path());
        handle_event(&mut state, &press(KeyCode::Tab));
        assert_eq!(state.layer, WhichLayer::Global);
        handle_event(&mut state, &press(KeyCode::Char('a')));
        for c in "old-thing".chars() {
            handle_event(&mut state, &press_char(c));
        }
        handle_event(&mut state, &press(KeyCode::Enter));
        let global = std::fs::read_to_string(root.path().join("config.toml")).unwrap();
        assert_eq!(global, GLOBAL_TOML, "already-present entry must not duplicate");
    }

    #[test]
    fn esc_exits_input_mode_before_quit() {
        let root = tempfile::TempDir::new().unwrap();
        let mut state = state_for(root.path());
        handle_event(&mut state, &press(KeyCode::Char('a')));
        assert!(state.input_mode);
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Esc)),
            Action::Continue
        ));
        assert!(!state.input_mode);
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Esc)),
            Action::Quit
        ));
    }

    #[test]
    fn policy_screen_both_layers() {
        let global = LayerReport {
            path: PathBuf::from("/home/w/.lunchbox/config.toml"),
            exists: true,
            allow: vec![],
            deny: vec!["old-thing".to_string()],
        };
        let project = LayerReport {
            path: PathBuf::from("./lunchbox.toml"),
            exists: true,
            allow: vec!["demo-review".to_string()],
            deny: vec![],
        };
        let mut state = PolicyState::from_reports(global, project);
        let mut terminal = Terminal::new(TestBackend::new(72, 12)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }
}
