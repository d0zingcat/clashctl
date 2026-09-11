use bytesize::ByteSize;
use chrono::Utc;
use tui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

use crate::{
    HMS,
    components::{FooterItem, MovableList, MovableListItem},
    define_widget,
    interactive::clashctl::model::ConnectionWithSpeed,
    ui::state::ConnectionCloseState,
};

define_widget!(ConnectionPage);

impl<'a> Widget for ConnectionPage<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        MovableList::new("Connections", &self.state.con_state)
            .footer_hint(FooterItem::span(Span::styled(
                " [K] Close all ",
                Style::default().fg(Color::LightRed),
            )))
            .render(area, buf);

        if !matches!(
            self.state.connection_close_state,
            ConnectionCloseState::Idle
        ) {
            self.render_connection_close_dialog(area, buf);
        }
    }
}

impl<'a> ConnectionPage<'a> {
    fn render_connection_close_dialog(&self, area: Rect, buf: &mut Buffer) {
        let dialog = centered_rect(area, 60, 7);
        let (title, content, color) = match &self.state.connection_close_state {
            ConnectionCloseState::Confirming { count } => (
                " Close all connections? ",
                format!(
                    "Close all {count} active connections?\n\n[Y] Confirm    [N] / [Esc] Cancel"
                ),
                Color::Yellow,
            ),
            ConnectionCloseState::Closing { count } => (
                " Closing connections ",
                format!("Closing all {count} active connections..."),
                Color::LightBlue,
            ),
            ConnectionCloseState::Result { count, error: None } => (
                " Connections closed ",
                format!("Closed {count} connections.\n\n[Enter] [Space] [Esc] Dismiss"),
                Color::Green,
            ),
            ConnectionCloseState::Result {
                count,
                error: Some(error),
            } => (
                " Could not close connections ",
                format!(
                    "Could not close {count} connections:\n{error}\n\n[Enter] [Space] [Esc] \
                     Dismiss"
                ),
                Color::Red,
            ),
            ConnectionCloseState::Idle => return,
        };

        Clear.render(dialog, buf);
        Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(title, Style::default().fg(color)))
                    .style(Style::default().fg(color)),
            )
            .wrap(Wrap { trim: true })
            .render(dialog, buf);
    }
}

fn centered_rect(area: Rect, width_percent: u16, height: u16) -> Rect {
    let width = area.width.saturating_mul(width_percent).saturating_div(100);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

impl<'a> MovableListItem<'a> for ConnectionWithSpeed {
    fn to_spans(&self) -> Spans<'a> {
        let dimmed = Style::default().fg(Color::DarkGray);
        let bolded = Style::default().add_modifier(Modifier::BOLD);
        let (dl, up) = (
            ByteSize(self.connection.download).to_string_as(true),
            ByteSize(self.connection.upload).to_string_as(true),
        );
        let (dl_speed, up_speed) = (
            ByteSize(self.download.unwrap_or_default()).to_string_as(true) + "/s",
            ByteSize(self.upload.unwrap_or_default()).to_string_as(true) + "/s",
        );
        let meta = &self.connection.metadata;
        let host = format!("{}:{}", meta.host, meta.destination_port);

        let src = format!("{}:{} ", meta.source_ip, meta.source_port);
        let dest = format!(
            " {}:{}",
            if meta.destination_ip.is_empty() {
                "?"
            } else {
                &meta.destination_ip
            },
            meta.source_port
        );
        let dash: String = "─".repeat(44_usize.saturating_sub(src.len() + dest.len()).max(1));

        let time = (Utc::now() - self.connection.start).hms();
        vec![
            Span::styled(format!("{:45}", host), bolded),
            // Download size
            Span::styled(" ▼  ", dimmed),
            Span::raw(format!("{:12}", dl)),
            // Download speed
            Span::styled(" ⇊  ", dimmed),
            Span::raw(format!("{:12}", dl_speed)),
            // Upload size
            Span::styled(" ▲  ", dimmed),
            Span::raw(format!("{:12}", up)),
            // Upload Speed
            Span::styled(" ⇈  ", dimmed),
            Span::raw(format!("{:12}", up_speed)),
            // Time
            Span::styled(" ⏲  ", dimmed),
            Span::raw(format!("{:10}", time)),
            // Rule
            Span::styled(" ✤  ", dimmed),
            Span::raw(format!("{:15}", self.connection.rule)),
            // IP
            Span::styled(" ⇄  ", dimmed),
            Span::raw(src),
            Span::styled(dash, dimmed),
            Span::raw(dest),
            // Chain
            Span::styled("   ⟴  ", dimmed),
            Span::raw(self.connection.chains.join(" - ")),
        ]
        .into()
    }
}
