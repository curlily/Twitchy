use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;
use crate::command_registry::{CommandDescriptor, CommandFuture, TwitchClient};

fn run(_client: Arc<TwitchClient>, _msg: PrivmsgMessage, args: Vec<String>) -> CommandFuture {

    let sides = args.get(0) // get the first argument if it exists
        .and_then(|s| s.parse::<u32>().ok()) // try parsing to u32
        .filter(|&n| n > 2) // at least 3 sides
        .unwrap_or(6); // fallback to default

    let roll = rand::random::<u32>() % sides + 1;
    let reply = format!("You rolled a {}! 🎲", roll);

    Box::pin(async move {
        Some(reply)
    })
}

// expose descriptor
pub const DESCRIPTOR: CommandDescriptor = CommandDescriptor {
    name: "dice",
    run,
};