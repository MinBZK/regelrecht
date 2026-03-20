use crate::app::{App, Tab};
use ratatui::prelude::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.active_tab {
        Tab::Dashboard => "0-9:tabs",
        Tab::Bdd => "Enter:run  a:all  j/k:nav  Tab:focus",
        Tab::Engine => "Enter:eval  j/k:nav  Tab:field",
        Tab::Corpus => "Enter:open  Space:expand  j/k:nav  Tab:focus",
        Tab::Pipeline => "r:refresh  j/k:nav",
        Tab::Validation => "Enter:validate  a:all  j/k:nav",
        Tab::Trace => "Space:collapse  j/k:nav  e:expand  c:collapse",
        Tab::Dependencies => "j/k:nav  J/K:scroll detail",
        Tab::Logs => "e/w/i/d:filter  f:follow  j/k:nav",
        Tab::Actions => "f:fmt  l:lint  b:build  v:validate  c:check  t:test",
    };

    let line = Line::from(vec![
        Span::styled(
            " q:quit  ?:help  ",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled("│ ", Style::default().add_modifier(Modifier::DIM)),
        Span::styled(hints, Style::default().add_modifier(Modifier::DIM)),
    ]);

    frame.render_widget(line, area);
}
