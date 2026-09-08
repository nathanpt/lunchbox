use crate::tui::menu::{chrome, Action};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[derive(Default)]
pub struct HomeState {
    pub cursor: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Pantry,
    Manifests,
    Doctor,
    Policy,
}

impl Route {
    fn label(self) -> &'static str {
        match self {
            Route::Pantry => "Pantry",
            Route::Manifests => "Manifests",
            Route::Doctor => "Doctor",
            Route::Policy => "Policy",
        }
    }
}

pub const ROWS: [Route; 4] = [Route::Pantry, Route::Manifests, Route::Doctor, Route::Policy];

pub fn render(state: &mut HomeState, frame: &mut Frame, area: Rect) {
    let mut lines = Vec::new();
    for (index, route) in ROWS.iter().enumerate() {
        let marker = if index == state.cursor { "▸ " } else { "  " };
        let label = route.label().to_string();
        if index == state.cursor {
            lines.push(Line::from(vec![Span::raw(marker), chrome::bold(label)]));
        } else {
            lines.push(Line::from(vec![Span::raw(marker), Span::raw(label)]));
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub fn handle_event(state: &mut HomeState, event: &Event) -> Action {
    let Event::Key(key) = event else {
        return Action::Continue;
    };
    if key.kind != KeyEventKind::Press {
        return Action::Continue;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            state.cursor = state.cursor.saturating_sub(1);
            Action::Continue
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            state.cursor = (state.cursor + 1).min(ROWS.len() - 1);
            Action::Continue
        }
        (KeyCode::Enter, _) => Action::Open(ROWS[state.cursor]),
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

    fn press(code: KeyCode) -> Event {
        Event::Key(KeyEvent::from(code))
    }

    #[test]
    fn home_screen_rows() {
        let mut state = HomeState::default();
        handle_event(&mut state, &press(KeyCode::Down));
        let mut terminal = Terminal::new(TestBackend::new(64, 8)).unwrap();
        let frame = terminal
            .draw(|frame| render(&mut state, frame, frame.area()))
            .unwrap();
        insta::assert_snapshot!(frame_to_string(frame.buffer, frame.area));
    }

    #[test]
    fn cursor_moves_and_opens_routes() {
        let mut state = HomeState::default();
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Enter)),
            Action::Open(Route::Pantry)
        ));
        handle_event(&mut state, &press(KeyCode::Down));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Enter)),
            Action::Open(Route::Manifests)
        ));
        handle_event(&mut state, &press(KeyCode::Down));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Enter)),
            Action::Open(Route::Doctor)
        ));
        handle_event(&mut state, &press(KeyCode::Down));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Enter)),
            Action::Open(Route::Policy)
        ));
        for _ in 0..8 {
            handle_event(&mut state, &press(KeyCode::Down));
        }
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Up)),
            Action::Continue
        ));
        assert!(matches!(
            handle_event(&mut state, &press(KeyCode::Esc)),
            Action::Quit
        ));
    }
}
