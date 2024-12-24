use std::{collections::HashSet, time::Duration};

use crossbeam::channel::{Receiver, Sender};

const PAGE_DEPTH: u8 = 1;
const SET_RESET_LEN: usize = 10_000;

use crate::{
    domain::html_tag::HtmlTag,
    routes::lead_route::{build_founder_seach_query, get_domain_from_url, BLACK_LIST_DOMAINS},
};

use super::{
    extract_data_from_google_search_with_reqwest, DomainData, DomainPageData,
    FounderQueryChannelData, GoogleSearchResult, GoogleSearchType, PersistantData,
};

pub struct ProductQuerySender {
    pub sender: Sender<String>,
}

pub async fn domain_scraper_handler(
    product_query_receiver: Receiver<String>,
    founder_query_sender: Sender<FounderQueryChannelData>,
    persistant_data_sender: Sender<PersistantData>,
) {
    log::info!("Started domain scraper");
    let mut seen_queries = HashSet::new();

    loop {
        match product_query_receiver.recv() {
            Ok(query) => match seen_queries.contains(&query) {
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
            },

            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

async fn scrape_domain_query(
    query: String,
    founder_query_sender: Sender<FounderQueryChannelData>,
    persistant_data_sender: Sender<PersistantData>,
) {
    // Fetch domain urls for url, if exist don't search

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
                    if let Some(domain) = get_domain_from_url(domain_url) {
                        // Remove blacklisted domains
                        if BLACK_LIST_DOMAINS
                            .iter()
                            .any(|&blacklist| domain.contains(blacklist))
                        {
                            let query = build_founder_seach_query(&domain);
                            founder_query_sender
                                .send(FounderQueryChannelData { query, domain })
                                .unwrap();
                        }
                    }
                }

                let data = DomainPageData {
                    page_source,
                    page_number: current_page_index + 1,
                    html_tags: domain_urls
                        .clone()
                        .into_iter()
                        .map(|url| HtmlTag::ATag(url))
                        .collect(),
                    domains: domain_urls
                        .iter()
                        .map(|url| get_domain_from_url(url))
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
        persistant_data_sender
            .send(PersistantData::Domain(DomainData::NoResult { query }))
            .unwrap();
    } else {
        let data = PersistantData::Domain(DomainData::Result { query, pages_data });
        persistant_data_sender.send(data).unwrap();
    }
}
