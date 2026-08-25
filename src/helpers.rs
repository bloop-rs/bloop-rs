use std::path::PathBuf;

pub(crate) fn word_right(s: &str, cursor: usize) -> usize {
    let mut i = cursor;
    let mut iter = s.chars().skip(cursor).peekable();
    while iter.peek().map(|c| !c.is_whitespace()).unwrap_or(false) {
        iter.next();
        i += 1;
    }
    while iter.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
        iter.next();
        i += 1;
    }
    i
}

pub(crate) fn word_left(s: &str, cursor: usize) -> usize {
    let mut i = cursor;
    // Collect only the prefix up to cursor (not the full string) then reverse.
    let mut iter = s
        .chars()
        .take(cursor)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .peekable();
    while iter.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
        iter.next();
        i -= 1;
    }
    while iter.peek().map(|c| !c.is_whitespace()).unwrap_or(false) {
        iter.next();
        i -= 1;
    }
    i
}

pub(crate) fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub(crate) fn normalize_address(addr: &str) -> String {
    let digits: String = addr.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() >= 10 {
        digits[digits.len() - 10..].to_string()
    } else if digits.len() >= 7 {
        digits
    } else {
        addr.to_lowercase()
    }
}

pub(crate) fn open_file(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

pub(crate) fn send_notification(title: &str, body: &str) {
    let t = title.replace('"', "'");
    let b = body.replace('"', "'");
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("notify-send")
        .arg(&t)
        .arg(&b)
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            r#"display notification "{}" with title "{}""#,
            b, t
        ))
        .spawn();
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = (t, b);
}

pub(crate) fn friendly_login_error(msg: &str) -> String {
    if msg.contains("401") {
        "Password is incorrect".to_string()
    } else if msg.contains("refused")
        || msg.contains("connection")
        || msg.contains("timeout")
        || msg.contains("resolve")
        || msg.contains("dns")
    {
        "Could not connect to server. Check the host address.".to_string()
    } else {
        format!("Login failed: {}", msg)
    }
}

pub(crate) fn log_error(msg: &str) {
    use std::io::Write;
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let path = PathBuf::from(home)
        .join(".config")
        .join("bloop")
        .join("errors.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{}", msg);
    }
}
