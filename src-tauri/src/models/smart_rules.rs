use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    All,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOperator {
    Contains,
    NotContains,
    Equals,
    NotEquals,
    StartsWith,
    EndsWith,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    InRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateOperator {
    InLastDays,
    NotInLastDays,
    Before,
    After,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnumOperator {
    Equals,
    NotEquals,
    IsEmpty,
    IsNotEmpty,
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagOperator {
    HasAny,
    HasAll,
    HasNone,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SmartCondition {
    Text {
        field: String,
        operator: TextOperator,
        value: Option<String>,
    },
    Numeric {
        field: String,
        operator: NumericOperator,
        value: Option<f64>,
        value2: Option<f64>,
    },
    Date {
        field: String,
        operator: DateOperator,
        value: Option<String>,
    },
    Enum {
        field: String,
        operator: EnumOperator,
        value: Option<String>,
    },
    Tags {
        operator: TagOperator,
        tag_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartLimit {
    pub count: u32,
    pub sort_field: String,
    pub sort_direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRules {
    pub match_mode: MatchMode,
    pub conditions: Vec<SmartCondition>,
    pub limit: Option<SmartLimit>,
}
