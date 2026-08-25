mod config;
mod helpers;
mod input;
mod render;
mod state;

use color_eyre::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, EnableFocusChange, Event, KeyCode,
    KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use ratatui::DefaultTerminal;
use ratatui_image::picker::Picker;
use state::{AppState, CHAT_RELOAD_INTERVAL, DEBOUNCE_DURATION, MESSAGE_RELOAD_INTERVAL};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    // Query terminal capabilities before sending keyboard-enhancement sequences so
    // the picker's stdin reads don't race with enhancement-protocol responses.
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    let _ = execute!(
        std::io::stdout(),
        EnableFocusChange,
        EnableBracketedPaste,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
    );
    let result = run(terminal, picker);
    let _ = execute!(
        std::io::stdout(),
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste
    );
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal, picker: Picker) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let initial_state = if let Some(saved) = config::load_saved_config() {
        match config::try_login(
            saved.host.clone(),
            saved.password.clone(),
            saved.send_method.clone(),
            picker.clone(),
        ) {
            Ok(mut main) => {
                main.load_contacts(tx.clone());
                if !main.chats.is_empty() {
                    main.load_messages(0, tx.clone());
                }
                AppState::Main(Box::new(main))
            }
            Err(_) => AppState::Login(config::initial_login_fields(&saved)),
        }
    } else {
        AppState::Login(state::LoginFields {
            host: String::new(),
            password: String::new(),
            use_private_api: false,
            active_field: 0,
            error: None,
        })
    };

    let mut state = initial_state;

    loop {
        // ── Periodic timers ───────────────────────────────────────────────
        if let AppState::Main(main) = &mut state {
            let now = Instant::now();

            if let Some(fade_start) = main.notification_fade_start
                && now.duration_since(fade_start) >= Duration::from_secs(1)
            {
                main.tui_notifications.clear();
                main.notification_fade_start = None;
            }

            if let Some(exp) = main.status_expires
                && now >= exp
            {
                main.attachment_status = None;
                main.status_expires = None;
            }

            // Chat reload first so notifications fire before messages appear in the UI.
            if now.duration_since(main.last_chat_reload) >= CHAT_RELOAD_INTERVAL {
                main.reload_chats(tx.clone());
                main.last_chat_reload = Instant::now();
            }

            if let Some(idx) = main.pending_load {
                if now.duration_since(main.last_nav) >= DEBOUNCE_DURATION {
                    main.load_messages(idx, tx.clone());
                    main.pending_load = None;
                    main.last_message_reload = Instant::now();
                }
            } else if now.duration_since(main.last_message_reload) >= MESSAGE_RELOAD_INTERVAL {
                if let Some(idx) = main.selected_chat_index {
                    main.load_messages(idx, tx.clone());
                }
                main.last_message_reload = Instant::now();
            }
        }

        // ── Draw ──────────────────────────────────────────────────────────
        terminal.draw(|f| render::render(f, &mut state))?;

        // ── Handle Background Responses ───────────────────────────────────
        while let Ok(response) = rx.try_recv() {
            if let AppState::Main(main) = &mut state {
                main.handle_api_response(response);
            }
        }

        // ── Execute pending blocking operations ───────────────────────────
        if let AppState::Main(main) = &mut state {
            if let Some((guid, name)) = main.pending_attachment_open.take() {
                main.execute_download(&guid, &name, tx.clone());
            }
            for path in std::mem::take(&mut main.pending_file_attachments) {
                main.execute_send_file(path, tx.clone());
            }
            for (guid, name) in std::mem::take(&mut main.pending_image_downloads) {
                main.execute_image_download(guid, name, tx.clone());
            }
            if main.pending_send {
                main.pending_send = false;
                main.execute_send(tx.clone());
            }
        }

        // ── Input ─────────────────────────────────────────────────────────
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::FocusGained => {
                    if let AppState::Main(main) = &mut state {
                        main.terminal_focused = true;
                    }
                }
                Event::FocusLost => {
                    if let AppState::Main(main) = &mut state {
                        main.terminal_focused = false;
                    }
                }
                Event::Paste(text) => {
                    input::handle_paste(&mut state, text);
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        return Ok(());
                    }
                    if input::handle_key(&mut state, key.code, key.modifiers, tx.clone(), &picker) {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }
}
