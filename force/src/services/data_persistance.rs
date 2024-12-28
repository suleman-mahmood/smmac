use crossbeam::channel::Receiver;
use sqlx::{Acquire, PgPool};

use crate::{
    dal::{data_extract_db, email_db, google_webpage_db, html_tag_db},
    domain::{
        data_extract::DataExtract,
        email::{Email, FounderDomainEmail, Reachability, VerificationStatus},
        google_webpage::{DataExtractionIntent, GoogleWebPage},
        html_tag::HtmlTag,
    },
};

pub enum PersistantData {
    Domain(DomainData),
    Founder(FounderData),
    Email(FounderDomainEmail),
    UpdateEmailVerified(String),
}

pub enum DomainData {
    Result {
        query: String,
        pages_data: Vec<DomainPageData>,
    },
    NoResult {
        query: String,
    },
}

// TODO: Combine page data for domain and founder
pub struct DomainPageData {
    pub page_source: String,
    pub page_number: u8,
    pub html_tags: Vec<HtmlTag>,
    pub domains: Vec<Option<String>>,
}

pub enum FounderData {
    Result {
        query: String,
        page_data: FounderPageData,
    },
    NoResult {
        query: String,
    },
}

pub struct FounderPageData {
    pub page_source: String,
    pub page_number: u8,
    pub html_tags: Vec<HtmlTag>,
    pub founder_names: Vec<Option<String>>,
}

pub async fn data_persistance_handler(data_receiver: Receiver<PersistantData>, pool: PgPool) {
    log::info!("Started data persistance handler");

    for data in data_receiver.iter() {
        log::info!(
            "Data persistance handler has {} elements",
            data_receiver.len()
        );

        // TODO: Make sure that it can live long enough
        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        match data {
            PersistantData::Domain(data) => match data {
                DomainData::NoResult { query } => {
                    let webpage = GoogleWebPage {
                        search_query: query.clone(),
                        page_source: "".to_string(),
                        page_number: 0,
                        data_extraction_intent: DataExtractionIntent::Domain,
                        any_result: false,
                    };

                    google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();
                }
                DomainData::Result { query, pages_data } => {
                    for page_data in pages_data {
                        let webpage = GoogleWebPage {
                            search_query: query.clone(),
                            page_source: page_data.page_source,
                            page_number: page_data.page_number,
                            data_extraction_intent: DataExtractionIntent::Domain,
                            any_result: true,
                        };

                        let web_page_id = google_webpage_db::insert_web_page(con, webpage)
                            .await
                            .unwrap();

                        for (i, tag) in page_data.html_tags.into_iter().enumerate() {
                            let tag_id = html_tag_db::insert_html_tag(con, tag, web_page_id)
                                .await
                                .unwrap();

                            if let Some(Some(domain)) = page_data.domains.get(i) {
                                data_extract_db::insert_data(
                                    con,
                                    DataExtract::Domain(domain.to_string()),
                                    tag_id,
                                )
                                .await
                                .unwrap();
                            }
                        }
                    }
                }
            },
            PersistantData::Founder(data) => match data {
                FounderData::NoResult { query } => {
                    let webpage = GoogleWebPage {
                        search_query: query.clone(),
                        page_source: "".to_string(),
                        page_number: 0,
                        data_extraction_intent: DataExtractionIntent::FounderName,
                        any_result: false,
                    };

                    google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();
                }
                FounderData::Result { query, page_data } => {
                    let webpage = GoogleWebPage {
                        search_query: query.clone(),
                        page_source: page_data.page_source,
                        page_number: page_data.page_number,
                        data_extraction_intent: DataExtractionIntent::FounderName,
                        any_result: true,
                    };

                    let web_page_id = google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();

                    for (i, tag) in page_data.html_tags.into_iter().enumerate() {
                        let tag_id = html_tag_db::insert_html_tag(con, tag, web_page_id)
                            .await
                            .unwrap();

                        if let Some(Some(domain)) = page_data.founder_names.get(i) {
                            data_extract_db::insert_data(
                                con,
                                DataExtract::FounderName(domain.to_string()),
                                tag_id,
                            )
                            .await
                            .unwrap();
                        }
                    }
                }
            },
            PersistantData::Email(data) => {
                let email = Email {
                    email_address: data.email,
                    founder_name: data.founder_name,
                    domain: data.domain,
                    verification_status: VerificationStatus::Pending,
                    reachability: Reachability::Unknown,
                };
                if let Err(e) = email_db::insert_email(con, email).await {
                    log::error!("Error inserting email in db: {:?}", e);
                }
            }
            PersistantData::UpdateEmailVerified(email) => {
                _ = email_db::update_email_verified(con, email).await;
            }
        }
    }
}
