use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, SelectableList, Text, Widget};
use tui::Terminal;
use std::io;
use crate::app::App;
use crate::chat::ChatSender;

pub fn draw<'a, B: Backend>(terminal: &mut Terminal<B>, app: &App<'a>) -> Result<(), io::Error> {
    terminal.draw(|mut f| {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)].as_ref())
            .split(f.size());

        draw_status_pane(&mut f, &app, panes[0]);
        draw_chat_pane(&mut f, &app, panes[1]);
    })?;

    Ok(())
}

fn draw_status_pane<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
        .split(area);

    let style = Style::default().fg(Color::Black).bg(Color::White);
    SelectableList::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Peers"),
        )
        .items(&app.peer_list())
        .select(app.selected)
        .style(style)
        .highlight_style(style.fg(Color::LightGreen).modifier(Modifier::BOLD))
        .highlight_symbol(">")
        .render(f, chunks[0]);

    let debug_log = app
        .debug_log
        .iter()
        .map(|(evt, level)| {
            Text::styled(
                format!("{}: {}\n", level, evt),
                match *level {
                    "NEW PEER" => app.error_style,
                    "ERROR" => app.critical_style,
                    "DEBUG" => app.warning_style,
                    _ => app.info_style,
                },
            )
        })
        .collect::<Vec<Text>>();

    Paragraph::new(debug_log.iter())
        .block(Block::default().borders(Borders::ALL).title("Debug Log"))
        .wrap(true)
        .render(f, chunks[1]);
}

fn draw_chat_pane<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(4)].as_ref())
        .split(area);

    let selected_peer = app
        .selected
        .and_then(|idx| app.available_peers.keys().nth(idx));

    let chat_title = format!(
        "Chat ({})",
        selected_peer.unwrap_or(&"No peer selected".to_string())
    );

    let maybe_chat = selected_peer.and_then(|feed_id| app.chats.get(feed_id));

    let default_chat_text = vec![Text::styled(
        "No chat selected. To initiate a chat, select a user, and press <RETURN>",
        app.info_style,
    )];

    let chat_texts = maybe_chat
        .map(|chat| {
            chat.iter()
                .map(|chat_msg| {
                    Text::styled(
                        format!("{}: {}\n", chat_msg.sender, chat_msg.message),
                        match chat_msg.sender {
                            ChatSender::_You => app.error_style,
                            ChatSender::Info => app.info_style,
                            ChatSender::Peer(_) => app.warning_style,
                        },
                    )
                })
                .collect::<Vec<Text>>()
        })
        .unwrap_or(default_chat_text);

    Paragraph::new(chat_texts.iter())
        .block(Block::default().borders(Borders::ALL).title(&chat_title))
        .wrap(true)
        .render(f, chunks[0]);

    Paragraph::new([Text::raw("")].iter())
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .wrap(true)
        .render(f, chunks[1]);
}
