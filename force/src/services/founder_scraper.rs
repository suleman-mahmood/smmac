use std::{collections::HashSet, error::Error};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::domain::{
    email::{construct_email_permutations, FounderDomainEmail},
    html_tag::extract_founder_name,
};

use super::{
    extract_data_from_google_search_with_reqwest, FounderData, FounderPageData, GoogleSearchResult,
    GoogleSearchType, PersistantData,
};

const SET_RESET_LEN: usize = 10_000;

pub struct FounderQueryChannelData {
    pub query: String,
    pub domain: String,
}

pub async fn founder_scraper_handler(
    mut founder_query_receiver: UnboundedReceiver<FounderQueryChannelData>,
    email_sender: UnboundedSender<FounderDomainEmail>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Started founder scraper");
    let mut seen_queries = HashSet::new();

    while let Some(data) = founder_query_receiver.recv().await {
        log::info!(
            "Founder scraper handler has {} elements",
            founder_query_receiver.len()
        );

        match seen_queries.contains(&data.query) {
            true => {}
            false => {
                // TODO: Implement time based reset like 10 mins after channel was empty
                if seen_queries.len() > SET_RESET_LEN {
                    seen_queries.clear();
                }
                seen_queries.insert(data.query.clone());
                tokio::spawn(scrape_founder_query(
                    data,
                    email_sender.clone(),
                    persistant_data_sender.clone(),
                ));
            }
        }
    }
}

async fn scrape_founder_query(
    data: FounderQueryChannelData,
    email_sender: UnboundedSender<FounderDomainEmail>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Scraping google for founder: {}", data.query);

    let google_search_result = extract_data_from_google_search_with_reqwest(
        data.query.clone(),
        GoogleSearchType::Founder(data.domain.clone()),
    )
    .await;

    match google_search_result {
        GoogleSearchResult::NotFound => {
            if let Err(e) =
                persistant_data_sender.send(PersistantData::Founder(FounderData::NoResult {
                    query: data.query,
                }))
            {
                log::error!(
                    "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                    e,
                    e.source(),
                );
            }
        }
        GoogleSearchResult::Domains { .. } => {
            log::error!("Returning domains from founder google search");
        }
        GoogleSearchResult::Founders(tag_candidate, page_source) => {
            let founder_names: Vec<Option<String>> = tag_candidate
                .elements
                .iter()
                .map(|ele| extract_founder_name(ele.clone()))
                .collect();

            let emails: Vec<FounderDomainEmail> = founder_names
                .clone()
                .into_iter()
                .filter_map(|name| {
                    name.map(|name| construct_email_permutations(&name, &data.domain))
                })
                .flatten()
                .collect();

            for em in emails {
                email_sender.send(em.clone()).unwrap();

                if let Err(e) = persistant_data_sender.send(PersistantData::Email(em)) {
                    log::error!(
                        "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                        e,
                        e.source(),
                    );
                }
            }
            let page_data = FounderPageData {
                page_source: page_source.clone(),
                page_number: 1,
                html_tags: tag_candidate.elements.clone(),
                founder_names: founder_names.clone(),
            };

            if let Err(e) =
                persistant_data_sender.send(PersistantData::Founder(FounderData::Result {
                    query: data.query,
                    page_data,
                }))
            {
                log::error!(
                    "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                    e,
                    e.source(),
                );
            }
        }
        GoogleSearchResult::CaptchaBlocked => {
            log::error!("Returning from captcha blocked on url {}", data.query);
        }
    };
}
