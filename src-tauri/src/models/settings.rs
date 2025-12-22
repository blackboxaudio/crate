use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Ja,
    De,
    Es,
    Fr,
    It,
    Ko,
    Nl,
    Pt,
    Sv,
    Zh,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::En => write!(f, "en"),
            Language::Ja => write!(f, "ja"),
            Language::De => write!(f, "de"),
            Language::Es => write!(f, "es"),
            Language::Fr => write!(f, "fr"),
            Language::It => write!(f, "it"),
            Language::Ko => write!(f, "ko"),
            Language::Nl => write!(f, "nl"),
            Language::Pt => write!(f, "pt"),
            Language::Sv => write!(f, "sv"),
            Language::Zh => write!(f, "zh"),
        }
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "en" => Ok(Language::En),
            "ja" => Ok(Language::Ja),
            "de" => Ok(Language::De),
            "es" => Ok(Language::Es),
            "fr" => Ok(Language::Fr),
            "it" => Ok(Language::It),
            "ko" => Ok(Language::Ko),
            "nl" => Ok(Language::Nl),
            "pt" => Ok(Language::Pt),
            "sv" => Ok(Language::Sv),
            "zh" => Ok(Language::Zh),
            _ => Err(format!("Unknown language: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Light => write!(f, "light"),
            Theme::Dark => write!(f, "dark"),
            Theme::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "light" => Ok(Theme::Light),
            "dark" => Ok(Theme::Dark),
            "system" => Ok(Theme::System),
            _ => Err(format!("Unknown theme: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccentColor {
    #[default]
    Blue,
    Indigo,
    Violet,
    Purple,
    Pink,
    Rose,
    Orange,
    Amber,
    Emerald,
    Teal,
}

impl std::fmt::Display for AccentColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccentColor::Blue => write!(f, "blue"),
            AccentColor::Indigo => write!(f, "indigo"),
            AccentColor::Violet => write!(f, "violet"),
            AccentColor::Purple => write!(f, "purple"),
            AccentColor::Pink => write!(f, "pink"),
            AccentColor::Rose => write!(f, "rose"),
            AccentColor::Orange => write!(f, "orange"),
            AccentColor::Amber => write!(f, "amber"),
            AccentColor::Emerald => write!(f, "emerald"),
            AccentColor::Teal => write!(f, "teal"),
        }
    }
}

impl std::str::FromStr for AccentColor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "blue" => Ok(AccentColor::Blue),
            "indigo" => Ok(AccentColor::Indigo),
            "violet" => Ok(AccentColor::Violet),
            "purple" => Ok(AccentColor::Purple),
            "pink" => Ok(AccentColor::Pink),
            "rose" => Ok(AccentColor::Rose),
            "orange" => Ok(AccentColor::Orange),
            "amber" => Ok(AccentColor::Amber),
            "emerald" => Ok(AccentColor::Emerald),
            "teal" => Ok(AccentColor::Teal),
            _ => Err(format!("Unknown accent color: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Font {
    #[default]
    IbmPlexMono,
    JetBrainsMono,
    FiraCode,
    Inter,
    OpenSans,
}

impl std::fmt::Display for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Font::IbmPlexMono => write!(f, "ibm-plex-mono"),
            Font::JetBrainsMono => write!(f, "jetbrains-mono"),
            Font::FiraCode => write!(f, "fira-code"),
            Font::Inter => write!(f, "inter"),
            Font::OpenSans => write!(f, "open-sans"),
        }
    }
}

impl std::str::FromStr for Font {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ibm-plex-mono" => Ok(Font::IbmPlexMono),
            "jetbrains-mono" => Ok(Font::JetBrainsMono),
            "fira-code" => Ok(Font::FiraCode),
            "inter" => Ok(Font::Inter),
            "open-sans" => Ok(Font::OpenSans),
            _ => Err(format!("Unknown font: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeyNotationFormat {
    Standard,
    #[default]
    Camelot,
}

impl std::fmt::Display for KeyNotationFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyNotationFormat::Standard => write!(f, "standard"),
            KeyNotationFormat::Camelot => write!(f, "camelot"),
        }
    }
}

impl std::str::FromStr for KeyNotationFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(KeyNotationFormat::Standard),
            "camelot" => Ok(KeyNotationFormat::Camelot),
            _ => Err(format!("Unknown key notation format: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub accent_color: AccentColor,
    pub font: Font,
    pub audio_device: Option<String>,
    pub language: Language,
    pub key_notation_format: KeyNotationFormat,
    pub auto_analyze_on_import: bool,
    pub auto_sync_on_connect: bool,
    pub auto_sync_on_change: bool,
    pub ignored_device_ids: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            accent_color: AccentColor::default(),
            font: Font::default(),
            audio_device: None,
            language: Language::default(),
            key_notation_format: KeyNotationFormat::default(),
            auto_analyze_on_import: true,
            auto_sync_on_connect: false,
            auto_sync_on_change: false,
            ignored_device_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
    pub is_default: bool,
    pub is_built_in: bool,
}
