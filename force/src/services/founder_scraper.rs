use std::time::Duration;

use crossbeam::channel::Receiver;

use crate::routes::lead_route::{extract_founder_names, FounderThreadResult};

use super::{extract_data_from_google_search_with_reqwest, GoogleSearchResult, GoogleSearchType};

pub struct FounderQueryChannelData {
    pub query: String,
    pub domain: String,
}

pub async fn founder_scraper_handler(founder_query_receiver: Receiver<FounderQueryChannelData>) {
    log::info!("Started founder scraper");
    loop {
        // TODO: Add seen set here to avoid scraping duplicate queries
        match founder_query_receiver.recv() {
            Ok(data) => {
                tokio::spawn(scrape_founder_query(data));
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

async fn scrape_founder_query(data: FounderQueryChannelData) {
    let google_search_result = extract_data_from_google_search_with_reqwest(
        data.query.clone(),
        GoogleSearchType::Founder(data.domain.clone()),
    )
    .await;

    _ = match google_search_result {
        GoogleSearchResult::NotFound => FounderThreadResult::NotFounder(data.domain),
        GoogleSearchResult::Domains { .. } => {
            log::error!("Returning domains from founder google search");
            FounderThreadResult::Ignore
        }
        GoogleSearchResult::Founders(tag_candidate, page_source) => {
            let founder_names = extract_founder_names(tag_candidate.clone());

            FounderThreadResult::Insert(tag_candidate, founder_names, data.domain, page_source)
        }
        GoogleSearchResult::CaptchaBlocked => {
            log::error!("Returning from captcha blocked on url {}", data.query);
            FounderThreadResult::Ignore
        }
    };
}
