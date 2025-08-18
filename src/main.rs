mod command_registry;
mod commands;
mod helper;
mod config;
mod features;

use crate::config::Config;
use crate::features::init_features;
use command_registry::{CommandDescriptor};
use dotenvy::dotenv;
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};
use once_cell::sync::Lazy;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{Block, Borders, Paragraph},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color},
    text::{Span, Line},
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use std::{
    collections::HashMap,
    env,
    sync::Arc,
    io,
    sync::mpsc,
    time::Duration,
};
use std::sync::mpsc::Sender;
use crossterm::event::KeyEventKind;
use ratatui::widgets::{List, ListItem};

pub static CONFIG: Lazy<Arc<Config>> = Lazy::new(|| {
    Arc::new(Config::load())
});

fn main() -> Result<(), Box<dyn std::error::Error>> {

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (log_tx, log_rx) = mpsc::channel::<String>();
    let mut logs: Vec<String> = vec!["Welcome to Twitchy!".to_string()];
    let mut input = String::new();
    let mut character_index = 0;

    dotenv().ok();

    let username = Arc::new(env::var("BOT_USERNAME")
        .expect("BOT_USERNAME missing in .env"));
    let oauth = env::var("BOT_OAUTH_TOKEN")
        .expect("BOT_OAUTH_TOKEN missing in .env");

    let mut features = init_features();

    for feature in features.iter_mut() {
        if feature.is_enabled() {
            feature.start();
            log_tx.send(format!("Feature '{}' started.", feature.name())).unwrap();
        }
    }

    let log_tx_clone = log_tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(run_twitch_bot(&username, &oauth, log_tx_clone));
    });

    let mut should_quit = false;
    while !should_quit {
        // Drain all new log messages
        while let Ok(msg) = log_rx.try_recv() {
            logs.push(msg);
            if logs.len() > 10 {
                logs.drain(0..logs.len() - 10);
            }
        }

        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(3),
                    Constraint::Length(3),
                ].as_ref())
                .split(f.size());

            // Logs/messages
            let log_lines: Vec<ListItem> = logs.iter().map(|l| ListItem::new(Line::from(Span::raw(l.as_str())))).collect();
            let log_widget = List::new(log_lines)
                .block(Block::default().borders(Borders::ALL).title("Logs"))
                .style(Style::default().fg(Color::White));
            f.render_widget(log_widget, chunks[0]);

            // Input box
            let input_widget = Paragraph::new(input.as_str())
                .block(Block::default().borders(Borders::ALL).title("Input"))
                .style(Style::default().fg(Color::Yellow));
            f.render_widget(input_widget, chunks[1]);
            f.set_cursor(
                chunks[1].x + character_index as u16 + 1,
                chunks[1].y + 1,
            );
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char(c) => {
                            let idx = byte_index(&input, character_index);
                            input.insert(idx, c);
                            character_index = clamp_cursor(&input, character_index + 1);
                        }
                        KeyCode::Backspace => {
                            if character_index > 0 {
                                let idx = byte_index(&input, character_index);
                                let prev_idx = byte_index(&input, character_index - 1);
                                input.replace_range(prev_idx..idx, "");
                                character_index -= 1;
                            }
                        }
                        KeyCode::Left => {
                            if character_index > 0 {
                                character_index -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if character_index < input.chars().count() {
                                character_index += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if !input.trim().is_empty() {

                                log_tx.send(format!("> {}", input)).unwrap();

                                match input.trim() {
                                    "list" => {
                                        for f in features.iter() {
                                            log_tx.send(format!(
                                                "{} - {}",
                                                f.name(),
                                                if f.is_enabled() { "Enabled" } else { "Disabled" }
                                            )).unwrap();
                                        }
                                    }
                                    input if input.starts_with("start ") => {
                                        let name = input.strip_prefix("start ").unwrap();
                                        if let Some(f) = features.iter_mut().find(|f| f.name() == name) {
                                            f.start();
                                            log_tx.send(format!("Feature '{}' started.", name)).unwrap();
                                        } else {
                                            log_tx.send(format!("Feature '{}' not found.", name)).unwrap();
                                        }
                                    }
                                    input if input.starts_with("stop ") => {
                                        let name = input.strip_prefix("stop ").unwrap();
                                        if let Some(f) = features.iter_mut().find(|f| f.name() == name) {
                                            f.stop();
                                            log_tx.send(format!("Feature '{}' stopped.", name)).unwrap();
                                        } else {
                                            log_tx.send(format!("Feature '{}' not found.", name)).unwrap();
                                        }
                                    }
                                    "help" => log_tx.send("Available commands: help, list, start <feature>, stop <feature>".to_string()).unwrap(),
                                    _ => log_tx.send(format!("Unknown command '{}' ~ Type 'help' for a list of commands!", input.clone()).to_string()).unwrap(),
                                }

                                input.clear();
                                character_index = 0;
                            }
                        }
                        KeyCode::Esc => should_quit = true,
                        _ => {}
                    }
                }
            }
        }
    }

    // Clean up terminal before exit
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// Helper: get byte index for char position
fn byte_index(input: &str, char_pos: usize) -> usize {
    input.char_indices().map(|(i, _)| i).nth(char_pos).unwrap_or(input.len())
}

// Helper: clamp cursor position
fn clamp_cursor(input: &str, new_cursor_pos: usize) -> usize {
    new_cursor_pos.clamp(0, input.chars().count())
}

async fn run_twitch_bot(username: &str, oauth: &str, log_tx: Sender<String>) {
    // --- Create client ---
    let client_config = ClientConfig::new_simple(StaticLoginCredentials::new(
        username.parse().unwrap(),
        Some(oauth.parse().unwrap()),
    ));

    let (mut incoming, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

    let client = Arc::new(client);

    log_tx.send(format!("--- Logged in as {} ---", username)).unwrap();
    log_tx.send(format!("Joining {} channels: {}",
        CONFIG.channels.len(),
        CONFIG.channels.join(", ")
    )).unwrap();

    for name in &CONFIG.channels {
        client.join(name.clone()).expect(format!("Failed to join channel {}", name).as_str());
    }

    // --- Build a command map for fast lookup ---
    let mut command_map: HashMap<&'static str, &'static CommandDescriptor> = HashMap::new();
    for cmd in commands::COMMANDS {
        command_map.insert(cmd.name, cmd);
    }

    log_tx.send(format!("Registered {} commands: {}",
        command_map.len(),
        command_map.keys().cloned().collect::<Vec<_>>().join(", ")
    )).unwrap();


    // --- Main message loop ---
    while let Some(message) = incoming.recv().await {
        if let ServerMessage::Privmsg(msg) = message {
            let username_ref = username.to_lowercase();
            let client_ref = client.clone();
            let command_map = command_map.clone();

            tokio::spawn(async move {

                // Handle commands
                if let Some((cmd_name, args)) = parse_command(&msg) {
                    if let Some(descriptor) = command_map.get(cmd_name.as_str()) {
                        if let Some(reply) =
                            (descriptor.run)(client_ref.clone(), msg.clone(), args).await
                        {
                            let _ = client_ref.say_in_reply_to(&msg, reply).await;
                        }
                        return; // skip AI if a command matched
                    }
                }

                // Handle mentions for AI reply
                if msg
                    .message_text
                    .to_lowercase()
                    .contains(&username_ref.to_lowercase())
                {
                    // Remove all occurrences of @username (case-insensitive)
                    let message_without_mention = &msg.message_text
                        .split_whitespace()
                        .filter(|word| word.to_lowercase() != format!("@{}", &username_ref.to_lowercase()))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let emotes = helper::fetch_user_emotes::fetch_user_emotes(&*msg.channel_id).await.unwrap_or_default();

                    let prompt = format!(
                        "{}\nYou can use any of the following emotes: {}\nRespond in one paragraph to this message:\n\"{}\"",
                        CONFIG.ai_prompt, emotes.join(", "), message_without_mention
                    );

                    if let Some(ai_reply) = helper::ai_response::call_gemini_api(&prompt).await {
                        let _ = client_ref.say_in_reply_to(&msg, fix_emote_spacing(&emotes, &ai_reply)).await;
                    }
                }
            });
        }
    }
}

pub fn fix_emote_spacing(emotes: &Vec<String>, input: &str) -> String {
    let mut output = input.to_string();

    for emote in emotes {
        let mut i = 0;
        while let Some(pos) = output[i..].find(emote) {
            let pos = i + pos;
            let end = pos + emote.len();

            // Check if already followed by a space or end of string
            if output.chars().nth(end).map(|c| c != ' ').unwrap_or(false) {
                output.insert(end, ' ');
                i = end + 1; // skip past what we just inserted
            } else {
                i = end + 1;
            }
        }
    }

    output
}

/// Parse a message starting with '!' into (command_name, args_vec)
fn parse_command(msg: &PrivmsgMessage) -> Option<(String, Vec<String>)> {
    let text = msg.message_text.trim();
    if !text.starts_with('!') {
        return None;
    }
    let mut parts = text[1..].split_whitespace();
    let cmd = parts.next()?.to_lowercase();
    let args = parts.map(|s| s.to_string()).collect();
    Some((cmd, args))
}