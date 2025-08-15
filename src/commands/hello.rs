use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;
use crate::command_registry::{CommandDescriptor, CommandFuture, TwitchClient};

fn run(_client: Arc<TwitchClient>, msg: PrivmsgMessage, _args: Vec<String>) -> CommandFuture {
    Box::pin(async move {
        Some(format!("Hello, {}!", msg.sender.name))
    })
}

// expose descriptor
pub const DESCRIPTOR: CommandDescriptor = CommandDescriptor {
    name: "hello",
    run,
};