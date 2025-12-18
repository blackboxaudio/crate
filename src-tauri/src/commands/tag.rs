use tauri::State;

use crate::error::CrateError;
use crate::models::{Tag, TagCategory};
use crate::services::TagService;

#[tauri::command]
pub async fn get_tag_categories(
    tags: State<'_, TagService>,
) -> Result<Vec<TagCategory>, CrateError> {
    tags.get_categories()
}

#[tauri::command]
pub async fn create_tag_category(
    name: String,
    tags: State<'_, TagService>,
) -> Result<TagCategory, CrateError> {
    tags.create_category(name)
}

#[tauri::command]
pub async fn update_tag_category(
    id: String,
    name: String,
    tags: State<'_, TagService>,
) -> Result<TagCategory, CrateError> {
    tags.update_category(&id, name)
}

#[tauri::command]
pub async fn delete_tag_category(
    id: String,
    tags: State<'_, TagService>,
) -> Result<(), CrateError> {
    tags.delete_category(&id)
}

#[tauri::command]
pub async fn create_tag(
    category_id: String,
    name: String,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<Tag, CrateError> {
    tags.create_tag(category_id, name, color)
}

#[tauri::command]
pub async fn update_tag(
    id: String,
    name: Option<String>,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<Tag, CrateError> {
    tags.update_tag(&id, name, color)
}

#[tauri::command]
pub async fn delete_tag(
    id: String,
    tags: State<'_, TagService>,
) -> Result<(), CrateError> {
    tags.delete_tag(&id)
}

#[tauri::command]
pub async fn assign_tags(
    track_ids: Vec<String>,
    tag_ids: Vec<String>,
    tags: State<'_, TagService>,
) -> Result<(), CrateError> {
    tags.assign_tags(track_ids, tag_ids)
}

#[tauri::command]
pub async fn remove_tags(
    track_ids: Vec<String>,
    tag_ids: Vec<String>,
    tags: State<'_, TagService>,
) -> Result<(), CrateError> {
    tags.remove_tags(track_ids, tag_ids)
}
