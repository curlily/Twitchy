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
    channels: Vec<String>,
    ai_prompt: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    // --- Load env & config ---
    let username = Arc::new(env::var("TWITCH_USERNAME")?);
    let oauth = env::var("TWITCH_OAUTH")?;
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
        cfg.channels.join(", ")
    );
    for ch in &cfg.channels {
        client.join(ch.clone())?;
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

                    // Remove all occurrences of @botname (case-insensitive)
                    let message_for_ai = &msg_clone.message_text
                        .split_whitespace()
                        .filter(|word| word.to_lowercase() != format!("@{}", &username_ref.to_lowercase()))
                        .collect::<Vec<_>>()
                        .join(" ");

                    if let Some(ai_reply) = helper::ai_response::call_gemini_api(&cfg_ref.ai_prompt, message_for_ai).await {
                        let _ = client_ref.say_in_reply_to(&msg_clone, ai_reply).await;
                    }
                }
            });
        }
    }

    Ok(())
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