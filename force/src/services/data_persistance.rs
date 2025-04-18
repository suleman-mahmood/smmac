use std::{error::Error, time::Duration};

use sqlx::{Acquire, PgPool};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    dal::{
        data_extract_db, email_db, google_webpage_db, html_tag_db,
        smart_scout_db::{self, SmartScoutJobStatus},
    },
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
    CompanyName(CompanyNameData),
    Email(FounderDomainEmail),
    UpdateEmailVerified(String),
    UpdateEmailUnverified(String),
    CompleteSmartScoutJob(i64),
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

pub enum CompanyNameData {
    Result {
        query: String,
        page_source: String,
        page_number: u8,
        html_tags: Vec<HtmlTag>,
        company_name: String,
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

pub async fn data_persistance_handler(
    mut data_receiver: UnboundedReceiver<PersistantData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
    pool: PgPool,
) {
    log::info!("Started data persistance handler");

    while let Some(data) = data_receiver.recv().await {
        log::info!(
            "Data persistance handler has {} elements",
            data_receiver.len()
        );

        // TODO: Make sure that it can live long enough
        let pool_con_result = pool.acquire().await;
        if let Err(e) = pool_con_result {
            log::error!("Pool timed out: {:?}", e);
            tokio::time::sleep(Duration::from_secs(10)).await;

            if let Err(e) = persistant_data_sender.send(data) {
                log::error!(
                    "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                    e,
                    e.source(),
                );
            }
            continue;
        }
        let mut pool_con = pool_con_result.unwrap();
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
                    match e {
                        sqlx::Error::Database(ref data) => {
                            if data.constraint() != Some("idx_unique_email_email_address") {
                                log::error!("Error inserting email in db: {:?}", e);
                            }
                        }
                        _ => {}
                    }
                }
            }
            PersistantData::UpdateEmailVerified(email) => {
                if let Err(e) = email_db::update_email_verified(con, email).await {
                    log::error!("Error while persisting email verified status: {:?}", e);
                }
            }
            PersistantData::UpdateEmailUnverified(email) => {
                if let Err(e) = email_db::update_email_unverified(con, email).await {
                    log::error!("Error while persisting email unverified status: {:?}", e);
                }
            }
            PersistantData::CompleteSmartScoutJob(smart_scout_id) => {
                if let Err(e) =
                    smart_scout_db::finish_job(con, smart_scout_id, SmartScoutJobStatus::Completed)
                        .await
                {
                    log::error!(
                        "Error while persisting smart scout job completion status: {:?}",
                        e
                    );
                }
            }
            PersistantData::CompanyName(data) => match data {
                CompanyNameData::NoResult { query } => {
                    let webpage = GoogleWebPage {
                        search_query: query.clone(),
                        page_source: "".to_string(),
                        page_number: 0,
                        data_extraction_intent: DataExtractionIntent::CompanyName,
                        any_result: false,
                    };

                    google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();
                }
                CompanyNameData::Result {
                    query,
                    page_source,
                    page_number,
                    company_name,
                    html_tags,
                } => {
                    let webpage = GoogleWebPage {
                        search_query: query.clone(),
                        page_source,
                        page_number,
                        data_extraction_intent: DataExtractionIntent::CompanyName,
                        any_result: true,
                    };

                    let web_page_id = google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();

                    for (i, tag) in html_tags.into_iter().enumerate() {
                        let tag_id = html_tag_db::insert_html_tag(con, tag, web_page_id)
                            .await
                            .unwrap();

                        if i == 0 {
                            data_extract_db::insert_data(
                                con,
                                DataExtract::CompanyName(company_name.clone()),
                                tag_id,
                            )
                            .await
                            .unwrap();
                        }
                    }
                }
            },
        }
    }
}
