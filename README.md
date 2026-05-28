# Twitchy

A Twitch chat bot written in Rust with a terminal UI. Connects to one or more channels, handles chat commands, replies to mentions using Google Gemini, and includes an experimental Windows-only feature that automatically updates your stream category based on what you have open.

## Features

- **Terminal UI** - ratatui-based dashboard with a live log panel and input box; press `Esc` to exit
- **Chat commands** - `!hello` and `!dice [sides]` out of the box, easily extensible
- **AI responses** - when the bot is @mentioned in chat, it replies via Gemini 2.0 Flash, with awareness of the channel's 7TV emotes
- **Auto Category** *(experimental, Windows only)* - polls the active foreground window, matches the executable name against Twitch game names, and updates your stream category automatically. See known issues below.

## Setup

### Prerequisites

- Rust (stable)
- A Twitch account for the bot with an OAuth token
- A Twitch application (Client ID + OAuth token with `channel:manage:broadcast` scope) for the Helix API
- A Google Gemini API key

### 1. Configure environment

Copy `.env.example` to `.env` and fill in your credentials:

```env
TWITCH_USERNAME=xxxxxxxxxxxxxx
TWITCH_OAUTH=xxxxxxxxxxxxxx
GEMINI_API_KEY=xxxxxxxxxxxxxx
```

### 2. Configure the bot

Edit `Config.toml`:

```toml
channels = ["yourchannel"]
ai_prompt = "You are a friendly Twitch chat bot."
own_user_id = "123456789"

[features]
auto_category = false
auto_reply = true
```

### 3. Run

```bash
cargo run
```

## Terminal commands

Once running, type into the input box and press Enter:

| Command | Description |
|---|---|
| `help` | List available commands |
| `list` | Show all features and their status |
| `start <feature>` | Start a feature (e.g. `start auto_category`) |
| `stop <feature>` | Stop a feature |

## Adding commands

Each command is a file in `src/commands/` that exposes a `CommandDescriptor`. See `dice.rs` or `hello.rs` for examples, then register it in `src/commands/mod.rs`.

## Known issues

Auto Category is experimental and currently broken. This feature was a work in progress and development has been put on hold.
