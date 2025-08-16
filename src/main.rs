mod command_registry;
mod commands;
mod helper;

use command_registry::{CommandDescriptor};
use dotenvy::dotenv;
use serde::Deserialize;
use std::{collections::HashMap, env, fs, sync::Arc};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

#[derive(Deserialize)]
struct Config {
    channels: HashMap<String, String>,
    ai_prompt: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // --- Load env & config ---
    let username = Arc::new(env::var("TWITCH_USERNAME")
        .expect("TWITCH_USERNAME missing in .env"));
    let oauth = env::var("TWITCH_OAUTH")
        .expect("TWITCH_OAUTH missing in .env");
    let cfg = Arc::new(toml::from_str::<Config>(&fs::read_to_string("Config.toml")?)?);

    // --- Create client ---
    let client_config = ClientConfig::new_simple(StaticLoginCredentials::new(
        username.as_ref().clone(),
        Some(oauth),
    ));

    let (mut incoming, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

    let client = Arc::new(client);

    println!("Logged in as {}", username);

    println!(
        "Joining {} channels: {}",
        cfg.channels.len(),
        cfg.channels.keys().cloned().collect::<Vec<_>>().join(", ")
    );

    for (name, _id) in &cfg.channels {
        client.join(name.clone())?;
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
            let client_ref = client.clone();
            let username_ref = username.clone();
            let cfg_ref = Arc::clone(&cfg); // single Arc clone here
            let msg_clone = msg.clone();
            let command_map = command_map.clone();

            tokio::spawn(async move {
                // Handle commands
                if let Some((cmd_name, args)) = parse_command(&msg_clone) {
                    if let Some(descriptor) = command_map.get(cmd_name.as_str()) {
                        if let Some(reply) =
                            (descriptor.run)(client_ref.clone(), msg_clone.clone(), args).await
                        {
                            let _ = client_ref.say_in_reply_to(&msg_clone, reply).await;
                        }
                        return; // skip AI if a command matched
                    }
                }

                // Handle mentions for AI reply
                if msg_clone
                    .message_text
                    .to_lowercase()
                    .contains(&username_ref.to_lowercase())
                {
                    // Remove all occurrences of @username (case-insensitive)
                    let message_without_mention = &msg_clone.message_text
                        .split_whitespace()
                        .filter(|word| word.to_lowercase() != format!("@{}", &username_ref.to_lowercase()))
                        .collect::<Vec<_>>()
                        .join(" ");

                    let user_id = cfg_ref.channels.get(&msg.channel_login)
                        .expect(&format!("Missing user ID for {}", &msg.channel_login));

                    let emotes = helper::fetch_user_emotes::fetch_user_emotes(user_id).await.unwrap_or_default();

                    let prompt = format!(
                        "{}\nYou can use any of the following emotes: {}\nRespond in one paragraph to this message:\n\"{}\"",
                        cfg_ref.ai_prompt, emotes.join(", "), message_without_mention
                    );

                    if let Some(ai_reply) = helper::ai_response::call_gemini_api(&prompt).await {
                        let _ = client_ref.say_in_reply_to(&msg_clone, fix_emote_spacing(&emotes, &ai_reply)).await;
                    }
                }
            });
        }
    }

    Ok(())
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