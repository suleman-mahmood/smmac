struct WebPage {
    search_query: String,
    page_source: String,
    page_number: u8,
    data_extraction_intent: DataExtractionIntent,
    any_result: bool,
}

enum DataExtractionIntent {
    Domain,
    FounderName,
    CompanyName,
}
