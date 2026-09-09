use crate::config::Config;
use crate::library::ListedSkill;
use crate::tui::menu::{chrome, Action};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct WorkerRow {
    pub name: String,
    pub pack: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Workers,
    Skills,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    NewName,
    Rename,
    Task,
    Budget,
    ManifestName,
}

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::NewName => "new worker",
            Field::Rename => "rename worker",
            Field::Task => "task",
            Field::Budget => "budget",
            Field::ManifestName => "manifest name",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SaveTarget {
    Project,
    Home,
}

impl SaveTarget {
    fn toggle(self) -> Self {
        match self {
            SaveTarget::Project => SaveTarget::Home,
            SaveTarget::Home => SaveTarget::Project,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SaveTarget::Project => "./lunchbox/manifests",
            SaveTarget::Home => "~/.lunchbox/manifests",
        }
    }

    fn dir(self) -> Result<PathBuf> {
        Ok(match self {
            SaveTarget::Project => PathBuf::from("lunchbox").join("manifests"),
            SaveTarget::Home => crate::config::lunchbox_home()?.join("manifests"),
        })
    }
}

pub struct EditorState {
    pub path: Option<PathBuf>,
    pub doc: toml_edit::DocumentMut,
    pub name: String,
    pub workers: Vec<WorkerRow>,
    pub worker_cursor: usize,
    pub focus: Focus,
    pub task: String,
    pub adapter: String,
    pub budget: Option<u64>,
    pub pin_hashes: bool,
    pub skills: Vec<ListedSkill>,
    pub skill_cursor: usize,
    pub input: String,
    pub input_mode: Option<Field>,
    pub save_target: SaveTarget,
    pub status: String,
}

impl EditorState {
    pub fn load(path: PathBuf, cfg: &Config, libraries: &[PathBuf]) -> Result<Self> {
        let input = crate::run::read_manifest_input(&path)?;
        let text = std::fs::read_to_string(&path)?;
        let doc: toml_edit::DocumentMut = text.parse()?;
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self::from_parts(
            Some(path),
            doc,
            name,
            input
                .workers
                .iter()
                .map(|worker| WorkerRow {
                    name: worker.name.clone(),
                    pack: worker.pack.clone(),
                })
                .collect(),
            input.task.clone(),
            input.adapter.clone(),
            input.budget.as_ref().map(|budget| budget.max_menu_tokens),
            cfg,
            libraries,
        )
    }

    pub fn new_draft(cfg: &Config, libraries: &[PathBuf]) -> Result<Self> {
        let mut state = Self::from_parts(
            None,
            toml_edit::DocumentMut::new(),
            String::new(),
            vec![WorkerRow {
                name: "main".to_string(),
                pack: Vec::new(),
            }],
            String::new(),
            "none".to_string(),
            None,
            cfg,
            libraries,
        )?;
        state.input_mode = Some(Field::ManifestName);
        state.status = "name the manifest first".to_string();
        Ok(state)
    }

    fn from_parts(
        path: Option<PathBuf>,
        doc: toml_edit::DocumentMut,
        name: String,
        workers: Vec<WorkerRow>,
        task: String,
        adapter: String,
        budget: Option<u64>,
        cfg: &Config,
        libraries: &[PathBuf],
    ) -> Result<Self> {
        let sections = super::pantry::build_sections(cfg, libraries)?;
        let skills: Vec<ListedSkill> = super::pantry::skill_union(&sections)
            .into_values()
            .collect();
        let mut state = EditorState {
            path,
            doc,
            name,
            workers,
            worker_cursor: 0,
            focus: Focus::Skills,
            task,
            adapter,
            budget,
            pin_hashes: false,
            skills,
            skill_cursor: 0,
            input: String::new(),
            input_mode: None,
            save_target: SaveTarget::Project,
            status: String::new(),
        };
        state.refresh_status();
        Ok(state)
    }

    pub fn skill_named(&self, name: &str) -> Option<&ListedSkill> {
        self.skills.iter().find(|skill| skill.name == name)
    }

    pub fn live_tokens(&self) -> u64 {
        let mut names: Vec<&str> = Vec::new();
        for worker in &self.workers {
            for pin in &worker.pack {
                let base = pin_base(pin);
                if !names.contains(&base) {
                    names.push(base);
                }
            }
        }
        crate::tokens::with_preamble(
            names
                .iter()
                .filter_map(|name| {
                    self.skills
                        .iter()
                        .find(|skill| skill.name == *name)
                        .map(|skill| skill.tokens)
                })
                .sum(),
        )
    }

    pub fn build_input(&self) -> crate::run::ManifestInput {
        crate::run::ManifestInput {
            schema: 1,
            task: self.task.clone(),
            adapter: self.adapter.clone(),
            run_id: None,
            created_at: None,
            harness_argv: None,
            budget: self
                .budget
                .map(|max_menu_tokens| crate::run::Budget { max_menu_tokens }),
            workers: self
                .workers
                .iter()
                .map(|worker| crate::run::Worker {
                    name: worker.name.clone(),
                    pack: worker.pack.clone(),
                    description: None,
                    tools: None,
                })
                .collect(),
        }
    }

    fn refresh_status(&mut self) {
        self.status = match self.build_input().validate() {
            Ok(_) => "ok — s save · S save+start".to_string(),
            Err(error) => format!("{error:#}"),
        };
    }
    pub fn prompt(&self) -> String {
        match self.input_mode {
            Some(Field::ManifestName) => format!(
                "manifest name: {} (Tab target: {})",
                self.input,
                self.save_target.label()
            ),
            Some(Field::NewName) => format!("new worker name: {}", self.input),
            Some(Field::Rename) => format!("rename worker: {}", self.input),
            Some(Field::Task) => format!("task: {}", self.input),
            Some(Field::Budget) => format!("budget max_menu_tokens: {}", self.input),
            None => self.status.clone(),
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Workers => Focus::Skills,
            Focus::Skills => Focus::Workers,
        };
    }

    fn move_cursor(&mut self, delta: i64) {
        match self.focus {
            Focus::Workers => {
                if self.workers.is_empty() {
                    self.worker_cursor = 0;
                    return;
                }
                let max = self.workers.len() - 1;
                let next = self.worker_cursor as i64 + delta;
                self.worker_cursor = next.clamp(0, max as i64) as usize;
            }
            Focus::Skills => {
                if self.skills.is_empty() {
                    self.skill_cursor = 0;
                    return;
                }
                let max = self.skills.len() - 1;
                let next = self.skill_cursor as i64 + delta;
                self.skill_cursor = next.clamp(0, max as i64) as usize;
            }
        }
    }

    fn toggle_skill(&mut self) {
        let Some(skill) = self.skills.get(self.skill_cursor) else {
            return;
        };
        let Some(worker) = self.workers.get_mut(self.worker_cursor) else {
            return;
        };
        if let Some(at) = worker.pack.iter().position(|pin| pin == &skill.name) {
            worker.pack.remove(at);
        } else {
            worker.pack.push(skill.name.clone());
        }
        self.refresh_status();
    }

    fn delete_worker(&mut self) {
        if self.workers.is_empty() {
            return;
        }
        self.workers.remove(self.worker_cursor);
        self.worker_cursor = self
            .worker_cursor
            .saturating_sub(1)
            .min(self.workers.len().saturating_sub(1));
        self.refresh_status();
    }

    fn cycle_adapter(&mut self) {
        self.adapter = match self.adapter.as_str() {
            "none" => "pi".to_string(),
            "pi" => "omp".to_string(),
            _ => "none".to_string(),
        };
        self.refresh_status();
    }

    fn begin_input(&mut self, field: Field) {
        self.input = match field {
            Field::Rename => self
                .workers
                .get(self.worker_cursor)
                .map(|worker| worker.name.clone())
                .unwrap_or_default(),
            Field::Task => self.task.clone(),
            Field::Budget => self
                .budget
                .map(|budget| budget.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.input_mode = Some(field);
    }

    fn save_requested(&mut self, and_start: bool) -> Action {
        if self.path.is_none() && self.name.is_empty() {
            self.input.clear();
            self.input_mode = Some(Field::ManifestName);
            self.status = "name the manifest first".to_string();
            return Action::Continue;
        }
        if self.build_input().validate().is_err() {
            self.status = "fix validation errors first".to_string();
            return Action::Continue;
        }
        if and_start {
            Action::EditorSaveAndStart
        } else {
            Action::EditorSave
        }
    }

    fn commit(&mut self, field: Field) {
        let value = self.input.trim().to_string();
        match field {
            Field::ManifestName => {
                if value.is_empty() || value.contains('/') {
                    self.status = "manifest name must be one path component".to_string();
                    return;
                }
                self.name = value;
            }
            Field::NewName => {
                if value.is_empty() {
                    return;
                }
                self.workers.push(WorkerRow {
                    name: value,
                    pack: Vec::new(),
                });
                self.worker_cursor = self.workers.len() - 1;
                self.focus = Focus::Workers;
            }
            Field::Rename => {
                if let Some(worker) = self.workers.get_mut(self.worker_cursor) {
                    worker.name = value;
                }
            }
            Field::Task => self.task = value,
            Field::Budget => {
                let parsed = if value.is_empty() {
                    Ok(None)
                } else {
                    value
                        .parse::<u64>()
                        .map(Some)
                        .map_err(|_| "budget must be a whole number".to_string())
                };
                match parsed {
                    Ok(budget) => self.budget = budget,
                    Err(message) => {
                        self.status = message;
                        return;
                    }
                }
            }
        }
        self.input.clear();
        self.input_mode = None;
        self.refresh_status();
    }
}

fn pin_base(pin: &str) -> &str {
    pin.split_once('@').map_or(pin, |(base, _)| base)
}

pub fn save(state: &mut EditorState, cfg: &Config, libraries: &[PathBuf]) -> Result<PathBuf> {
    let path = match &state.path {
        Some(path) => path.clone(),
        None => state.save_target.dir()?.join(format!("{}.toml", state.name)),
    };
    let hashes = if state.pin_hashes {
        Some(resolve_hashes(state, cfg, libraries)?)
    } else {
        None
    };
    let doc = &mut state.doc;
    doc["schema"] = toml_edit::value(1);
    doc["task"] = toml_edit::value(state.task.clone());
    doc["adapter"] = toml_edit::value(state.adapter.clone());
    match state.budget {
        Some(budget) => doc["budget"]["max_menu_tokens"] = toml_edit::value(budget as i64),
        None => {
            doc.as_table_mut().remove("budget");
        }
    }
    let mut workers = toml_edit::ArrayOfTables::new();
    for worker in &state.workers {
        let mut table = toml_edit::Table::new();
        table["name"] = toml_edit::value(worker.name.clone());
        let mut pack = toml_edit::Array::new();
        for pin in &worker.pack {
            match &hashes {
                Some(map) => {
                    let name = pin
                        .split_once('@')
                        .map_or(pin.as_str(), |(base, _)| base);
                    pack.push(format!("{name}@{}", map.get(name).cloned().unwrap_or_default()));
                }
                None => pack.push(pin.clone()),
            }
        }
        table["pack"] = toml_edit::value(pack);
        workers.push(table);
    }
    doc.as_table_mut().remove("workers");
    doc.as_table_mut()
        .insert("workers", toml_edit::Item::ArrayOfTables(workers));

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let staged = path.with_extension("toml.new");
    std::fs::write(&staged, doc.to_string())?;
    std::fs::rename(&staged, &path)?;
    state.path = Some(path.clone());
    state.status = format!("wrote {}", path.display());
    Ok(path)
}

fn resolve_hashes(
    state: &EditorState,
    cfg: &Config,
    libraries: &[PathBuf],
) -> Result<BTreeMap<String, String>> {
    let mut names: Vec<String> = Vec::new();
    for worker in &state.workers {
        for pin in &worker.pack {
            let name = pin
                .split_once('@')
                .map_or(pin.clone(), |(base, _)| base.to_string());
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let locked = crate::resolve::resolve(&names, cfg, libraries, false)?;
    Ok(locked
        .into_iter()
        .map(|skill| (skill.name, skill.hash))
        .collect())
}

pub fn render(state: &mut EditorState, frame: &mut Frame, area: Rect) {
    let label = |text: &str| chrome::dim(format!("{text:<15} "));
    let editing = |field: Field| state.input_mode == Some(field);
    let mut lines = Vec::new();

    if editing(Field::ManifestName) {
        lines.push(Line::from(vec![
            label("manifest"),
            chrome::accent(format!("{}▌", state.input)),
        ]));
    } else {
        match &state.path {
            Some(path) => lines.push(Line::from(vec![
                label("manifest"),
                chrome::dim(path.display().to_string()),
            ])),
            None => lines.push(Line::from(vec![
                label("manifest"),
                chrome::dim(format!(
                    "{} → {}/{}.toml",
                    state.save_target.label(),
                    state.name,
                    state.name
                )),
            ])),
        };
    }

    if editing(Field::Task) {
        lines.push(Line::from(vec![
            label("task"),
            chrome::accent(format!("{}▌", state.input)),
        ]));
    } else {
        lines.push(Line::from(vec![label("task"), Span::raw(state.task.clone())]));
    }

    lines.push(Line::from(vec![
        label("adapter"),
        Span::raw(state.adapter.clone()),
    ]));

    if editing(Field::Budget) {
        lines.push(Line::from(vec![
            label("budget"),
            chrome::accent(format!("{}▌", state.input)),
        ]));
    } else {
        lines.push(Line::from(vec![
            label("budget"),
            Span::raw(match state.budget {
                Some(budget) => budget.to_string(),
                None => "(none)".to_string(),
            }),
        ]));
    }

    lines.push(Line::from(vec![
        label("pin hashes"),
        Span::raw(if state.pin_hashes { "on" } else { "off" }.to_string()),
    ]));
    lines.push(Line::from(vec![
        chrome::dim("menu_tokens      ".to_string()),
        chrome::bold(format!("this run: {}", state.live_tokens())),
    ]));

    lines.push(Line::from(""));
    let workers_focused = state.focus == Focus::Workers;
    lines.push(Line::from(chrome::bold(format!(
        "workers{}",
        if workers_focused { " ▸" } else { "" }
    ))));
    for (index, worker) in state.workers.iter().enumerate() {
        let cursor_here = index == state.worker_cursor && workers_focused;
        let marker = if cursor_here { "▸ " } else { "  " };
        let body = format!("{:<16}", worker.name);
        let pack_text = if worker.pack.is_empty() {
            "(empty)".to_string()
        } else {
            worker
                .pack
                .iter()
                .map(|pin| match state.skill_named(pin_base(pin)) {
                    Some(skill) => format!("{pin} ({})", skill.tokens),
                    None => format!("{pin} (?)"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let pack = if worker.pack.is_empty() {
            chrome::dim(format!("pack: {pack_text}"))
        } else {
            Span::raw(format!("pack: {pack_text}"))
        };
        if cursor_here {
            lines.push(Line::from(vec![
                Span::raw(marker),
                chrome::bold(body),
                Span::raw(" "),
                pack,
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw(marker),
                Span::raw(body),
                Span::raw(" "),
                pack,
            ]));
        }
    }
    if state.workers.is_empty() {
        lines.push(Line::from(chrome::dim("  (no workers)".to_string())));
    }

    lines.push(Line::from(""));
    let skills_focused = state.focus == Focus::Skills;
    lines.push(Line::from(chrome::bold(format!(
        "skills{}",
        if skills_focused { " ▸" } else { "" }
    ))));
    for (index, skill) in state.skills.iter().enumerate() {
        let cursor_here = index == state.skill_cursor && skills_focused;
        let marker = if cursor_here { "▸ " } else { "  " };
        let in_pack = state
            .workers
            .get(state.worker_cursor)
            .is_some_and(|worker| worker.pack.contains(&skill.name));
        let check = if in_pack {
            chrome::good("[x]")
        } else {
            chrome::dim("[ ]")
        };
        let body = format!(" {:<16} {}", skill.name, skill.tokens);
        if cursor_here {
            lines.push(Line::from(vec![Span::raw(marker), check, chrome::bold(body)]));
        } else {
            lines.push(Line::from(vec![Span::raw(marker), check, Span::raw(body)]));
        }
    }
    if state.skills.is_empty() {
        lines.push(Line::from(chrome::dim("  (no skills discovered)".to_string())));
    } else if let Some(skill) = state.skills.get(state.skill_cursor) {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            chrome::bold(skill.name.clone()),
            chrome::dim(format!("  {}", skill.description)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(chrome::dim(
        "w/r/d workers · c adapter · p pin hashes · f finish · S save+start".to_string(),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut EditorState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    if let Some(field) = state.input_mode {
        return input_event(state, field, key.code, key.modifiers);
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Action::Quit,
        (KeyCode::Esc, _) => Action::Pop,
        (KeyCode::Tab, _) => {
            state.toggle_focus();
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
            state.toggle_skill();
            Action::Continue
        }
        (KeyCode::Char('r'), KeyModifiers::NONE) => {
            state.begin_input(Field::Rename);
            Action::Continue
        }
        (KeyCode::Char('d'), KeyModifiers::NONE) => {
            state.delete_worker();
            Action::Continue
        }
        (KeyCode::Char('w'), KeyModifiers::NONE) => {
            state.begin_input(Field::NewName);
            Action::Continue
        }
        (KeyCode::Char('t'), KeyModifiers::NONE) => {
            state.begin_input(Field::Task);
            Action::Continue
        }
        (KeyCode::Char('b'), KeyModifiers::NONE) => {
            state.begin_input(Field::Budget);
            Action::Continue
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            state.cycle_adapter();
            Action::Continue
        }
        (KeyCode::Char('p'), KeyModifiers::NONE) => {
            state.pin_hashes = !state.pin_hashes;
            Action::Continue
        }
        (KeyCode::Char('f'), KeyModifiers::NONE) => Action::Finish,
        (KeyCode::Char('S'), _) | (KeyCode::Char('s'), KeyModifiers::SHIFT) => {
            state.save_requested(true)
        }
        (KeyCode::Char('s'), KeyModifiers::NONE) => state.save_requested(false),
        _ => Action::Continue,
    }
}

fn input_event(
    state: &mut EditorState,
    field: Field,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Action {
    match (code, modifiers) {
        (KeyCode::Esc, _) => {
            state.input.clear();
            state.input_mode = None;
        }
        (KeyCode::Enter, _) => state.commit(field),
        (KeyCode::Backspace, _) => {
            state.input.pop();
        }
        (KeyCode::Tab, _) if field == Field::ManifestName => {
            state.save_target = state.save_target.toggle();
        }
        (KeyCode::Char(ch), KeyModifiers::NONE) | (KeyCode::Char(ch), KeyModifiers::SHIFT) => {
            state.input.push(ch);
        }
        _ => {}
    }
    Action::Continue
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::tui::testkit::demo_tree_home;
    use crossterm::event::KeyEvent;

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    fn config_for(home: &std::path::Path) -> Config {
        let runs = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(home.join(".lunchbox")).unwrap();
        std::fs::write(
            home.join(".lunchbox").join("config.toml"),
            format!("runs_dir = \"{}\"\n", runs.path().display()),
        )
        .unwrap();
        crate::config::with_home(home, || Config::load().unwrap())
    }

    fn libraries_for(home: &std::path::Path) -> Vec<PathBuf> {
        vec![home.join(".agents").join("skills")]
    }

    fn type_input(state: &mut EditorState, text: &str) {
        for ch in text.chars() {
            handle_event(
                state,
                &Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
            );
        }
        handle_event(state, &press(KeyCode::Enter));
    }

    fn named_draft(cfg: &Config, libraries: &[PathBuf], name: &str) -> EditorState {
        let mut state = EditorState::new_draft(cfg, libraries).unwrap();
        assert!(matches!(state.input_mode, Some(Field::ManifestName)));
        type_input(&mut state, name);
        assert_eq!(state.name, name);
        state
    }

    #[test]
    fn empty_pack_and_missing_name_block_save() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let mut state = EditorState::new_draft(&cfg, &libraries).unwrap();
            handle_event(&mut state, &press(KeyCode::Char('s')));
            assert!(
                matches!(state.input_mode, Some(Field::ManifestName)),
                "save without a name must ask for one"
            );
            type_input(&mut state, "e2e");
            assert!(matches!(
                handle_event(&mut state, &press(KeyCode::Char('s'))),
                Action::Continue
            ));
            assert_eq!(state.status, "fix validation errors first");

            handle_event(&mut state, &press(KeyCode::Char(' ')));
            handle_event(&mut state, &press(KeyCode::Down));
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            assert_eq!(state.status, "ok — s save · S save+start");
            assert!(matches!(
                handle_event(&mut state, &press(KeyCode::Char('s'))),
                Action::EditorSave
            ));
        });
    }

    #[test]
    fn live_tokens_and_pack_costs_track_selections() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let mut state = named_draft(&cfg, &libraries, "e2e");
            assert_eq!(state.live_tokens(), 0);
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            assert_eq!(state.live_tokens(), 139, "union updates on first toggle");
            handle_event(&mut state, &press(KeyCode::Down));
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            assert_eq!(state.live_tokens(), 203);
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            assert_eq!(
                state.live_tokens(), 139,
                "toggling off removes the cost again"
            );
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 20)).unwrap();
            let frame = terminal
                .draw(|frame| render(&mut state, frame, frame.area()))
                .unwrap();
            let rendered = crate::tui::snap::frame_to_string(frame.buffer, frame.area);
            assert!(rendered.contains("this run: 203"), "{rendered}");
            assert!(rendered.contains("demo-review (65)"), "{rendered}");
            assert!(rendered.contains("demo-scan (64)"), "{rendered}");
            assert!(
                rendered.contains("Scan for leaked secrets in the worktree."),
                "focused skill description must be visible: {rendered}"
            );
        });
    }

    #[test]
    fn duplicate_worker_names_are_flagged_live() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let mut state = named_draft(&cfg, &libraries, "e2e");
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            handle_event(&mut state, &press(KeyCode::Char('w')));
            type_input(&mut state, "main");
            assert_eq!(state.workers.len(), 2);
            assert!(
                state.status.contains("duplicate worker name 'main'"),
                "{}",
                state.status
            );
        });
    }

    #[test]
    fn rename_add_delete_workers() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let mut state = named_draft(&cfg, &libraries, "e2e");
            handle_event(&mut state, &press(KeyCode::Char('w')));
            type_input(&mut state, "second");
            assert_eq!(
                state
                    .workers
                    .iter()
                    .map(|worker| worker.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["main", "second"]
            );
            assert_eq!(state.worker_cursor, 1, "new worker gains the cursor");

            handle_event(&mut state, &press(KeyCode::Char('r')));
            for _ in 0.."second".len() {
                handle_event(&mut state, &press(KeyCode::Backspace));
            }
            type_input(&mut state, "renamed");
            assert_eq!(state.workers[1].name, "renamed");

            handle_event(&mut state, &press(KeyCode::Char('d')));
            assert_eq!(state.workers.len(), 1);
            assert_eq!(state.workers[0].name, "main");
        });
    }

    #[test]
    fn budget_and_adapter_edits() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let mut state = named_draft(&cfg, &libraries, "e2e");
            handle_event(&mut state, &press(KeyCode::Char('b')));
            type_input(&mut state, "not-a-number");
            assert_eq!(state.status, "budget must be a whole number");
            handle_event(&mut state, &press(KeyCode::Esc));
            handle_event(&mut state, &press(KeyCode::Char('b')));
            type_input(&mut state, "1500");
            assert_eq!(state.budget, Some(1500));
            handle_event(&mut state, &press(KeyCode::Char('b')));
            for _ in 0.."1500".len() {
                handle_event(&mut state, &press(KeyCode::Backspace));
            }
            handle_event(&mut state, &press(KeyCode::Enter));
            assert_eq!(state.budget, None, "empty budget commit clears it");

            handle_event(&mut state, &press(KeyCode::Char('c')));
            assert_eq!(state.adapter, "pi");
            handle_event(&mut state, &press(KeyCode::Char('c')));
            assert_eq!(state.adapter, "omp");
            handle_event(&mut state, &press(KeyCode::Char('c')));
            assert_eq!(state.adapter, "none");
        });
    }

    #[test]
    fn save_round_trips_comments_and_unknown_keys() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join("rt.toml");
            std::fs::write(
                &path,
                "# my note\nschema = 1\ntask = \"old\"\nadapter = \"none\"\nrun_id = \"x\"\n\
                 [[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n",
            )
            .unwrap();
            let mut state = EditorState::load(path.clone(), &cfg, &libraries).unwrap();
            assert_eq!(state.name, "rt");
            handle_event(&mut state, &press(KeyCode::Char('t')));
            for _ in 0.."old".len() {
                handle_event(&mut state, &press(KeyCode::Backspace));
            }
            type_input(&mut state, "new task");
            let saved = save(&mut state, &cfg, &libraries).unwrap();
            assert_eq!(saved, path);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains("# my note"), "{text}");
            assert!(text.contains("run_id = \"x\""), "{text}");
            assert!(text.contains("task = \"new task\""), "{text}");
            assert!(text.contains("schema = 1"), "{text}");
            assert!(text.contains("pack = ["));
            assert!(text.contains("\"demo-review\""), "{text}");
            assert_eq!(state.path, Some(path));
            assert!(state.status.contains("wrote"), "{}", state.status);
        });
    }

    #[test]
    fn pin_hashes_writes_hash_pins() {
        let home = demo_tree_home();
        let cfg = config_for(home.path());
        let libraries = libraries_for(home.path());
        crate::config::with_home(home.path(), || {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join("pinned.toml");
            std::fs::write(
                &path,
                "schema = 1\ntask = \"t\"\nadapter = \"none\"\n\
                 [[workers]]\nname = \"w\"\npack = [\"demo-review\"]\n",
            )
            .unwrap();
            let mut state = EditorState::load(path.clone(), &cfg, &libraries).unwrap();
            handle_event(&mut state, &press(KeyCode::Down));
            handle_event(&mut state, &press(KeyCode::Char(' ')));
            handle_event(&mut state, &press(KeyCode::Char('p')));
            save(&mut state, &cfg, &libraries).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let pinned = text
                .lines()
                .find(|line| line.contains("demo-review@"))
                .unwrap_or_else(|| panic!("no hash pin written: {text}"));
            let hash = pinned
                .split("@sha256:")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(hash.len(), 64, "{pinned}");
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "{pinned}");
        });
    }
}

