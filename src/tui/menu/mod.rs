use crate::config::Config;
use crate::run::{self, Outcome, PreparedRun};
use crate::tui::terminal::{self, Restore};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::path::PathBuf;

pub mod chrome;
pub mod confirm;
pub mod editor;
pub mod home;
pub mod manifests;
pub mod pantry;

use confirm::ConfirmState;
use home::HomeState;
use manifests::{ManifestDetailState, ManifestsState};
use pantry::PantryState;

pub enum Action {
    Continue,
    Pop,
    Open(home::Route),
    ConfirmFlat(Vec<String>),
    Detail(manifests::DiscoveredManifest),
    Mount {
        source: confirm::Source,
        task: String,
    },
    Quit,
    EditManifest(manifests::DiscoveredManifest),
    NewEditor,
    EditorSave,
    EditorSaveAndStart,
    Finish,
}

pub enum Layer {
    Home(HomeState),
    Pantry(PantryState),
    Manifests(ManifestsState),
    ManifestDetail(ManifestDetailState),
    Doctor(crate::tui::doctor::DoctorState),
    Policy(crate::tui::policy::PolicyState),
    Confirm(ConfirmState),
    Editor(editor::EditorState),
}

pub struct MenuState {
    pub cfg: Config,
    pub libraries: Vec<PathBuf>,
    pub stack: Vec<Layer>,
    pub live: Option<PreparedRun>,
    pub status: String,
}

impl MenuState {
    pub fn new(cfg: Config, libraries: Vec<PathBuf>) -> Self {
        MenuState {
            cfg,
            libraries,
            stack: vec![Layer::Home(HomeState::default())],
            live: None,
            status: "no live run".to_string(),
        }
    }

    fn finish_live(&mut self) {
        let Some(live) = self.live.take() else {
            self.status = "no live run".to_string();
            return;
        };
        match run::teardown(&live.run_dir, Outcome::Ok, &self.cfg) {
            Ok(()) => self.status = "unmounted — result.json written".to_string(),
            Err(error) => {
                self.status = format!("finish failed: {error:#}");
                self.live = Some(live);
            }
        }
    }

    fn mount(&mut self, source: confirm::Source, task: &str) -> bool {
        if self.live.is_some() {
            self.status = "a run is already mounted; press f to finish it".to_string();
            return false;
        }
        let (workers, use_packs) = match &source {
            confirm::Source::Flat(names) => (vec![run::Worker::default_pack(names.clone())], false),
            confirm::Source::Manifest(input) => (input.workers.clone(), true),
        };
        match run::prepare_run(
            &self.cfg,
            "none",
            task,
            workers,
            self.cfg.max_menu_tokens,
            &self.libraries,
            &[],
            use_packs,
            false,
        ) {
            Ok(prepared) => {
                self.status = format!(
                    "run {} mounted ({})",
                    prepared.run_id,
                    prepared.mount_mode.as_str()
                );
                self.live = Some(prepared);
                true
            }
            Err(error) => {
                self.status = format!("start failed: {error:#}");
                false
            }
        }
    }

    fn quit(&mut self) -> Result<()> {
        match self.live.take() {
            None => Ok(()),
            Some(live) => {
                if run::teardown(&live.run_dir, Outcome::Ok, &self.cfg).is_ok() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "run {} still mounted at {}; finish failed — recover with lunchbox finish {}",
                        live.run_id,
                        live.run_dir.display(),
                        live.run_id
                    ))
                }
            }
        }
    }

    fn open(&self, route: home::Route) -> Result<Layer> {
        Ok(match route {
            home::Route::Pantry => {
                Layer::Pantry(PantryState::build(&self.cfg, &self.libraries)?)
            }
            home::Route::Manifests => {
                Layer::Manifests(ManifestsState::build(&self.cfg, &self.libraries)?)
            }
            home::Route::Doctor => Layer::Doctor(crate::tui::doctor::DoctorState::new(
                crate::doctor_data(None)?,
            )),
            home::Route::Policy => Layer::Policy(crate::tui::policy::PolicyState::load()?),
        })
    }

    fn wants_finish(&self, event: &Event) -> bool {
        let Event::Key(key) = event else {
            return false;
        };
        if key.kind != KeyEventKind::Press {
            return false;
        }
        if !matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('f'), KeyModifiers::NONE)
        ) {
            return false;
        }
        !matches!(
            self.stack.last(),
            Some(Layer::Doctor(_)) | Some(Layer::Policy(_)) | Some(Layer::Editor(_))
        )
    }

    fn dispatch(&mut self, event: &Event) -> Result<bool> {
        if self.wants_finish(event) {
            self.finish_live();
            return Ok(false);
        }
        let action = match self.stack.last_mut() {
            None => return Ok(true),
            Some(Layer::Home(state)) => home::handle_event(state, event),
            Some(Layer::Pantry(state)) => pantry::handle_event(state, event),
            Some(Layer::Manifests(state)) => manifests::handle_event(state, event),
            Some(Layer::ManifestDetail(state)) => manifests::handle_detail_event(state, event),
            Some(Layer::Doctor(state)) => match crate::tui::doctor::handle_event(state, event) {
                crate::tui::doctor::Action::Continue => Action::Continue,
                crate::tui::doctor::Action::Quit => Action::Pop,
            },
            Some(Layer::Policy(state)) => match crate::tui::policy::handle_event(state, event) {
                crate::tui::policy::Action::Continue => Action::Continue,
                crate::tui::policy::Action::Quit => Action::Pop,
            },
            Some(Layer::Confirm(state)) => confirm::handle_event(state, event),
            Some(Layer::Editor(state)) => editor::handle_event(state, event),
        };
        match action {
            Action::Continue => Ok(false),
            Action::Pop => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                    if let Some(Layer::Manifests(state)) = self.stack.last_mut() {
                        *state = ManifestsState::build(&self.cfg, &self.libraries)?;
                    }
                }
                Ok(false)
            }
            Action::Quit => Ok(true),
            Action::Open(route) => {
                let layer = self.open(route)?;
                self.stack.push(layer);
                Ok(false)
            }
            Action::ConfirmFlat(names) => {
                let confirm =
                    ConfirmState::new(confirm::Source::Flat(names), &self.cfg, &self.libraries);
                self.stack.push(Layer::Confirm(confirm));
                Ok(false)
            }
            Action::Detail(manifest) => {
                let detail = ManifestDetailState::new(manifest, &self.cfg, &self.libraries)?;
                self.stack.push(Layer::ManifestDetail(detail));
                Ok(false)
            }
            Action::Mount { source, task } => {
                if self.mount(source, &task) {
                    self.stack.pop();
                }
                Ok(false)
            }
            Action::EditManifest(manifest) => {
                match editor::EditorState::load(manifest.path, &self.cfg, &self.libraries) {
                    Ok(editor) => self.stack.push(Layer::Editor(editor)),
                    Err(error) => self.status = format!("cannot edit: {error:#}"),
                }
                Ok(false)
            }
            Action::NewEditor => {
                match editor::EditorState::new_draft(&self.cfg, &self.libraries) {
                    Ok(editor) => self.stack.push(Layer::Editor(editor)),
                    Err(error) => self.status = format!("cannot start editor: {error:#}"),
                }
                Ok(false)
            }
            Action::Finish => {
                self.finish_live();
                Ok(false)
            }
            Action::EditorSave => {
                if let Some(Layer::Editor(state)) = self.stack.last_mut() {
                    if let Err(error) = editor::save(state, &self.cfg, &self.libraries) {
                        state.status = format!("save failed: {error:#}");
                    }
                }
                Ok(false)
            }
            Action::EditorSaveAndStart => {
                let input = match self.stack.last_mut() {
                    Some(Layer::Editor(state)) => {
                        match editor::save(state, &self.cfg, &self.libraries) {
                            Ok(_) => state.build_input(),
                            Err(error) => {
                                state.status = format!("save failed: {error:#}");
                                return Ok(false);
                            }
                        }
                    }
                    _ => return Ok(false),
                };
                let confirm =
                    ConfirmState::new(confirm::Source::Manifest(input), &self.cfg, &self.libraries);
                self.stack.push(Layer::Confirm(confirm));
                Ok(false)
            }
        }
    }
}

fn draw_layer(layer: Option<&mut Layer>, frame: &mut Frame, area: Rect) {
    let title = match layer {
        Some(Layer::Home(_)) => "lunchbox menu".to_string(),
        Some(Layer::Pantry(_)) => "Pantry".to_string(),
        Some(Layer::Manifests(_)) => "Manifests".to_string(),
        Some(Layer::ManifestDetail(state)) => format!("manifest {}", state.manifest.name),
        Some(Layer::Doctor(_)) => "Doctor".to_string(),
        Some(Layer::Policy(_)) => "Policy".to_string(),
        Some(Layer::Confirm(_)) => "Start run?".to_string(),
        Some(Layer::Editor(editor)) => match editor.input_mode {
            Some(field) => format!("Editor — {}", field.label()),
            None => "Editor".to_string(),
        },
        None => return,
    };
    let block = chrome::screen(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match layer {
        Some(Layer::Home(state)) => home::render(state, frame, inner),
        Some(Layer::Pantry(state)) => pantry::render(state, frame, inner),
        Some(Layer::Manifests(state)) => manifests::render(state, frame, inner),
        Some(Layer::ManifestDetail(state)) => manifests::render_detail(state, frame, inner),
        Some(Layer::Doctor(state)) => crate::tui::doctor::render(state, frame, inner),
        Some(Layer::Policy(state)) => crate::tui::policy::render(state, frame, inner),
        Some(Layer::Confirm(state)) => confirm::render(state, frame, inner),
        Some(Layer::Editor(state)) => editor::render(state, frame, inner),
        None => {}
    }
}

fn status_line(state: &MenuState) -> Line<'static> {
    let live = match &state.live {
        Some(run) => chrome::good(format!("▶ {} mounted", run.run_id)),
        None => chrome::dim("no live run".to_string()),
    };
    let text = match state.stack.last() {
        Some(Layer::Policy(policy)) => {
            if policy.input_mode {
                format!("add entry: {}", policy.input)
            } else {
                policy.status.clone()
            }
        }
        Some(Layer::Editor(editor)) => editor.prompt(),
        _ => state.status.clone(),
    };
    Line::from(vec![live, Span::from("  ·  "), Span::from(text)])
}

fn footer_text(state: &MenuState) -> &'static str {
    match state.stack.last() {
        Some(Layer::Home(_)) => "↑/↓ move · Enter open · Esc back · q quit",
        Some(Layer::Pantry(_)) => {
            "←/→ section · ↑/↓ move · Space toggle · s start · f finish · Esc back · q quit"
        }
        Some(Layer::Manifests(_)) => {
            "↑/↓ move · Enter details · n new · e edit · Esc back · f finish · q quit"
        }
        Some(Layer::ManifestDetail(_)) => "Esc back · f finish · q quit",
        Some(Layer::Doctor(_)) => "↑/↓ scroll · Esc back",
        Some(Layer::Policy(_)) => "Tab layer · ←/→ list · a add · d delete · Esc back",
        Some(Layer::Confirm(_)) => "Enter mount · Esc cancel · f finish",
        Some(Layer::Editor(_)) => {
            "Tab pane · Space select · a add tool · t task · b budget · s save · Esc back · q quit"
        }
        None => "",
    }
}

pub fn run(mut state: MenuState) -> Result<()> {
    let mut terminal = terminal::install()?;
    let _restore = Restore;
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let body = Rect {
                height: area.height.saturating_sub(2),
                ..area
            };
            draw_layer(state.stack.last_mut(), frame, body);
            let status = Rect {
                y: body.bottom(),
                height: 1,
                ..area
            };
            frame.render_widget(Paragraph::new(status_line(&state)), status);
            let footer = Rect {
                y: status.bottom(),
                height: 1,
                ..area
            };
            frame.render_widget(Paragraph::new(chrome::keybar(footer_text(&state))), footer);
        })?;
        let event = terminal::next_event()?;
        if state.dispatch(&event)? {
            return state.quit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testkit::demo_tree_home;
    use std::os::unix::fs::PermissionsExt;
    use crossterm::event::{KeyCode, KeyEvent};

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn menu_for(home: &std::path::Path, runs: &std::path::Path) -> MenuState {
        std::fs::create_dir_all(home.join(".lunchbox")).unwrap();
        std::fs::write(
            home.join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.display()),
        )
        .unwrap();
        let cfg = crate::config::Config::load().unwrap();
        let libraries = vec![home.join(".agents").join("skills")];
        MenuState::new(cfg, libraries)
    }

    fn select_two_skills_and_start(menu: &mut MenuState) {
        menu.dispatch(&press(KeyCode::Enter)).unwrap();
        menu.dispatch(&press(KeyCode::Char(' '))).unwrap();
        menu.dispatch(&press(KeyCode::Down)).unwrap();
        menu.dispatch(&press(KeyCode::Char(' '))).unwrap();
        menu.dispatch(&press(KeyCode::Char('s'))).unwrap();
        assert!(matches!(menu.stack.last(), Some(Layer::Confirm(_))));
        menu.dispatch(&press(KeyCode::Enter)).unwrap();
    }

    fn run_dirs(runs: &std::path::Path) -> Vec<PathBuf> {
        std::fs::read_dir(runs)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect()
    }

    #[test]
    fn menu_mount_and_finish_roundtrip() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        crate::config::with_home(home.path(), || {
            let mut menu = menu_for(home.path(), runs.path());
            select_two_skills_and_start(&mut menu);

            let run_dir = menu
                .live
                .as_ref()
                .expect("run mounted")
                .run_dir
                .clone();
            assert!(menu.status.contains("mounted"), "{}", menu.status);
            assert!(run_dir.join("workdir").exists());
            let dirs = run_dirs(runs.path());
            assert_eq!(dirs.len(), 1, "exactly one run dir");
            assert!(matches!(menu.stack.last(), Some(Layer::Pantry(_))));

            menu.dispatch(&press(KeyCode::Char('f'))).unwrap();
            assert!(menu.live.is_none(), "f must finish the live run");
            assert_eq!(menu.status, "unmounted — result.json written");
            assert!(!run_dir.join("workdir").exists());
            let result: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(run_dir.join("result.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(result["unmounted"], serde_json::json!(true));

            assert!(menu.dispatch(&press(KeyCode::Char('q'))).unwrap());
            menu.quit().unwrap();
        });
    }

    #[test]
    fn quit_with_mounted_run_implies_finish() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        crate::config::with_home(home.path(), || {
            let mut menu = menu_for(home.path(), runs.path());
            select_two_skills_and_start(&mut menu);
            let run_dir = menu.live.as_ref().unwrap().run_dir.clone();
            assert!(run_dir.join("workdir").exists());

            assert!(menu.dispatch(&press(KeyCode::Char('q'))).unwrap());
            menu.quit().unwrap();
            assert!(!run_dir.join("workdir").exists(), "quit must not leak a mount");
            assert!(run_dir.join("result.json").exists());
        });
    }

    #[test]
    fn failed_finish_keeps_run_recoverable_and_quit_fails_closed() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        crate::config::with_home(home.path(), || {
            let mut menu = menu_for(home.path(), runs.path());
            select_two_skills_and_start(&mut menu);
            let run_dir = menu.live.as_ref().unwrap().run_dir.clone();
            std::fs::set_permissions(&run_dir, PermissionsExt::from_mode(0o500)).unwrap();

            menu.dispatch(&press(KeyCode::Char('f'))).unwrap();
            assert!(menu.live.is_some(), "failed finish must keep the run handle");
            assert!(menu.status.contains("finish failed"), "{}", menu.status);

            menu.dispatch(&press(KeyCode::Char('s'))).unwrap();
            menu.dispatch(&press(KeyCode::Enter)).unwrap();
            assert!(
                menu.status.contains("already mounted"),
                "a stuck run must block starting another: {}",
                menu.status
            );
            assert!(matches!(menu.stack.last(), Some(Layer::Confirm(_))));
            menu.dispatch(&press(KeyCode::Esc)).unwrap();
            assert!(menu.dispatch(&press(KeyCode::Char('q'))).unwrap());
            let error = menu.quit().unwrap_err().to_string();
            assert!(error.contains("still mounted"), "{error}");
            assert!(menu.live.is_none(), "quit consumed the handle");

            std::fs::set_permissions(&run_dir, PermissionsExt::from_mode(0o700)).unwrap();
            run::teardown(&run_dir, Outcome::Ok, &menu.cfg).unwrap();
            assert!(!run_dir.join("workdir").exists());
            assert!(run_dir.join("result.json").exists());
        });
    }

    #[test]
    fn f_reaches_editor_input_and_finishes_only_outside_it() {
        let home = demo_tree_home();
        let runs = tempfile::TempDir::new().unwrap();
        crate::config::with_home(home.path(), || {
            let mut menu = menu_for(home.path(), runs.path());
            menu.dispatch(&press(KeyCode::Down)).unwrap();
            menu.dispatch(&press(KeyCode::Enter)).unwrap();
            menu.dispatch(&press(KeyCode::Char('n'))).unwrap();
            assert!(matches!(menu.stack.last(), Some(Layer::Editor(_))));
            menu.dispatch(&press(KeyCode::Char('f'))).unwrap();
            match menu.stack.last() {
                Some(Layer::Editor(editor)) => assert_eq!(
                    editor.input, "f",
                    "f must reach the editor input, not trigger finish"
                ),
                _ => panic!("editor must still be on top"),
            }
            assert_eq!(menu.status, "no live run", "finish must not have fired");
            menu.dispatch(&press(KeyCode::Backspace)).unwrap();
            menu.dispatch(&press(KeyCode::Char('e'))).unwrap();
            menu.dispatch(&press(KeyCode::Enter)).unwrap();
            menu.dispatch(&press(KeyCode::Char('f'))).unwrap();
            assert_eq!(menu.status, "no live run");
            assert!(matches!(menu.stack.last(), Some(Layer::Editor(_))), "f outside input must not pop the editor");
        });
    }
}
