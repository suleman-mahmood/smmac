use std::{collections::HashSet, time::Duration};

use crossbeam::channel::{Receiver, Sender};

use crate::routes::lead_route::{
    extract_founder_names, get_email_permutations, FounderDomainEmail, FounderThreadResult,
};

use super::{extract_data_from_google_search_with_reqwest, GoogleSearchResult, GoogleSearchType};

const SET_RESET_LEN: usize = 10_000;

pub struct FounderQueryChannelData {
    pub query: String,
    pub domain: String,
}

pub async fn founder_scraper_handler(
    founder_query_receiver: Receiver<FounderQueryChannelData>,
    email_sender: Sender<String>,
) {
    log::info!("Started founder scraper");
    let mut seen_queries = HashSet::new();

    loop {
        match founder_query_receiver.recv() {
            Ok(data) => match seen_queries.contains(&data.query) {
                true => {}
                false => {
                    // TODO: Implement time based reset like 10 mins after channel was empty
                    if seen_queries.len() > SET_RESET_LEN {
                        seen_queries.clear();
                    }
                    seen_queries.insert(data.query.clone());
                    tokio::spawn(scrape_founder_query(data, email_sender.clone()));
                }
            },
            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

async fn scrape_founder_query(data: FounderQueryChannelData, email_sender: Sender<String>) {
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

            let emails: Vec<FounderDomainEmail> = founder_names
                .clone()
                .into_iter()
                .filter_map(|name| name.map(|name| get_email_permutations(&name, &data.domain)))
                .flatten()
                .collect();

            for em in emails {
                email_sender.send(em.email).unwrap();
            }

            FounderThreadResult::Insert(tag_candidate, founder_names, data.domain, page_source)
        }
        GoogleSearchResult::CaptchaBlocked => {
            log::error!("Returning from captcha blocked on url {}", data.query);
            FounderThreadResult::Ignore
        }
    };
}
