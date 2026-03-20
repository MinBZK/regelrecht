use crate::app::{App, Tab};
use ratatui::prelude::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    for (i, tab) in Tab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " │ ",
                Style::default().add_modifier(Modifier::DIM),
            ));
        }

        let label = format!("{}:{}", tab.key_label(), tab.label());

        if *tab == app.active_tab {
            spans.push(Span::styled(
                label,
                Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
            ));
        } else {
            spans.push(Span::styled(
                label,
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
    }

    let line = Line::from(spans);
    frame.render_widget(line, area);
}
