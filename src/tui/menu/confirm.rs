use crate::config::Config;
use crate::resolve::Locked;
use crate::run;
use crate::tui::menu::{chrome, Action};
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;

#[derive(Clone)]
pub enum Source {
    Flat(Vec<String>),
    Manifest(run::ManifestInput),
}

pub struct ConfirmState {
    pub source: Source,
    pub resolved: Result<Vec<Locked>, String>,
    pub task: String,
}

impl ConfirmState {
    pub fn new(source: Source, cfg: &Config, libraries: &[PathBuf]) -> Self {
        let pins = source_pins(&source);
        let task = match &source {
            Source::Flat(_) => "menu run".to_string(),
            Source::Manifest(input) => input.task.clone(),
        };
        let resolved = (|| -> anyhow::Result<Vec<Locked>> {
            if let Source::Manifest(input) = &source {
                input.validate()?;
            }
            crate::resolve::resolve(&pins, cfg, libraries, false)
        })()
        .map_err(|error| format!("{error:#}"));
        ConfirmState {
            source,
            resolved,
            task,
        }
    }
}

pub fn source_pins(source: &Source) -> Vec<String> {
    match source {
        Source::Flat(names) => names.clone(),
        Source::Manifest(input) => {
            let mut pins: Vec<String> = Vec::new();
            for worker in &input.workers {
                for pin in &worker.pack {
                    if !pins.contains(pin) {
                        pins.push(pin.clone());
                    }
                }
            }
            pins
        }
    }
}

pub fn render(state: &mut ConfirmState, frame: &mut Frame, area: Rect) {
    let mut lines = vec![Line::from(
        "adapter        none — mounted, not spawned",
    )];
    match &state.resolved {
        Ok(locked) => {
            for skill in locked {
                lines.push(Line::from(format!(
                    "skills         {}@{}",
                    skill.name, skill.hash
                )));
            }
            let tokens: u64 = crate::tokens::with_preamble(
                locked.iter().map(|skill| skill.description_tokens).sum(),
            );
            lines.push(Line::from(format!(
                "menu_tokens    this run: {tokens}"
            )));
            lines.push(Line::from(format!("task           {}", state.task)));
        }
        Err(error) => {
            lines.push(Line::from(""));
            lines.push(Line::from(chrome::bad(format!("resolve error: {error}"))));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(chrome::bold("Enter mount · Esc cancel")));
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut ConfirmState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
        KeyCode::Enter => Action::Mount {
            source: state.source.clone(),
            task: state.task.clone(),
        },
        _ => Action::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::snap::frame_to_string;
    use crossterm::event::KeyEvent;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn config_for(home: &std::path::Path) -> (tempfile::TempDir, Config) {
        let runs = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(home.join(".lunchbox")).unwrap();
        std::fs::write(
            home.join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.path().display()),
        )
        .unwrap();
        let cfg = crate::config::with_home(home, || Config::load().unwrap());
        (runs, cfg)
    }

    #[test]
    fn confirm_screen_flat_pack() {
        let home = crate::tui::testkit::demo_tree_home();
        let (_runs, cfg) = config_for(home.path());
        let libraries = vec![home.path().join(".agents").join("skills")];
        let mut state = ConfirmState::new(
            Source::Flat(vec!["demo-review".to_string(), "demo-scan".to_string()]),
            &cfg,
            &libraries,
        );
        assert!(state.resolved.is_ok(), "{:?}", state.resolved);
        let mut terminal = Terminal::new(TestBackend::new(72, 10)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }

    #[test]
    fn confirm_reports_resolve_errors_instead_of_mounting_blindly() {
        let home = crate::tui::testkit::demo_tree_home();
        let (_runs, cfg) = config_for(home.path());
        let libraries = vec![home.path().join(".agents").join("skills")];
        let mut state = ConfirmState::new(
            Source::Flat(vec!["no-such-skill".to_string()]),
            &cfg,
            &libraries,
        );
        let Err(error) = &state.resolved else {
            panic!("expected resolve failure");
        };
        assert!(error.contains("no-such-skill"), "{error}");
        let mut terminal = Terminal::new(TestBackend::new(72, 10)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        let rendered = frame_to_string(frame.buffer, frame.area);
        assert!(rendered.contains("resolve error:"), "{rendered}");
    }

    #[test]
    fn enter_emits_mount_and_esc_pops() {
        let home = crate::tui::testkit::demo_tree_home();
        let (_runs, cfg) = config_for(home.path());
        let libraries = vec![home.path().join(".agents").join("skills")];
        let mut state = ConfirmState::new(
            Source::Flat(vec!["demo-review".to_string()]),
            &cfg,
            &libraries,
        );
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Enter)),
            Action::Mount { .. }
        ));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Esc)),
            Action::Pop
        ));
    }
}
