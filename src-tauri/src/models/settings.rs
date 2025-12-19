use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub accent_color: AccentColor,
}
