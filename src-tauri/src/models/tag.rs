use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCategory {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

#[allow(dead_code)]
impl TagCategory {
    pub fn new(name: String, color: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            color,
            sort_order: 0,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub category_id: String,
    pub name: String,
    pub color: Option<String>,
    pub sort_order: i32,
}

#[allow(dead_code)]
impl Tag {
    pub fn new(category_id: String, name: String, color: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            category_id,
            name,
            color,
            sort_order: 0,
        }
    }
}
