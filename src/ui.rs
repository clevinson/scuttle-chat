use crate::app::{App, AppMode, UiStyles};
use crate::chat::{ChatMsg, ChatSender};
use std::io;
use tui::backend::Backend;
use tui::layout::{Constraint, Corner, Direction, Layout, Margin, ScrollMode};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, SelectableList, Text, Widget};
use tui::Terminal;

pub fn draw<'a, B: Backend>(terminal: &mut Terminal<B>, app: &App<'a>) -> Result<(), io::Error> {
    terminal.draw(|mut f| match app.mode {
        AppMode::Debug => {
            let area = Layout::default()
                .direction(Direction::Horizontal)
                .margin(5)
                .constraints([Constraint::Min(1)].as_ref())
                .split(f.size());

            draw_debug_window(&mut f, &app, area[0]);
        }
        _ => {
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)].as_ref())
                .split(f.size());

            draw_status_pane(&mut f, &app, panes[0]);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(5)].as_ref())
                .split(panes[1]);

            match app.selected {
                None => draw_welcome_pane(&mut f, &app, chunks[0]),
                Some(_) => draw_chat_pane(&mut f, &app, chunks[0]),
            };

            draw_input_area(&mut f, &app, chunks[1]);
        }
    })?;

    Ok(())
}

fn draw_status_pane<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let style = Style::default();

    let block_style = match app.mode {
        AppMode::Normal => app.ui_styles.highlighted_block_style,
        _ => app.ui_styles.normal_block_style,
    };

    SelectableList::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Peers")
                .border_style(block_style)
                .title_style(block_style),
        )
        .items(&app.peer_list())
        .select(app.selected)
        .style(style)
        .highlight_style(style.fg(Color::LightGreen).modifier(Modifier::BOLD))
        .highlight_symbol(">")
        .render(f, area);
}

fn draw_debug_window<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let debug_log = app
        .debug_log
        .iter()
        .map(|(evt, level)| {
            Text::styled(
                format!("{}: {}\n", level, evt),
                match *level {
                    "NEW PEER" => app.ui_styles.error_style,
                    "ERROR" => app.ui_styles.critical_style,
                    "DEBUG" => app.ui_styles.warning_style,
                    _ => app.ui_styles.info_style,
                },
            )
        })
        .collect::<Vec<Text>>();

    Paragraph::new(debug_log.iter())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Debug Log")
                .border_style(app.ui_styles.normal_block_style)
                .title_style(app.ui_styles.normal_block_style),
        )
        .wrap(true)
        .render(f, area);
}

fn draw_welcome_pane<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let welcome_text = vec![Text::styled(
        "

  ███████╗ ██████╗██╗   ██╗████████╗████████╗██╗     ███████╗ 
  ██╔════╝██╔════╝██║   ██║╚══██╔══╝╚══██╔══╝██║     ██╔════╝ 
  ███████╗██║     ██║   ██║   ██║      ██║   ██║     █████╗   
  ╚════██║██║     ██║   ██║   ██║      ██║   ██║     ██╔══╝   
  ███████║╚██████╗╚██████╔╝   ██║      ██║   ███████╗███████╗ 
  ╚══════╝ ╚═════╝ ╚═════╝    ╚═╝      ╚═╝   ╚══════╝╚══════╝ 
                     ██████╗██╗  ██╗ █████╗ ████████╗
                    ██╔════╝██║  ██║██╔══██╗╚══██╔══╝
                    ██║     ███████║███████║   ██║   
                    ██║     ██╔══██║██╔══██║   ██║   
                    ╚██████╗██║  ██║██║  ██║   ██║   
                     ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝   

  <k>      : Select up
  <j>      : Select down
  <RETURN> : Start chat with selected peer
  <ESC>    : Return to main menu
  <h>      : Help (not yet implemented)
  <d>      : View debug window
  <q>      : Quit

",
        app.ui_styles.info_style,
    )];

    Paragraph::new(welcome_text.iter())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Welcome!")
                .border_style(app.ui_styles.normal_block_style)
                .title_style(app.ui_styles.normal_block_style),
        )
        .render(f, area);
}

fn draw_chat_pane<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let selected_peer = app
        .selected
        .and_then(|idx| app.available_peers.keys().nth(idx));

    let chat_title = format!(
        "Chat ({})",
        selected_peer.unwrap_or(&"No peer selected".to_string())
    );

    let scroll_offset = app
        .selected_chat()
        .map(|chat| chat.scroll_offset)
        .unwrap_or(0);

    let chat_texts = app
        .selected_chat()
        .map(|chat| {
            chat.messages
                .iter()
                .map(|chat_msg| format_chat_msg(&app.ui_styles, chat_msg))
                .collect::<Vec<Text>>()
        })
        .unwrap_or(vec![Text::styled(
            "No chat initiated. To start a chat, select a user,\
             and press <RETURN> to initiate handshake",
            app.ui_styles.info_style,
        )]);

    Paragraph::new(chat_texts.iter())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(&chat_title)
                .border_style(app.ui_styles.normal_block_style)
                .title_style(app.ui_styles.normal_block_style),
        )
        .wrap(true)
        .scroll_overflow_char(Some('~'))
        .scroll_mode(ScrollMode::Tail)
        .scroll(scroll_offset)
        .render(f, area);
}

fn format_chat_msg<'t>(ui_styles: &UiStyles, chat_msg: &ChatMsg) -> Text<'t> {
    Text::styled(
        format!("{}: {}\n", chat_msg.sender, chat_msg.message),
        match chat_msg.sender {
            ChatSender::_You => ui_styles.error_style,
            ChatSender::Info => ui_styles.info_style,
            ChatSender::Peer(_) => ui_styles.warning_style,
        },
    )
}

fn draw_input_area<'a, B: Backend>(
    f: &mut tui::terminal::Frame<B>,
    app: &App<'a>,
    area: tui::layout::Rect,
) {
    let input_text = app
        .selected_chat()
        .map(|chat| chat.input.clone())
        .unwrap_or("".to_string());

    let (input_block_style, input_text_style) = match app.mode {
        AppMode::Chat(_) => (app.ui_styles.highlighted_block_style, Style::default()),
        _ => (
            app.ui_styles.hidden_block_style,
            Style::default().fg(Color::DarkGray),
        ),
    };

    Paragraph::new([Text::styled(input_text, input_text_style)].iter())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Input")
                .border_style(input_block_style)
                .title_style(input_block_style),
        )
        .wrap(true)
        .scroll_mode(ScrollMode::Tail)
        .scroll(0)
        .render(f, area);
}
