use crate::library::ListedSkill;
use crate::tui::terminal::{self, Restore};
use crate::run::{PreparedRun, prepare_run};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub enum Action {
    Continue,
    Quit,
    StartRun,
    FinishRun,
}

pub struct PickerState {
    pub cfg: crate::config::Config,
    pub libraries: Vec<std::path::PathBuf>,
    pub library: Vec<ListedSkill>,
    pub selected: Vec<bool>,
    pub cursor: usize,
    pub run: Option<PreparedRun>,
    pub status: String,
}

impl PickerState {
    pub fn new(
        cfg: crate::config::Config,
        libraries: Vec<std::path::PathBuf>,
        library: Vec<ListedSkill>,
    ) -> Self {
        let selected = vec![false; library.len()];
        PickerState {
            cfg,
            libraries,
            library,
            selected,
            cursor: 0,
            run: None,
            status: "compose a pack, then press s".to_string(),
        }
    }

    fn selected_names(&self) -> Vec<String> {
        self.library
            .iter()
            .zip(&self.selected)
            .filter(|(_, selected)| **selected)
            .map(|(skill, _)| skill.name.clone())
            .collect()
    }

    fn move_cursor(&mut self, delta: i64) {
        if self.library.is_empty() {
            return;
        }
        let max = self.library.len() - 1;
        let next = self.cursor as i64 + delta;
        self.cursor = next.clamp(0, max as i64) as usize;
    }

    fn start_run(&mut self) -> Action {
        if self.run.is_some() {
            self.status = "a run is already mounted; press f to finish it".to_string();
            return Action::Continue;
        }
        let names = self.selected_names();
        if names.is_empty() {
            self.status = "select at least one skill first".to_string();
            return Action::Continue;
        }
        let workers = vec![crate::run::Worker {
            name: "default".to_string(),
            pack: names,
            description: None,
        }];
        match prepare_run(
            &self.cfg,
            "none",
            "tui picker run",
            workers,
            self.cfg.max_menu_tokens,
            &self.libraries,
            &[],
            false,
        ) {
            Ok(run) => {
                self.status = format!("run {} mounted ({})", run.run_id, run.mount_mode.as_str());
                self.run = Some(run);
                Action::StartRun
            }
            Err(error) => {
                self.status = format!("start failed: {error:#}");
                Action::Continue
            }
        }
    }

    fn finish_run(&mut self) -> Action {
        let Some(run) = self.run.take() else {
            self.status = "no live run".to_string();
            return Action::Continue;
        };
        match crate::run::teardown(&run.run_dir, crate::run::Outcome::Ok, &self.cfg) {
            Ok(()) => {
                self.status = "unmounted — result.json written".to_string();
                Action::FinishRun
            }
            Err(error) => {
                self.status = format!("finish failed: {error:#}");
                self.run = Some(run);
                Action::Continue
            }
        }
    }

}

pub fn render(state: &mut PickerState, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    if let Some(run) = &state.run {
        for skill in &run.locked {
            lines.push(Line::from(format!("{}@{}", skill.name, skill.hash)));
        }
    } else {
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
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut PickerState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => {
            if state.run.is_some() {
                state.finish_run();
            }
            Action::Quit
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
            if let Some(flag) = state.selected.get_mut(state.cursor) {
                *flag = !*flag;
            }
            Action::Continue
        }
        (KeyCode::Char('s'), KeyModifiers::NONE) => state.start_run(),
        (KeyCode::Char('f'), KeyModifiers::NONE) => state.finish_run(),
        _ => Action::Continue,
    }
}

pub fn run(mut state: PickerState) -> Result<()> {
    let mut terminal = terminal::install()?;
    let _restore = Restore;
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let body = Rect {
                height: area.height.saturating_sub(2),
                ..area
            };
            render(&mut state, frame, body);
            let status = Rect {
                y: body.bottom(),
                height: 1,
                ..area
            };
            frame.render_widget(Paragraph::new(state.status.clone()), status);
            let footer = Rect {
                y: status.bottom(),
                height: 1,
                ..area
            };
            frame.render_widget(
                Paragraph::new(Line::from(
                    "↑/↓ move · Space toggle · s start · f finish · q quit · spawn: use lunchbox start --adapter pi …",
                )),
                footer,
            );
        })?;
        let event = terminal::next_event()?;
        if let Action::Quit = handle_event(&mut state, &event) {
            return match state.run.take() {
                None => Ok(()),
                Some(run) => Err(anyhow::anyhow!(
                    "run {} still mounted at {}; finish failed — recover with lunchbox finish {}",
                    run.run_id,
                    run.run_dir.display(),
                    run.run_id
                )),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::library;
    use crate::tui::snap::frame_to_string;
    use crate::tui::testkit::demo_tree_home;
    use crossterm::event::KeyEvent;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::os::unix::fs::PermissionsExt;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn state_for(home: &std::path::Path) -> PickerState {
        crate::config::with_home(home, || {
            let cfg = Config::load().unwrap();
            let libraries = vec![home.join(".agents").join("skills")];
            let listing = library::scan_roots(&cfg.search_roots(&libraries)).unwrap();
            PickerState::new(cfg, libraries, listing)
        })
    }

    #[test]
    fn picker_start_mounts_and_finish_unmounts() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(home.path().join(".lunchbox")).unwrap();
        std::fs::write(
            home.path().join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.path().display()),
        )
        .unwrap();
        let mut state = state_for(home.path());

        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        assert_eq!(state.selected_names(), vec!["demo-review", "demo-scan"]);

        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('s'))),
            Action::StartRun
        ));
        let run_dirs: Vec<_> = std::fs::read_dir(runs.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        assert_eq!(run_dirs.len(), 1, "exactly one run dir");
        let workdir = run_dirs[0].join("workdir");
        let mut entries: Vec<String> = std::fs::read_dir(&workdir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        assert_eq!(entries, vec!["demo-review", "demo-scan"]);

        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('f'))),
            Action::FinishRun
        ));
        assert!(!workdir.exists(), "workdir gone after finish");
        let result: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(run_dirs[0].join("result.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(result["unmounted"], serde_json::json!(true));

        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('q'))),
            Action::Quit
        ));
    }

    #[test]
    fn quit_with_mounted_run_implies_finish() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(home.path().join(".lunchbox")).unwrap();
        std::fs::write(
            home.path().join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.path().display()),
        )
        .unwrap();
        let mut state = state_for(home.path());
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Char('s')));
        let run_dir = state.run.as_ref().unwrap().run_dir.clone();
        assert!(run_dir.join("workdir").exists());
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('q'))),
            Action::Quit
        ));
        assert!(!run_dir.join("workdir").exists(), "quit must not leak a mount");
        assert!(run_dir.join("result.json").exists());
    }

    #[test]
    fn failed_finish_keeps_run_recoverable_and_quit_fails_closed() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(home.path().join(".lunchbox")).unwrap();
        std::fs::write(
            home.path().join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.path().display()),
        )
        .unwrap();
        let mut state = state_for(home.path());
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Char('s')));
        let run_dir = state.run.as_ref().unwrap().run_dir.clone();
        assert!(run_dir.join("workdir").exists());

        std::fs::set_permissions(&run_dir, PermissionsExt::from_mode(0o500)).unwrap();
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('f'))),
            Action::Continue
        ));
        assert!(state.run.is_some(), "failed finish must keep the run handle");
        assert!(state.status.contains("finish failed"));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('s'))),
            Action::Continue
        ));
        assert!(
            state.status.contains("already mounted"),
            "a stuck run must block starting another: {}",
            state.status
        );
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('q'))),
            Action::Quit
        ));
        assert!(
            state.run.is_some(),
            "quit must not discard a run it failed to unmount"
        );

        std::fs::set_permissions(&run_dir, PermissionsExt::from_mode(0o700)).unwrap();
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Char('f'))),
            Action::FinishRun
        ));
        assert!(state.run.is_none());
        assert!(!run_dir.join("workdir").exists());
        assert!(run_dir.join("result.json").exists());
    }

    #[test]
    fn picker_screen_two_selected() {
        let home = demo_tree_home();
        let mut state = state_for(home.path());
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        handle_event(&mut state, &press(KeyCode::Down));
        handle_event(&mut state, &press(KeyCode::Char(' ')));
        let mut terminal = Terminal::new(TestBackend::new(72, 6)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }
}
