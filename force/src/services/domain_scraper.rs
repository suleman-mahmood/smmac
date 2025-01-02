use std::{collections::HashSet, error::Error};

const PAGE_DEPTH: u8 = 1;
const SET_RESET_LEN: usize = 10_000;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    domain::html_tag::extract_domain,
    routes::lead_route::{build_founder_seach_queries, BLACK_LIST_DOMAINS},
};

use super::{
    extract_data_from_google_search_with_reqwest, DomainData, DomainPageData,
    FounderQueryChannelData, GoogleSearchResult, GoogleSearchType, PersistantData,
};

pub struct ProductQuerySender {
    pub sender: UnboundedSender<String>,
}

pub async fn domain_scraper_handler(
    mut product_query_receiver: UnboundedReceiver<String>,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Started domain scraper");
    let mut seen_queries = HashSet::new();

    // TODO: Use tokio::select! to check for a signal that asks to move certain tasks from priority queue to backgound
    while let Some(query) = product_query_receiver.recv().await {
        log::info!(
            "Domain scraper handler has {} elements",
            product_query_receiver.len()
        );

        match seen_queries.contains(&query) {
            true => {}
            false => {
                // TODO: Implement time based reset like 10 mins after channel was empty
                if seen_queries.len() > SET_RESET_LEN {
                    seen_queries.clear();
                }
                seen_queries.insert(query.clone());
                tokio::spawn(scrape_domain_query(
                    query,
                    founder_query_sender.clone(),
                    persistant_data_sender.clone(),
                ));
            }
        }
    }
}

async fn scrape_domain_query(
    query: String,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Scraping google for domain: {}", query);

    let mut current_url = None;
    let mut not_found = false;

    let mut pages_data: Vec<DomainPageData> = vec![];

    for current_page_index in 0..PAGE_DEPTH {
        let google_search_result = extract_data_from_google_search_with_reqwest(
            query.clone(),
            GoogleSearchType::Domain(current_url.clone()),
        )
        .await;

        match google_search_result {
            GoogleSearchResult::NotFound => {
                not_found = true;
                break;
            }
            GoogleSearchResult::Founders(..) => {
                log::error!("Returning founders from domain google search");
                break;
            }
            GoogleSearchResult::Domains {
                domain_urls,
                next_page_url,
                page_source,
            } => {
                for domain_url in domain_urls.iter() {
                    if let Some(domain) = extract_domain(domain_url.clone()) {
                        // Remove blacklisted domains
                        if !BLACK_LIST_DOMAINS
                            .iter()
                            .any(|&blacklist| domain.contains(blacklist))
                        {
                            for query in build_founder_seach_queries(&domain) {
                                founder_query_sender
                                    .send(FounderQueryChannelData {
                                        query,
                                        domain: domain.clone(),
                                    })
                                    .unwrap();
                            }
                        }
                    }
                }

                let data = DomainPageData {
                    page_source,
                    page_number: current_page_index + 1,
                    html_tags: domain_urls.clone(),
                    domains: domain_urls
                        .iter()
                        .map(|tag| extract_domain(tag.clone()))
                        .collect(),
                };
                pages_data.push(data);

                match next_page_url {
                    Some(url) => current_url = Some(url),
                    None => break,
                }
            }
            GoogleSearchResult::CaptchaBlocked => {
                log::error!("Returning from captcha blocked on url {}", query);
                break;
            }
        }
    }

    not_found = pages_data.is_empty() && not_found;

    if not_found {
        if let Err(e) =
            persistant_data_sender.send(PersistantData::Domain(DomainData::NoResult { query }))
        {
            log::error!(
                "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                e,
                e.source(),
            );
        }
    } else {
        let data = PersistantData::Domain(DomainData::Result { query, pages_data });
        if let Err(e) = persistant_data_sender.send(data) {
            log::error!(
                "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                e,
                e.source(),
            );
        }
    }
}
