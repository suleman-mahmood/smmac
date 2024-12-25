use std::time::Duration;

use scraper::{Html, Selector};
use serde::Serialize;

use crate::{
    domain::html_tag::HtmlTag, routes::lead_route::FounderTagCandidate, services::get_random_proxy,
};

const NUM_CAPTCHA_RETRIES: u8 = 10; // Should be > 0

pub enum GoogleSearchType {
    Domain(Option<String>),
    Founder(String),
}

pub enum GoogleSearchResult {
    NotFound,
    Domains {
        domain_urls: Vec<HtmlTag>,
        next_page_url: Option<String>,
        page_source: String,
    },
    Founders(FounderTagCandidate, String),
    CaptchaBlocked,
}

#[derive(Serialize)]
struct GoogleQuery {
    q: String,
}

pub async fn extract_data_from_google_search_with_reqwest(
    query: String,
    search_type: GoogleSearchType,
) -> GoogleSearchResult {
    const GOOGLE_URL: &str = "https://www.google.com/search";
    let a_tag_selector = Selector::parse("a").unwrap();
    let footer_selector = Selector::parse("footer").unwrap();
    let h3_selector = Selector::parse("h3").unwrap();

    let mut retry_count = 0;

    while retry_count < NUM_CAPTCHA_RETRIES {
        let proxy = get_random_proxy();
        let http_proxy = reqwest::Proxy::http(proxy.clone()).unwrap();
        let https_proxy = reqwest::Proxy::https(proxy.clone()).unwrap();

        let client = reqwest::Client::builder()
            .proxy(http_proxy)
            .proxy(https_proxy)
            .read_timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let query = GoogleQuery { q: query.clone() };

        let req = match search_type {
            GoogleSearchType::Domain(Some(ref next_page_url)) => {
                let url = format!("https://www.google.com{}", next_page_url);
                client.get(url)
            }
            _ => client.get(GOOGLE_URL).query(&query),
        };

        match req.send().await {
            Ok(res) => {
                let html_content_result = res.text().await;
                if let Err(ref e) = html_content_result {
                    log::error!("Failed to parse text from html_content. Error: {:?}", e);
                    retry_count += 1;
                    continue;
                }
                let html_content = html_content_result.unwrap();
                let html_document = Html::parse_document(&html_content);

                let headings: Vec<String> = html_document
                    .select(&h3_selector)
                    .map(|tag| tag.text().collect())
                    .collect();

                match headings.is_empty() {
                    true => match html_content.contains("did not match any documents") {
                        true => {
                            log::error!("Found no results on query: {}", query.q);
                            return GoogleSearchResult::NotFound;
                        }
                        false => {
                            log::error!("Blocked by captcha on query: {}", query.q);
                            retry_count += 1;
                        }
                    },
                    false => match search_type {
                        GoogleSearchType::Domain(_) => {
                            let links: Vec<String> = html_document
                                .select(&a_tag_selector)
                                .filter_map(|tag| {
                                    tag.value().attr("href").map(|url| url.to_string())
                                })
                                .collect();

                            let next_page_url = html_document
                                .select(&footer_selector)
                                .next()
                                .and_then(|footer| {
                                    footer.select(&a_tag_selector).next().and_then(
                                        |next_page_a_tag| {
                                            next_page_a_tag.attr("href").map(|url| url.to_string())
                                        },
                                    )
                                });

                            log::info!(
                                "Found {} urls with next page? {} | Potential domains",
                                links.len(),
                                next_page_url.is_some()
                            );

                            return GoogleSearchResult::Domains {
                                domain_urls: links.into_iter().map(HtmlTag::ATag).collect(),
                                next_page_url,
                                page_source: html_content,
                            };
                        }
                        GoogleSearchType::Founder(ref domain) => {
                            log::info!("Found {} h3_tags| Potential founder names", headings.len(),);

                            let elements = headings.into_iter().map(HtmlTag::H3Tag).collect();

                            return GoogleSearchResult::Founders(
                                FounderTagCandidate {
                                    elements,
                                    domain: domain.to_string(),
                                },
                                html_content,
                            );
                        }
                    },
                }
            }
            Err(e) => {
                log::error!("No response from reqwest, error: {:?}", e);
                retry_count += 1;
            }
        }
    }

    GoogleSearchResult::CaptchaBlocked
}
