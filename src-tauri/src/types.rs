use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Entry {
    pub format_name: String,
    pub format_id: u32,
    pub category: String,
    pub type_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Files(Vec<String>),
}

#[derive(Serialize)]
pub struct Output {
    pub formats: Vec<Entry>,
}
