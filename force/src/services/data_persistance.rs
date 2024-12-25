use std::time::Duration;

use crossbeam::channel::Receiver;

use crate::domain::html_tag::HtmlTag;

pub enum PersistantData {
    Domain(DomainData),
    Founder(FounderData),
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
        domain: String,
        page_data: FounderPageData,
    },
    NoResult {
        query: String,
        domain: String,
    },
}

pub struct FounderPageData {
    pub page_source: String,
    pub page_number: u8,
    pub html_tags: Vec<HtmlTag>,
    pub founder_names: Vec<Option<String>>,
}

pub async fn data_persistance_handler(data_receiver: Receiver<PersistantData>) {
    log::info!("Started data persistance handler");

    loop {
        match data_receiver.recv() {
            Ok(data) => {
                todo!();
            }

            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}
