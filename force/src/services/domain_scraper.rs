use std::time::Duration;

use crossbeam::channel::{Receiver, Sender};

const PAGE_DEPTH: u8 = 10;

use crate::routes::lead_route::{build_founder_seach_query, get_domain_from_url};

use super::{extract_data_from_google_search_with_reqwest, GoogleSearchResult, GoogleSearchType};

pub struct ProductQuerySender {
    pub sender: Sender<String>,
}

pub async fn domain_scraper_handler(product_query_receiver: Receiver<String>) {
    log::info!("Started domain scraper");
    loop {
        match product_query_receiver.recv() {
            Ok(query) => {
                tokio::spawn(scrape_domain_query(query));
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}

async fn scrape_domain_query(query: String) {
    // Fetch domain urls for url, if exist don't search

    let mut current_url = None;
    let mut domain_urls_list: Vec<String> = vec![];
    let mut page_source_list: Vec<(String, u8)> = vec![];
    let mut not_found = false;

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
                domain_urls_list.extend(domain_urls);
                page_source_list.push((page_source, current_page_index + 1));
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

    not_found = domain_urls_list.is_empty() && not_found;

    let domains: Vec<Option<String>> = domain_urls_list
        .iter()
        .map(|url| get_domain_from_url(url))
        .collect();
    let founder_search_queries: Vec<Option<String>> = domains
        .clone()
        .into_iter()
        .map(|dom| dom.as_deref().map(build_founder_seach_query))
        .collect();

    // (
    //     domain_urls_list,
    //     domains,
    //     founder_search_queries,
    //     query,
    //     not_found,
    //     page_source_list,
    // )
}
