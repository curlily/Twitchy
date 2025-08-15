use twitch_irc::SecureTCPTransport;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;

// Twitch client type alias
pub type TwitchClient = twitch_irc::TwitchIRCClient<SecureTCPTransport, twitch_irc::login::StaticLoginCredentials>;
pub type CommandFuture = Pin<Box<dyn Future<Output = Option<String>> + Send>>;

pub struct CommandDescriptor {
    pub name: &'static str,
    pub run: fn(Arc<TwitchClient>, PrivmsgMessage, Vec<String>) -> CommandFuture,
}
