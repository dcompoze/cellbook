//! TUI configuration for keybindings.

use std::path::PathBuf;

use ratatui::crossterm::event::KeyCode;
use serde::{Deserialize, Serialize};

/// TUI configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    pub keybindings: Keybindings,
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybindings {
    pub quit: KeyBinding,
    pub clear_context: KeyBinding,
    pub view_output: KeyBinding,
    pub view_error: KeyBinding,
    pub reload: KeyBinding,
    pub edit: KeyBinding,
    pub run_cell: KeyBinding,
    pub navigate_down: KeyBinding,
    pub navigate_up: KeyBinding,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: KeyBinding::Single("q".into()),
            clear_context: KeyBinding::Single("x".into()),
            view_output: KeyBinding::Single("o".into()),
            view_error: KeyBinding::Single("e".into()),
            reload: KeyBinding::Single("r".into()),
            edit: KeyBinding::Single("E".into()),
            run_cell: KeyBinding::Single("Enter".into()),
            navigate_down: KeyBinding::Multiple(vec!["Down".into(), "j".into()]),
            navigate_up: KeyBinding::Multiple(vec!["Up".into(), "k".into()]),
        }
    }
}

/// A keybinding that can be a single key or multiple alternatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBinding {
    Single(String),
    Multiple(Vec<String>),
}

impl KeyBinding {
    /// Check if the given key code matches this binding.
    pub fn matches(&self, code: KeyCode) -> bool {
        match self {
            KeyBinding::Single(s) => parse_key(s).is_some_and(|k| k == code),
            KeyBinding::Multiple(keys) => keys.iter().any(|s| parse_key(s).is_some_and(|k| k == code)),
        }
    }
}

/// Parse a key string into a KeyCode.
fn parse_key(s: &str) -> Option<KeyCode> {
    match s {
        "Enter" => Some(KeyCode::Enter),
        "Esc" | "Escape" => Some(KeyCode::Esc),
        "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace),
        "Delete" => Some(KeyCode::Delete),
        "Insert" => Some(KeyCode::Insert),
        "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End),
        "PageUp" => Some(KeyCode::PageUp),
        "PageDown" => Some(KeyCode::PageDown),
        "Up" => Some(KeyCode::Up),
        "Down" => Some(KeyCode::Down),
        "Left" => Some(KeyCode::Left),
        "Right" => Some(KeyCode::Right),
        "Space" => Some(KeyCode::Char(' ')),
        "F1" => Some(KeyCode::F(1)),
        "F2" => Some(KeyCode::F(2)),
        "F3" => Some(KeyCode::F(3)),
        "F4" => Some(KeyCode::F(4)),
        "F5" => Some(KeyCode::F(5)),
        "F6" => Some(KeyCode::F(6)),
        "F7" => Some(KeyCode::F(7)),
        "F8" => Some(KeyCode::F(8)),
        "F9" => Some(KeyCode::F(9)),
        "F10" => Some(KeyCode::F(10)),
        "F11" => Some(KeyCode::F(11)),
        "F12" => Some(KeyCode::F(12)),
        s if s.len() == 1 => s.chars().next().map(KeyCode::Char),
        _ => None,
    }
}

/// Get the path to the config file.
fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("cellbook").join("config.toml"))
}

/// Load the TUI configuration.
/// Returns defaults if the config file doesn't exist or can't be parsed.
pub fn load() -> TuiConfig {
    let Some(path) = config_path() else {
        return TuiConfig::default();
    };

    let Ok(contents) = std::fs::read_to_string(&path) else {
        return TuiConfig::default();
    };

    toml::from_str(&contents).unwrap_or_default()
}

/// Ensure the config file exists with default values.
/// Creates the config directory and file if they don't exist.
pub fn ensure_config_exists() {
    let Some(path) = config_path() else {
        return;
    };

    if path.exists() {
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let config = TuiConfig::default();
    if let Ok(contents) = toml::to_string(&config) {
        let _ = std::fs::write(&path, contents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_single_char() {
        assert_eq!(parse_key("q"), Some(KeyCode::Char('q')));
        assert_eq!(parse_key("j"), Some(KeyCode::Char('j')));
        assert_eq!(parse_key("1"), Some(KeyCode::Char('1')));
    }

    #[test]
    fn test_parse_key_special() {
        assert_eq!(parse_key("Enter"), Some(KeyCode::Enter));
        assert_eq!(parse_key("Esc"), Some(KeyCode::Esc));
        assert_eq!(parse_key("Space"), Some(KeyCode::Char(' ')));
        assert_eq!(parse_key("Up"), Some(KeyCode::Up));
        assert_eq!(parse_key("Down"), Some(KeyCode::Down));
    }

    #[test]
    fn test_parse_key_function() {
        assert_eq!(parse_key("F1"), Some(KeyCode::F(1)));
        assert_eq!(parse_key("F12"), Some(KeyCode::F(12)));
    }

    #[test]
    fn test_parse_key_invalid() {
        assert_eq!(parse_key("invalid"), None);
        assert_eq!(parse_key(""), None);
    }

    #[test]
    fn test_keybinding_matches_single() {
        let binding = KeyBinding::Single("q".into());
        assert!(binding.matches(KeyCode::Char('q')));
        assert!(!binding.matches(KeyCode::Char('x')));
    }

    #[test]
    fn test_keybinding_matches_multiple() {
        let binding = KeyBinding::Multiple(vec!["Down".into(), "j".into()]);
        assert!(binding.matches(KeyCode::Down));
        assert!(binding.matches(KeyCode::Char('j')));
        assert!(!binding.matches(KeyCode::Up));
    }

    #[test]
    fn test_config_deserialize() {
        let toml = r#"
[keybindings]
quit = "q"
navigate_down = ["Down", "n"]
"#;
        let config: TuiConfig = toml::from_str(toml).unwrap();
        assert!(config.keybindings.quit.matches(KeyCode::Char('q')));
        assert!(config.keybindings.navigate_down.matches(KeyCode::Down));
        assert!(config.keybindings.navigate_down.matches(KeyCode::Char('n')));
    }

    #[test]
    fn test_default_config_serializes() {
        let config = TuiConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("[keybindings]"));
        assert!(serialized.contains("quit"));
        // Verify arrays are on single lines.
        assert!(serialized.contains(r#"navigate_down = ["Down", "j"]"#));
        assert!(serialized.contains(r#"navigate_up = ["Up", "k"]"#));
    }
}
