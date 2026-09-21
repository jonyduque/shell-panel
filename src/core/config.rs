use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_selected_bg() -> String {
    "cyan".to_string()
}

fn default_selected_fg() -> String {
    "black".to_string()
}

fn default_description_fg() -> String {
    "gray".to_string()
}

fn default_selected_prefix() -> String {
    "> ".to_string()
}

fn default_unselected_prefix() -> String {
    "  ".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColorConfig {
    #[serde(default = "default_selected_bg")]
    pub selected_bg: String,
    #[serde(default = "default_selected_fg")]
    pub selected_fg: String,
    #[serde(default)]
    pub unselected_fg: String,
    #[serde(default = "default_description_fg")]
    pub description_fg: String,
    #[serde(default = "default_selected_prefix")]
    pub selected_prefix: String,
    #[serde(default = "default_unselected_prefix")]
    pub unselected_prefix: String,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            selected_bg: default_selected_bg(),
            selected_fg: default_selected_fg(),
            unselected_fg: String::new(),
            description_fg: default_description_fg(),
            selected_prefix: default_selected_prefix(),
            unselected_prefix: default_unselected_prefix(),
        }
    }
}

fn default_icon_directory() -> String {
    "📁 ".to_string()
}

fn default_icon_file() -> String {
    "📄 ".to_string()
}

fn default_icon_command() -> String {
    "⚡ ".to_string()
}

fn default_icon_subcommand() -> String {
    "🔹 ".to_string()
}

fn default_icon_option() -> String {
    "🏷️  ".to_string()
}

fn default_icon_powershell_cmdlet() -> String {
    ">_ ".to_string()
}

fn default_icon_alias() -> String {
    "🔗 ".to_string()
}

fn default_icon_other() -> String {
    "  ".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IconConfig {
    #[serde(default = "default_icon_directory")]
    pub directory: String,
    #[serde(default = "default_icon_file")]
    pub file: String,
    #[serde(default = "default_icon_command")]
    pub command: String,
    #[serde(default = "default_icon_subcommand")]
    pub subcommand: String,
    #[serde(default = "default_icon_option")]
    pub option: String,
    #[serde(default = "default_icon_powershell_cmdlet")]
    pub powershell_cmdlet: String,
    #[serde(default = "default_icon_alias")]
    pub alias: String,
    #[serde(default = "default_icon_other")]
    pub other: String,
}

impl Default for IconConfig {
    fn default() -> Self {
        Self {
            directory: default_icon_directory(),
            file: default_icon_file(),
            command: default_icon_command(),
            subcommand: default_icon_subcommand(),
            option: default_icon_option(),
            powershell_cmdlet: default_icon_powershell_cmdlet(),
            alias: default_icon_alias(),
            other: default_icon_other(),
        }
    }
}

fn default_max_suggestions() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "default_max_suggestions")]
    pub max_suggestions: usize,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub colors: ColorConfig,
    #[serde(default)]
    pub icons: IconConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_suggestions: default_max_suggestions(),
            shell: None,
            colors: ColorConfig::default(),
            icons: IconConfig::default(),
        }
    }
}

impl Config {
    pub fn new(max_suggestions: usize) -> Self {
        Self {
            max_suggestions,
            ..Default::default()
        }
    }

    /// Loads the configuration and returns it with messages for the user. A file that cannot be
    /// read or parsed yields the defaults; unknown keys are reported and ignored; a missing file
    /// at the default location is not a problem.
    pub fn load(custom_path: Option<&Path>) -> (Self, Vec<String>) {
        let (path, explicit) = match custom_path {
            Some(p) => (p.to_path_buf(), true),
            None => match default_config_path() {
                Some(p) => (p, false),
                None => return (Self::default(), Vec::new()),
            },
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if !explicit && err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new())
            }
            Err(err) => {
                return (
                    Self::default(),
                    vec![format!("could not read config {}: {}", path.display(), err)],
                )
            }
        };

        match toml::from_str::<Self>(&content) {
            Ok(config) => {
                let warnings = content
                    .parse::<toml::Table>()
                    .map(|table| unknown_keys(&table))
                    .unwrap_or_default()
                    .into_iter()
                    .map(|key| format!("unknown key `{}` in {} (ignored)", key, path.display()))
                    .collect();
                (config, warnings)
            }
            Err(err) => (
                Self::default(),
                vec![format!(
                    "invalid config {}: {}; using defaults",
                    path.display(),
                    err
                )],
            ),
        }
    }

    /// Like [`Config::load`], discarding the warnings.
    pub fn load_or_default(custom_path: Option<&Path>) -> Self {
        Self::load(custom_path).0
    }
}

const TOP_LEVEL_KEYS: &[&str] = &["max_suggestions", "shell", "colors", "icons"];
const COLOR_KEYS: &[&str] = &[
    "selected_bg",
    "selected_fg",
    "unselected_fg",
    "description_fg",
    "selected_prefix",
    "unselected_prefix",
];
const ICON_KEYS: &[&str] = &[
    "directory",
    "file",
    "command",
    "subcommand",
    "option",
    "powershell_cmdlet",
    "alias",
    "other",
];

/// Keys serde would silently ignore, as dotted paths.
fn unknown_keys(table: &toml::Table) -> Vec<String> {
    let mut unknown = Vec::new();
    for (key, value) in table {
        let section_keys = match key.as_str() {
            "colors" => COLOR_KEYS,
            "icons" => ICON_KEYS,
            k if TOP_LEVEL_KEYS.contains(&k) => continue,
            _ => {
                unknown.push(key.clone());
                continue;
            }
        };
        if let toml::Value::Table(section) = value {
            unknown.extend(
                section
                    .keys()
                    .filter(|k| !section_keys.contains(&k.as_str()))
                    .map(|k| format!("{key}.{k}")),
            );
        }
    }
    unknown
}

pub fn default_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join(".config").join("shell-panel.toml"))
}

pub fn default_sample_toml() -> &'static str {
    r##"# Shell-Panel Configuration

# Maximum number of suggestions to display in the dropdown (default: 5)
max_suggestions = 5

# Shell to launch: "pwsh" or "powershell" (default: pwsh.exe when on PATH, else powershell.exe)
# shell = "pwsh"

[colors]
# Selected item background color (e.g. "cyan", "blue", "reverse", "#3b82f6", "244")
selected_bg = "cyan"

# Selected item foreground color
selected_fg = "black"

# Unselected item foreground color (empty for terminal default)
unselected_fg = ""

# Description text foreground color
description_fg = "gray"

# Indicator prefix for the selected row
selected_prefix = "> "

# Indicator prefix for unselected rows
unselected_prefix = "  "

[icons]
# Icons displayed for each suggestion category
directory = "📁 "
file = "📄 "
command = "⚡ "
subcommand = "🔹 "
option = "🏷️  "
powershell_cmdlet = ">_ "
alias = "🔗 "
other = "  "
"##
}
