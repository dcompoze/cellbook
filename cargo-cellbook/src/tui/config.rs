//! App and runtime configuration.

use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// App configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub keybindings: Keybindings,
}

/// General settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub auto_reload: bool,
    pub debounce_ms: u32,
    pub image_viewer: Option<String>,
    pub show_timings: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            auto_reload: true,
            debounce_ms: 500,
            image_viewer: None,
            show_timings: false,
        }
    }
}

/// Keybinding configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybindings {
    pub quit: KeyBinding,
    pub clear_context: KeyBinding,
    pub view_output: KeyBinding,
    pub view_error: KeyBinding,
    pub view_build_error: KeyBinding,
    pub reload: KeyBinding,
    pub edit: KeyBinding,
    pub run_cell: KeyBinding,
    pub navigate_down: KeyBinding,
    pub navigate_up: KeyBinding,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PartialAppConfig {
    general: Option<PartialGeneralConfig>,
    keybindings: Option<PartialKeybindings>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PartialGeneralConfig {
    auto_reload: Option<bool>,
    debounce_ms: Option<u32>,
    image_viewer: Option<String>,
    show_timings: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct PartialKeybindings {
    quit: Option<KeyBinding>,
    clear_context: Option<KeyBinding>,
    view_output: Option<KeyBinding>,
    view_error: Option<KeyBinding>,
    view_build_error: Option<KeyBinding>,
    reload: Option<KeyBinding>,
    edit: Option<KeyBinding>,
    run_cell: Option<KeyBinding>,
    navigate_down: Option<KeyBinding>,
    navigate_up: Option<KeyBinding>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            quit: KeyBinding::Single("q".into()),
            clear_context: KeyBinding::Single("x".into()),
            view_output: KeyBinding::Single("o".into()),
            view_error: KeyBinding::Single("e".into()),
            view_build_error: KeyBinding::Single("f".into()),
            reload: KeyBinding::Single("r".into()),
            edit: KeyBinding::Single("E".into()),
            run_cell: KeyBinding::Single("Enter".into()),
            navigate_down: KeyBinding::Multiple(vec!["Down".into(), "j".into()]),
            navigate_up: KeyBinding::Multiple(vec!["Up".into(), "k".into()]),
        }
    }
}

/// A keybinding that can be a single key or multiple alternatives.
///
/// Supports modifier prefixes: `Ctrl+`, `Alt+`, `Shift+`.
/// Uppercase single characters implicitly require Shift.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBinding {
    Single(String),
    Multiple(Vec<String>),
}

impl KeyBinding {
    /// Check if the given key code and modifiers match this binding.
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        match self {
            KeyBinding::Single(s) => key_matches(s, code, modifiers),
            KeyBinding::Multiple(keys) => keys.iter().any(|s| key_matches(s, code, modifiers)),
        }
    }
}

fn key_matches(binding: &str, code: KeyCode, modifiers: KeyModifiers) -> bool {
    parse_key(binding).is_some_and(|(k, m)| k == code && m == modifiers)
}

/// Parse a key string into a `KeyCode` and `KeyModifiers`.
///
/// Supports modifier prefixes (`Ctrl+`, `Alt+`, `Shift+`) and
/// implicit Shift for uppercase ASCII characters.
fn parse_key(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let (modifiers, key_part) = if let Some(rest) = s.strip_prefix("Ctrl+") {
        (KeyModifiers::CONTROL, rest)
    } else if let Some(rest) = s.strip_prefix("Alt+") {
        (KeyModifiers::ALT, rest)
    } else if let Some(rest) = s.strip_prefix("Shift+") {
        (KeyModifiers::SHIFT, rest)
    } else {
        (KeyModifiers::NONE, s)
    };

    let code = match key_part {
        "Enter" => KeyCode::Enter,
        "Esc" | "Escape" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Insert" => KeyCode::Insert,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Space" => KeyCode::Char(' '),
        "F1" => KeyCode::F(1),
        "F2" => KeyCode::F(2),
        "F3" => KeyCode::F(3),
        "F4" => KeyCode::F(4),
        "F5" => KeyCode::F(5),
        "F6" => KeyCode::F(6),
        "F7" => KeyCode::F(7),
        "F8" => KeyCode::F(8),
        "F9" => KeyCode::F(9),
        "F10" => KeyCode::F(10),
        "F11" => KeyCode::F(11),
        "F12" => KeyCode::F(12),
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            // Uppercase ASCII implies Shift when no explicit modifier is set.
            if c.is_ascii_uppercase() && modifiers == KeyModifiers::NONE {
                return Some((KeyCode::Char(c), KeyModifiers::SHIFT));
            }
            KeyCode::Char(c)
        }
        _ => return None,
    };

    Some((code, modifiers))
}

/// Get the path to the config file.
fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("cellbook").join("config.toml"))
}

/// Get the path to the local project config file.
fn local_config_path() -> Option<PathBuf> {
    std::env::current_dir().ok().map(|p| p.join("Cellbook.toml"))
}

fn merge(base: &mut AppConfig, patch: PartialAppConfig) {
    if let Some(general) = patch.general {
        if let Some(auto_reload) = general.auto_reload {
            base.general.auto_reload = auto_reload;
        }
        if let Some(debounce_ms) = general.debounce_ms {
            base.general.debounce_ms = debounce_ms;
        }
        if let Some(image_viewer) = general.image_viewer {
            base.general.image_viewer = Some(image_viewer);
        }
        if let Some(show_timings) = general.show_timings {
            base.general.show_timings = show_timings;
        }
    }

    if let Some(keybindings) = patch.keybindings {
        if let Some(v) = keybindings.quit {
            base.keybindings.quit = v;
        }
        if let Some(v) = keybindings.clear_context {
            base.keybindings.clear_context = v;
        }
        if let Some(v) = keybindings.view_output {
            base.keybindings.view_output = v;
        }
        if let Some(v) = keybindings.view_error {
            base.keybindings.view_error = v;
        }
        if let Some(v) = keybindings.view_build_error {
            base.keybindings.view_build_error = v;
        }
        if let Some(v) = keybindings.reload {
            base.keybindings.reload = v;
        }
        if let Some(v) = keybindings.edit {
            base.keybindings.edit = v;
        }
        if let Some(v) = keybindings.run_cell {
            base.keybindings.run_cell = v;
        }
        if let Some(v) = keybindings.navigate_down {
            base.keybindings.navigate_down = v;
        }
        if let Some(v) = keybindings.navigate_up {
            base.keybindings.navigate_up = v;
        }
    }
}

fn merge_file(config: &mut AppConfig, path: Option<PathBuf>) {
    let Some(path) = path else {
        return;
    };

    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };

    let Ok(partial) = toml::from_str::<PartialAppConfig>(&contents) else {
        return;
    };

    merge(config, partial);
}

/// Load app configuration from defaults, global, then local.
pub fn load() -> AppConfig {
    let mut config = AppConfig::default();
    merge_file(&mut config, global_config_path());
    merge_file(&mut config, local_config_path());
    config
}

/// Ensure the config file exists with default values.
/// Creates the config directory and file if they don't exist.
pub fn ensure_config_exists() {
    let Some(path) = global_config_path() else {
        return;
    };

    if path.exists() {
        return;
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let config = AppConfig::default();
    if let Ok(contents) = toml::to_string(&config) {
        let _ = std::fs::write(&path, contents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_single_char() {
        assert_eq!(parse_key("q"), Some((KeyCode::Char('q'), KeyModifiers::NONE)));
        assert_eq!(parse_key("j"), Some((KeyCode::Char('j'), KeyModifiers::NONE)));
        assert_eq!(parse_key("1"), Some((KeyCode::Char('1'), KeyModifiers::NONE)));
    }

    #[test]
    fn test_parse_key_uppercase_implies_shift() {
        assert_eq!(parse_key("E"), Some((KeyCode::Char('E'), KeyModifiers::SHIFT)));
        assert_eq!(parse_key("Q"), Some((KeyCode::Char('Q'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn test_parse_key_explicit_modifiers() {
        assert_eq!(
            parse_key("Ctrl+q"),
            Some((KeyCode::Char('q'), KeyModifiers::CONTROL))
        );
        assert_eq!(parse_key("Alt+x"), Some((KeyCode::Char('x'), KeyModifiers::ALT)));
        assert_eq!(
            parse_key("Shift+Enter"),
            Some((KeyCode::Enter, KeyModifiers::SHIFT))
        );
    }

    #[test]
    fn test_parse_key_special() {
        assert_eq!(parse_key("Enter"), Some((KeyCode::Enter, KeyModifiers::NONE)));
        assert_eq!(parse_key("Esc"), Some((KeyCode::Esc, KeyModifiers::NONE)));
        assert_eq!(parse_key("Space"), Some((KeyCode::Char(' '), KeyModifiers::NONE)));
        assert_eq!(parse_key("Up"), Some((KeyCode::Up, KeyModifiers::NONE)));
        assert_eq!(parse_key("Down"), Some((KeyCode::Down, KeyModifiers::NONE)));
    }

    #[test]
    fn test_parse_key_function() {
        assert_eq!(parse_key("F1"), Some((KeyCode::F(1), KeyModifiers::NONE)));
        assert_eq!(parse_key("F12"), Some((KeyCode::F(12), KeyModifiers::NONE)));
    }

    #[test]
    fn test_parse_key_invalid() {
        assert_eq!(parse_key("invalid"), None);
        assert_eq!(parse_key(""), None);
    }

    #[test]
    fn test_keybinding_matches_single() {
        let binding = KeyBinding::Single("q".into());
        assert!(binding.matches(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(!binding.matches(KeyCode::Char('x'), KeyModifiers::NONE));
    }

    #[test]
    fn test_keybinding_matches_shift() {
        let binding = KeyBinding::Single("E".into());
        assert!(binding.matches(KeyCode::Char('E'), KeyModifiers::SHIFT));
        assert!(!binding.matches(KeyCode::Char('E'), KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::Char('e'), KeyModifiers::NONE));
    }

    #[test]
    fn test_keybinding_matches_multiple() {
        let binding = KeyBinding::Multiple(vec!["Down".into(), "j".into()]);
        assert!(binding.matches(KeyCode::Down, KeyModifiers::NONE));
        assert!(binding.matches(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::Up, KeyModifiers::NONE));
    }

    #[test]
    fn test_config_deserialize() {
        let toml = r#"
[general]
show_timings = true

[keybindings]
quit = "q"
navigate_down = ["Down", "n"]
"#;
        let config: AppConfig = toml::from_str(toml).unwrap();
        assert!(config.general.show_timings);
        assert!(
            config
                .keybindings
                .quit
                .matches(KeyCode::Char('q'), KeyModifiers::NONE)
        );
        assert!(
            config
                .keybindings
                .view_build_error
                .matches(KeyCode::Char('f'), KeyModifiers::NONE)
        );
        assert!(
            config
                .keybindings
                .navigate_down
                .matches(KeyCode::Down, KeyModifiers::NONE)
        );
        assert!(
            config
                .keybindings
                .navigate_down
                .matches(KeyCode::Char('n'), KeyModifiers::NONE)
        );
    }

    #[test]
    fn test_default_config_serializes() {
        let config = AppConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("[general]"));
        assert!(serialized.contains("auto_reload = true"));
        assert!(serialized.contains("debounce_ms = 500"));
        assert!(serialized.contains("show_timings = false"));
        assert!(serialized.contains("[keybindings]"));
        assert!(serialized.contains("quit"));
        assert!(serialized.contains("view_build_error = \"f\""));
        // Verify arrays are on single lines.
        assert!(serialized.contains(r#"navigate_down = ["Down", "j"]"#));
        assert!(serialized.contains(r#"navigate_up = ["Up", "k"]"#));
    }

    #[test]
    fn test_merge_partial_general_fields() {
        let mut config = AppConfig::default();
        merge(
            &mut config,
            PartialAppConfig {
                general: Some(PartialGeneralConfig {
                    show_timings: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );

        assert!(config.general.show_timings);
        assert!(config.general.auto_reload);
        assert_eq!(config.general.debounce_ms, 500);
    }

    #[test]
    fn test_merge_local_overrides_global() {
        let mut config = AppConfig::default();
        merge(
            &mut config,
            toml::from_str::<PartialAppConfig>(
                r#"
[general]
debounce_ms = 900
show_timings = false
"#,
            )
            .unwrap(),
        );

        merge(
            &mut config,
            toml::from_str::<PartialAppConfig>(
                r#"
[general]
show_timings = true
"#,
            )
            .unwrap(),
        );

        assert_eq!(config.general.debounce_ms, 900);
        assert!(config.general.show_timings);
    }

    #[test]
    fn test_merge_keybindings_is_field_level() {
        let mut config = AppConfig::default();
        merge(
            &mut config,
            toml::from_str::<PartialAppConfig>(
                r#"
[keybindings]
quit = "Q"
"#,
            )
            .unwrap(),
        );

        assert!(
            config
                .keybindings
                .quit
                .matches(KeyCode::Char('Q'), KeyModifiers::SHIFT)
        );
        assert!(
            config
                .keybindings
                .reload
                .matches(KeyCode::Char('r'), KeyModifiers::NONE)
        );
    }
}
