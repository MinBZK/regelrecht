use crate::app::App;
use crate::components::{status_bar, tabs};
use ratatui::prelude::*;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let layout = Layout::vertical([
        Constraint::Length(1), // Tab bar
        Constraint::Min(1),    // Content area
        Constraint::Length(1), // Status bar
    ])
    .split(area);

    tabs::render(frame, app, layout[0]);
    render_active_view(frame, app, layout[1]);
    status_bar::render(frame, app, layout[2]);

    if app.show_help {
        render_help_overlay(frame, area);
    }
}

fn render_active_view(frame: &mut Frame, app: &App, area: Rect) {
    use crate::app::Tab;

    match app.active_tab {
        Tab::Dashboard => app.dashboard.render(frame, area),
        Tab::Bdd => app.bdd.render(frame, area),
        Tab::Engine => app.engine.render(frame, area),
        Tab::Corpus => app.corpus.render(frame, area),
        Tab::Pipeline => app.pipeline.render(frame, area),
        Tab::Validation => app.validation.render(frame, area),
        Tab::Trace => app.trace.render(frame, area),
        Tab::Dependencies => app.deps.render(frame, area),
        Tab::Logs => app.logs.render(frame, area),
        Tab::Actions => app.actions.render(frame, area),
    }
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    // Center a help box
    let width = 50.min(area.width.saturating_sub(4));
    let height = 18.min(area.height.saturating_sub(2));
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let help_area = Rect::new(x, y, width, height);

    // Clear background
    frame.render_widget(ratatui::widgets::Clear, help_area);

    let help_text = vec![
        Line::from(Span::styled(
            " Key Bindings ",
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" 1-9       ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Switch tab"),
        ]),
        Line::from(vec![
            Span::styled(" Tab       ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Next tab"),
        ]),
        Line::from(vec![
            Span::styled(" Shift+Tab ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Previous tab"),
        ]),
        Line::from(vec![
            Span::styled(" j/k       ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Navigate down/up"),
        ]),
        Line::from(vec![
            Span::styled(" Enter     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Select / execute"),
        ]),
        Line::from(vec![
            Span::styled(" Space     ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Toggle expand/collapse"),
        ]),
        Line::from(vec![
            Span::styled(" Esc       ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Back / cancel"),
        ]),
        Line::from(vec![
            Span::styled(" g/G       ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Go to top/bottom"),
        ]),
        Line::from(vec![
            Span::styled(" /         ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Filter / search"),
        ]),
        Line::from(vec![
            Span::styled(" q         ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled(" ?         ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Press any key to close ",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];

    let help = ratatui::widgets::Paragraph::new(help_text)
        .block(ratatui::widgets::Block::bordered().style(Style::default()));

    frame.render_widget(help, help_area);
}
