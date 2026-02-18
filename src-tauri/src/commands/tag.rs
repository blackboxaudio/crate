use tauri::State;

use crate::error::Result;
use crate::models::{Tag, TagCategory};
use crate::services::TagService;

#[tauri::command]
pub async fn get_tag_categories(tags: State<'_, TagService>) -> Result<Vec<TagCategory>> {
    tags.get_categories()
}

#[tauri::command]
pub async fn create_tag_category(
    name: String,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<TagCategory> {
    tags.create_category(name, color)
}

#[tauri::command]
pub async fn update_tag_category(
    id: String,
    name: Option<String>,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<TagCategory> {
    tags.update_category(&id, name, color)
}

#[tauri::command]
pub async fn delete_tag_category(id: String, tags: State<'_, TagService>) -> Result<()> {
    tags.delete_category(&id)
}

#[tauri::command]
pub async fn create_tag(
    category_id: String,
    name: String,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<Tag> {
    tags.create_tag(category_id, name, color)
}

#[tauri::command]
pub async fn update_tag(
    id: String,
    name: Option<String>,
    color: Option<String>,
    tags: State<'_, TagService>,
) -> Result<Tag> {
    tags.update_tag(&id, name, color)
}

#[tauri::command]
pub async fn move_tag(
    tag_id: String,
    target_category_id: String,
    tags: State<'_, TagService>,
) -> Result<Tag> {
    tags.move_tag(&tag_id, &target_category_id)
}

#[tauri::command]
pub async fn delete_tag(id: String, tags: State<'_, TagService>) -> Result<()> {
    tags.delete_tag(&id)
}

#[tauri::command]
pub async fn assign_tags(
    track_ids: Vec<String>,
    tag_ids: Vec<String>,
    tags: State<'_, TagService>,
) -> Result<()> {
    tags.assign_tags(track_ids, tag_ids)
}

#[tauri::command]
pub async fn remove_tags(
    track_ids: Vec<String>,
    tag_ids: Vec<String>,
    tags: State<'_, TagService>,
) -> Result<()> {
    tags.remove_tags(track_ids, tag_ids)
}
