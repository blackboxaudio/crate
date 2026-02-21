use rusqlite::types::Value;

use crate::error::{CrateError, Result};
use crate::models::{
    DateOperator, EnumOperator, MatchMode, NumericOperator, SmartCondition, SmartLimit, SmartRules,
    SortDirection, TagOperator, TextOperator,
};

/// Map a field name to its SQL column for the library (tracks) context.
fn library_field_column(field: &str) -> Result<&'static str> {
    match field {
        "title" => Ok("t.title"),
        "artist" => Ok("t.artist"),
        "album" => Ok("t.album"),
        "genre" => Ok("t.genre"),
        "label" => Ok("t.label"),
        "catalog_number" => Ok("t.catalog_number"),
        "bpm" => Ok("t.bpm"),
        "rating" => Ok("t.rating"),
        "play_count" => Ok("t.play_count"),
        "year" => Ok("t.year"),
        "duration_ms" => Ok("t.duration_ms"),
        "bitrate" => Ok("t.bitrate"),
        "sample_rate" => Ok("t.sample_rate"),
        "date_added" => Ok("t.date_added"),
        "last_played" => Ok("t.last_played"),
        "date_modified" => Ok("t.date_modified"),
        "color" => Ok("t.color"),
        "format" => Ok("t.format"),
        "key" => Ok("t.key"),
        _ => Err(CrateError::InvalidOperation(format!(
            "Invalid library field: {field}"
        ))),
    }
}

/// Map a field name to its SQL column for the discovery (releases) context.
fn discovery_field_column(field: &str) -> Result<&'static str> {
    match field {
        "title" => Ok("dr.title"),
        "artist" => Ok("dr.artist"),
        "label" => Ok("dr.label"),
        "source_type" => Ok("dr.source_type"),
        "release_date" => Ok("dr.release_date"),
        "notes" => Ok("dr.notes"),
        "date_added" => Ok("dr.date_added"),
        "date_modified" => Ok("dr.date_modified"),
        _ => Err(CrateError::InvalidOperation(format!(
            "Invalid discovery field: {field}"
        ))),
    }
}

/// Map a sort field to its SQL column for the library context.
fn library_sort_column(field: &str) -> Result<&'static str> {
    match field {
        "date_added" => Ok("t.date_added"),
        "rating" => Ok("t.rating"),
        "play_count" => Ok("t.play_count"),
        "bpm" => Ok("t.bpm"),
        "title" => Ok("t.title"),
        "artist" => Ok("t.artist"),
        "random" => Ok("RANDOM()"),
        _ => Err(CrateError::InvalidOperation(format!(
            "Invalid library sort field: {field}"
        ))),
    }
}

/// Map a sort field to its SQL column for the discovery context.
fn discovery_sort_column(field: &str) -> Result<&'static str> {
    match field {
        "date_added" => Ok("dr.date_added"),
        "title" => Ok("dr.title"),
        "artist" => Ok("dr.artist"),
        "release_date" => Ok("dr.release_date"),
        "random" => Ok("RANDOM()"),
        _ => Err(CrateError::InvalidOperation(format!(
            "Invalid discovery sort field: {field}"
        ))),
    }
}

/// Build a text condition SQL fragment.
fn build_text_condition(
    column: &str,
    operator: &TextOperator,
    value: &Option<String>,
    params: &mut Vec<Value>,
) -> String {
    match operator {
        TextOperator::Contains => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(format!("%{val}%")));
            format!("COALESCE({column}, '') LIKE ?{}", params.len())
        }
        TextOperator::NotContains => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(format!("%{val}%")));
            format!("COALESCE({column}, '') NOT LIKE ?{}", params.len())
        }
        TextOperator::Equals => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(val.to_string()));
            format!("COALESCE({column}, '') = ?{}", params.len())
        }
        TextOperator::NotEquals => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(val.to_string()));
            format!("COALESCE({column}, '') != ?{}", params.len())
        }
        TextOperator::StartsWith => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(format!("{val}%")));
            format!("COALESCE({column}, '') LIKE ?{}", params.len())
        }
        TextOperator::EndsWith => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(format!("%{val}")));
            format!("COALESCE({column}, '') LIKE ?{}", params.len())
        }
        TextOperator::IsEmpty => {
            format!("({column} IS NULL OR {column} = '')")
        }
        TextOperator::IsNotEmpty => {
            format!("({column} IS NOT NULL AND {column} != '')")
        }
    }
}

/// Build a numeric condition SQL fragment.
fn build_numeric_condition(
    column: &str,
    operator: &NumericOperator,
    value: &Option<f64>,
    value2: &Option<f64>,
    params: &mut Vec<Value>,
) -> String {
    match operator {
        NumericOperator::Equals => {
            let val = value.unwrap_or(0.0);
            params.push(Value::Real(val));
            format!("{column} = ?{}", params.len())
        }
        NumericOperator::NotEquals => {
            let val = value.unwrap_or(0.0);
            params.push(Value::Real(val));
            format!("{column} != ?{}", params.len())
        }
        NumericOperator::GreaterThan => {
            let val = value.unwrap_or(0.0);
            params.push(Value::Real(val));
            format!("{column} > ?{}", params.len())
        }
        NumericOperator::LessThan => {
            let val = value.unwrap_or(0.0);
            params.push(Value::Real(val));
            format!("{column} < ?{}", params.len())
        }
        NumericOperator::InRange => {
            let v1 = value.unwrap_or(0.0);
            let v2 = value2.unwrap_or(0.0);
            params.push(Value::Real(v1));
            params.push(Value::Real(v2));
            format!(
                "{column} BETWEEN ?{} AND ?{}",
                params.len() - 1,
                params.len()
            )
        }
    }
}

/// Build a date condition SQL fragment.
fn build_date_condition(
    column: &str,
    operator: &DateOperator,
    value: &Option<String>,
    params: &mut Vec<Value>,
) -> String {
    match operator {
        DateOperator::InLastDays => {
            let val = value.as_deref().unwrap_or("30");
            params.push(Value::Text(format!("-{val} days")));
            format!("{column} > datetime('now', ?{})", params.len())
        }
        DateOperator::NotInLastDays => {
            let val = value.as_deref().unwrap_or("30");
            params.push(Value::Text(format!("-{val} days")));
            format!(
                "({column} IS NULL OR {column} <= datetime('now', ?{}))",
                params.len()
            )
        }
        DateOperator::Before => {
            let val = value.as_deref().unwrap_or("2000-01-01");
            params.push(Value::Text(val.to_string()));
            format!("{column} < ?{}", params.len())
        }
        DateOperator::After => {
            let val = value.as_deref().unwrap_or("2000-01-01");
            params.push(Value::Text(val.to_string()));
            format!("{column} > ?{}", params.len())
        }
        DateOperator::IsEmpty => {
            format!("{column} IS NULL")
        }
        DateOperator::IsNotEmpty => {
            format!("{column} IS NOT NULL")
        }
    }
}

/// Build an enum condition SQL fragment.
fn build_enum_condition(
    column: &str,
    operator: &EnumOperator,
    value: &Option<String>,
    params: &mut Vec<Value>,
) -> String {
    match operator {
        EnumOperator::Equals => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(val.to_string()));
            format!("{column} = ?{}", params.len())
        }
        EnumOperator::NotEquals => {
            let val = value.as_deref().unwrap_or("");
            params.push(Value::Text(val.to_string()));
            format!("({column} IS NULL OR {column} != ?{})", params.len())
        }
        EnumOperator::IsEmpty => {
            format!("({column} IS NULL OR {column} = '')")
        }
        EnumOperator::IsNotEmpty => {
            format!("({column} IS NOT NULL AND {column} != '')")
        }
    }
}

/// Build a tag condition SQL fragment for the library context.
fn build_library_tag_condition(
    operator: &TagOperator,
    tag_ids: &[String],
    params: &mut Vec<Value>,
) -> String {
    if tag_ids.is_empty() {
        return "1=1".to_string();
    }

    let placeholders: Vec<String> = tag_ids
        .iter()
        .map(|id| {
            params.push(Value::Text(id.clone()));
            format!("?{}", params.len())
        })
        .collect();
    let ph = placeholders.join(", ");

    match operator {
        TagOperator::HasAny => {
            format!("t.id IN (SELECT track_id FROM track_tags WHERE tag_id IN ({ph}))")
        }
        TagOperator::HasAll => {
            format!(
                "t.id IN (SELECT track_id FROM track_tags WHERE tag_id IN ({ph}) GROUP BY track_id HAVING COUNT(DISTINCT tag_id) = {})",
                tag_ids.len()
            )
        }
        TagOperator::HasNone => {
            format!("t.id NOT IN (SELECT track_id FROM track_tags WHERE tag_id IN ({ph}))")
        }
    }
}

/// Build a tag condition SQL fragment for the discovery context.
fn build_discovery_tag_condition(
    operator: &TagOperator,
    tag_ids: &[String],
    params: &mut Vec<Value>,
) -> String {
    if tag_ids.is_empty() {
        return "1=1".to_string();
    }

    let placeholders: Vec<String> = tag_ids
        .iter()
        .map(|id| {
            params.push(Value::Text(id.clone()));
            format!("?{}", params.len())
        })
        .collect();
    let ph = placeholders.join(", ");

    match operator {
        TagOperator::HasAny => {
            format!(
                "dr.id IN (SELECT release_id FROM discovery_release_tags WHERE tag_id IN ({ph}))"
            )
        }
        TagOperator::HasAll => {
            format!(
                "dr.id IN (SELECT release_id FROM discovery_release_tags WHERE tag_id IN ({ph}) GROUP BY release_id HAVING COUNT(DISTINCT tag_id) = {})",
                tag_ids.len()
            )
        }
        TagOperator::HasNone => {
            format!(
                "dr.id NOT IN (SELECT release_id FROM discovery_release_tags WHERE tag_id IN ({ph}))"
            )
        }
    }
}

/// Build a single condition SQL fragment.
fn build_condition_sql(
    condition: &SmartCondition,
    context: &str,
    params: &mut Vec<Value>,
) -> Result<String> {
    match condition {
        SmartCondition::Text {
            field,
            operator,
            value,
        } => {
            let column = if context == "discovery" {
                discovery_field_column(field)?
            } else {
                library_field_column(field)?
            };
            Ok(build_text_condition(column, operator, value, params))
        }
        SmartCondition::Numeric {
            field,
            operator,
            value,
            value2,
        } => {
            let column = if context == "discovery" {
                discovery_field_column(field)?
            } else {
                library_field_column(field)?
            };
            Ok(build_numeric_condition(
                column, operator, value, value2, params,
            ))
        }
        SmartCondition::Date {
            field,
            operator,
            value,
        } => {
            let column = if context == "discovery" {
                discovery_field_column(field)?
            } else {
                library_field_column(field)?
            };
            Ok(build_date_condition(column, operator, value, params))
        }
        SmartCondition::Enum {
            field,
            operator,
            value,
        } => {
            let column = if context == "discovery" {
                discovery_field_column(field)?
            } else {
                library_field_column(field)?
            };
            Ok(build_enum_condition(column, operator, value, params))
        }
        SmartCondition::Tags {
            operator, tag_ids, ..
        } => {
            if context == "discovery" {
                Ok(build_discovery_tag_condition(operator, tag_ids, params))
            } else {
                Ok(build_library_tag_condition(operator, tag_ids, params))
            }
        }
    }
}

/// Build the ORDER BY + LIMIT clause from a SmartLimit.
fn build_limit_sql(limit: &SmartLimit, context: &str, params: &mut Vec<Value>) -> Result<String> {
    let sort_col = if context == "discovery" {
        discovery_sort_column(&limit.sort_field)?
    } else {
        library_sort_column(&limit.sort_field)?
    };

    if limit.sort_field == "random" {
        params.push(Value::Integer(limit.count as i64));
        return Ok(format!("ORDER BY RANDOM() LIMIT ?{}", params.len()));
    }

    let dir = match limit.sort_direction {
        SortDirection::Ascending => "ASC",
        SortDirection::Descending => "DESC",
    };

    params.push(Value::Integer(limit.count as i64));
    Ok(format!("ORDER BY {sort_col} {dir} LIMIT ?{}", params.len()))
}

/// Build the full WHERE clause (and optional ORDER BY + LIMIT) for a library smart playlist.
/// Returns `(sql_fragment, params)` where `sql_fragment` is everything after `FROM tracks t WHERE`.
pub fn build_smart_query_library(rules: &SmartRules) -> Result<(String, Vec<Value>)> {
    let mut params: Vec<Value> = Vec::new();

    let condition_sqls: Vec<String> = rules
        .conditions
        .iter()
        .map(|c| build_condition_sql(c, "library", &mut params))
        .collect::<Result<Vec<_>>>()?;

    let where_clause = if condition_sqls.is_empty() {
        "1=1".to_string()
    } else {
        let joiner = match rules.match_mode {
            MatchMode::All => " AND ",
            MatchMode::Any => " OR ",
        };
        condition_sqls.join(joiner)
    };

    let mut sql = where_clause;

    if let Some(ref limit) = rules.limit {
        let limit_sql = build_limit_sql(limit, "library", &mut params)?;
        sql = format!("{sql} {limit_sql}");
    }

    Ok((sql, params))
}

/// Build the full WHERE clause (and optional ORDER BY + LIMIT) for a discovery smart playlist.
/// Returns `(sql_fragment, params)` where `sql_fragment` is everything after `FROM discovery_releases dr WHERE`.
pub fn build_smart_query_discovery(rules: &SmartRules) -> Result<(String, Vec<Value>)> {
    let mut params: Vec<Value> = Vec::new();

    let condition_sqls: Vec<String> = rules
        .conditions
        .iter()
        .map(|c| build_condition_sql(c, "discovery", &mut params))
        .collect::<Result<Vec<_>>>()?;

    let where_clause = if condition_sqls.is_empty() {
        "1=1".to_string()
    } else {
        let joiner = match rules.match_mode {
            MatchMode::All => " AND ",
            MatchMode::Any => " OR ",
        };
        condition_sqls.join(joiner)
    };

    let mut sql = where_clause;

    if let Some(ref limit) = rules.limit {
        let limit_sql = build_limit_sql(limit, "discovery", &mut params)?;
        sql = format!("{sql} {limit_sql}");
    }

    Ok((sql, params))
}

/// Validate that smart rules are well-formed for the given context.
pub fn validate_smart_rules(rules: &SmartRules, context: &str) -> Result<()> {
    for condition in &rules.conditions {
        match condition {
            SmartCondition::Text { field, .. }
            | SmartCondition::Numeric { field, .. }
            | SmartCondition::Date { field, .. }
            | SmartCondition::Enum { field, .. } => {
                // Validate field exists for context
                if context == "discovery" {
                    discovery_field_column(field)?;
                } else {
                    library_field_column(field)?;
                }
            }
            SmartCondition::Tags { .. } => {
                // Tags are valid in both contexts
            }
        }
    }

    if let Some(ref limit) = rules.limit {
        if context == "discovery" {
            discovery_sort_column(&limit.sort_field)?;
        } else {
            library_sort_column(&limit.sort_field)?;
        }
    }

    Ok(())
}
