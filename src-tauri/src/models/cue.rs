use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CueType {
    Memory,
    Hot,
    Loop,
}

impl std::fmt::Display for CueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CueType::Memory => write!(f, "memory"),
            CueType::Hot => write!(f, "hot"),
            CueType::Loop => write!(f, "loop"),
        }
    }
}

impl std::str::FromStr for CueType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "memory" => Ok(CueType::Memory),
            "hot" => Ok(CueType::Hot),
            "loop" => Ok(CueType::Loop),
            _ => Err(format!("Invalid cue type: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cue {
    pub id: String,
    pub track_id: String,
    pub position_ms: i64,
    pub cue_type: CueType,
    pub loop_end_ms: Option<i64>,
    pub hot_cue_index: Option<i32>,
    pub name: Option<String>,
    pub color: Option<String>,
}

#[allow(dead_code)]
impl Cue {
    pub fn new_memory(track_id: String, position_ms: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            track_id,
            position_ms,
            cue_type: CueType::Memory,
            loop_end_ms: None,
            hot_cue_index: None,
            name: None,
            color: None,
        }
    }

    pub fn new_hot(track_id: String, position_ms: i64, index: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            track_id,
            position_ms,
            cue_type: CueType::Hot,
            loop_end_ms: None,
            hot_cue_index: Some(index),
            name: None,
            color: None,
        }
    }

    pub fn new_loop(track_id: String, position_ms: i64, end_ms: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            track_id,
            position_ms,
            cue_type: CueType::Loop,
            loop_end_ms: Some(end_ms),
            hot_cue_index: None,
            name: None,
            color: None,
        }
    }
}
