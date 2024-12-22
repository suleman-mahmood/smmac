use serde::Deserialize;

pub struct GoogleWebPage {
    pub search_query: String,
    pub page_source: String,
    pub page_number: u8,
    pub data_extraction_intent: DataExtractionIntent,
    pub any_result: bool,
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(
    type_name = "DataExtractionIntent",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum DataExtractionIntent {
    Domain,
    FounderName,
    CompanyName,
}
