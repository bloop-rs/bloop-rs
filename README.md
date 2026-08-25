# Bloop 💬

A terminal client for [BlueBubbles](https://bluebubbles.app/) - the open-source iMessage relay server for non-Apple devices.

If you live in the terminal or don't want to open a terribly optimized browser or Electron app just to reply to a text, this is for you.

## What it does

Connects to a running BlueBubbles server and gives you a full iMessage interface in your terminal: browse conversations, read message history, send messages, and view attachments.

## Features

- 📋 Chat list with unread indicators
- 💬 Full message history with pagination
- ✉️ Send messages with live feedback
- 📎 Attachment sending and viewing (opens in your system's default app)
- 🔔 Desktop notifications for new messages
- ⌨️ Keyboard-driven
- 😃 Emoji Picker

## Requirements

- A running [BlueBubbles server](https://bluebubbles.app/) with network access from your machine
- Linux or macOS

## Build

```
cargo build --release
./target/release/bloop
```

## Install

```
cargo install --path .
```

On first launch you'll be prompted for your BlueBubbles server URL and password. These are saved locally (`~/.config/bloop` on Linux) so you won't need to enter them again.

## Usage

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate chats or messages |
| `←` / `→` | Switch between chat list and message pane |
| `i` | Compose a new message |
| `Enter` | Open attachment (when a message with attachments is selected) |
| `q` / `Esc` | Go back / quit |
| `:` | Open the emoji picker |
| `@@` | Open the file browser |
| `Alt+Enter` | Newline |
| `Ctrl+K` | Search for chat |