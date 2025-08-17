mod command_registry;
mod commands;
mod helper;
mod config;
mod features;

use command_registry::{CommandDescriptor};
use dotenvy::dotenv;
use std::{collections::HashMap, env, sync::Arc};
use std::sync::Mutex;
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};
use once_cell::sync::Lazy;
use rustyline::Editor;
use rustyline::history::DefaultHistory;
use crate::config::Config;
use crate::features::init_features;

pub static CONFIG: Lazy<Arc<Config>> = Lazy::new(|| {
    Arc::new(Config::load())
});

pub static EDITOR: Lazy<Arc<Mutex<Editor<(), DefaultHistory>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(Editor::new().unwrap()))
});

#[tokio::main]
async fn main() -> () {
    dotenv().ok();

    // Initialize features from config
    let mut features = init_features();

    // Start enabled features
    for feature in features.iter_mut() {
        if feature.is_enabled() {
            feature.start();
            println!("Feature '{}' started.", feature.name());
        }
    }

    // --- Load env & config ---
    let username = Arc::new(env::var("BOT_USERNAME")
        .expect("BOT_USERNAME missing in .env"));
    let oauth = env::var("BOT_OAUTH_TOKEN")
        .expect("BOT_OAUTH_TOKEN missing in .env");

    // Spawn Twitch bot in a new async task
    tokio::spawn(async move {
        run_twitch_bot(&username.clone(), &oauth.clone()).await;
    });

    println!("Type 'help' for a list of commands.");
    // TODO: Replace CLI using ratatui
    loop {
        let readline = EDITOR.lock().unwrap().readline("> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                let locked_features = &mut features;

                match input {
                    "list" => {
                        for f in locked_features.iter() {
                            println!(
                                "{} - {}",
                                f.name(),
                                if f.is_enabled() { "Enabled" } else { "Disabled" }
                            );
                        }
                    }
                    cmd if cmd.starts_with("start ") => {
                        let name = cmd.strip_prefix("start ").unwrap();
                        if let Some(f) = locked_features.iter_mut().find(|f| f.name() == name) {
                            f.start();
                            println!("Feature '{}' started.", name);
                        } else {
                            println!("Feature '{}' not found.", name);
                        }
                    }
                    cmd if cmd.starts_with("stop ") => {
                        let name = cmd.strip_prefix("stop ").unwrap();
                        if let Some(f) = locked_features.iter_mut().find(|f| f.name() == name) {
                            f.stop();
                            println!("Feature '{}' stopped.", name);
                        } else {
                            println!("Feature '{}' not found.", name);
                        }
                    }
                    "help" => println!("Available commands: list, start <feature>, stop <feature>, help, quit, exit"),
                    "quit" | "exit" => std::process::exit(0),
                    _ => println!("Unknown command"),
                }
            }
            Err(_) => {
                println!("Error reading line");
                break;
            }
        }
    }
}

async fn run_twitch_bot(username: &str, oauth: &str) {
    // --- Create client ---
    let client_config = ClientConfig::new_simple(StaticLoginCredentials::new(
        username.parse().unwrap(),
        Some(oauth.parse().unwrap()),
    ));

    let (mut incoming, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

    let client = Arc::new(client);

    println!("Logged in as {}", username);

    println!(
        "Joining {} channels: {}",
        CONFIG.channels.len(),
        CONFIG.channels.join(", ")
    );

    for name in &CONFIG.channels {
        client.join(name.clone()).expect(format!("Failed to join channel {}", name).as_str());
    }

    // --- Build a command map for fast lookup ---
    let mut command_map: HashMap<&'static str, &'static CommandDescriptor> = HashMap::new();
    for cmd in commands::COMMANDS {
        command_map.insert(cmd.name, cmd);
    }

    println!(
        "Registered {} commands: {}",
        command_map.len(),
        command_map.keys().cloned().collect::<Vec<_>>().join(", ")
    );

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