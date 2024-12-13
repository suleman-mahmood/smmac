use std::time::Duration;

use actix_web::{get, web, HttpResponse};
use check_if_email_exists::Reachable;
use itertools::Itertools;
use rand::Rng;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use thirtyfour::{error::WebDriverError, prelude::ElementQueryable, By};
use url::Url;

use crate::{
    dal::lead_db::{self, EmailReachability, EmailVerifiedStatus},
    services::{make_new_driver, Droid, OpenaiClient, Sentinel},
};

const DEPTH_GOOGLE_SEACH_PAGES: u8 = 1; // Should be > 0
const NUM_CAPTCHA_RETRIES: u8 = 10; // Should be > 0

#[derive(Deserialize)]
struct GetLeadsFromNicheQuery {
    niche: String,
    // requester_email: String,
}

#[get("")]
async fn get_leads_from_niche(
    openai_client: web::Data<OpenaiClient>,
    body: web::Query<GetLeadsFromNicheQuery>,
    pool: web::Data<PgPool>,
    sentinel: web::Data<Sentinel>,
) -> HttpResponse {
    /*
    1. (v2) User verification and free tier count
    2. Get boolean search list from openai using the niche prompt
    3. Perform web scraping on each boolean search page, store results in db
        3.1 (v2) Rotate ips if getting blocked from google
    4. Construct emails from results in previous step
    5. Verify emails from API
    6. Return verified leads (emails)
    */

    let domain_search_queries =
        get_product_search_queries(&pool, &openai_client, &body.niche).await;

    let domains_result = get_urls_from_google_searches(&pool, domain_search_queries).await;
    if let Err(error) = domains_result {
        return HttpResponse::Ok().body(format!(
            "Got webdriver error from domain google searches: {:?}",
            error
        ));
    }
    // These domains are unique
    let domains = domains_result.unwrap();

    log::info!(
        "Got {} unique domains for niche {}",
        domains.len(),
        &body.niche
    );

    let raw_founders_result = get_founders_from_google_searches(&pool, domains.clone()).await;
    if let Err(error) = raw_founders_result {
        return HttpResponse::Ok().body(format!(
            "Got webdriver error from founder google searches: {:?}",
            error
        ));
    }
    let raw_founders = raw_founders_result.unwrap();
    let count = raw_founders.iter().fold(0, |acc, x| acc + x.elements.len());

    log::info!(">>> >>> >>>");
    log::info!("Total Raw Founders: {}", count);
    log::info!(">>> >>> >>>");

    let raw_emails = construct_emails(&pool, domains).await;

    log::info!(">>> >>> >>>");
    log::info!("Constructed emails: {}", raw_emails.len());
    log::info!(">>> >>> >>>");

    verify_emails(pool, sentinel, raw_emails).await;

    HttpResponse::Ok().body("Done!")
}

async fn get_product_search_queries(
    pool: &PgPool,
    openai_client: &OpenaiClient,
    niche: &str,
) -> Vec<String> {
    if let Ok(Some(search_queries)) = lead_db::get_product_search_queries(niche, pool).await {
        return search_queries;
    }

    let products = openai_client
        .get_boolean_searches_from_niche(niche)
        .await
        .unwrap();

    let search_queries: Vec<String> = products
        .iter()
        .map(|p| build_seach_query(p.to_string()))
        .collect();

    lead_db::insert_niche_products(products.clone(), search_queries.clone(), niche, pool).await;

    search_queries
}

async fn get_urls_from_google_searches(
    pool: &PgPool,
    search_queries: Vec<String>,
) -> Result<Vec<String>, WebDriverError> {
    // TODO: Dont' return domains, just save them to db
    let mut all_domains: Vec<String> = vec![];

    for query in search_queries.into_iter() {
        // Fetch domain urls for url, if exist don't search
        if let Ok(Some(domains)) = lead_db::get_domains(&query, pool).await {
            all_domains.extend(domains);
            continue;
        };

        let mut query = query.clone();
        let mut domain_urls_list: Vec<String> = vec![];
        let mut not_found = false;

        for _ in 0..DEPTH_GOOGLE_SEACH_PAGES {
            match extract_data_from_google_search_with_reqwest(
                query.clone(),
                GoogleSearchType::Domain,
            )
            .await?
            {
                GoogleSearchResult::NotFound => {
                    not_found = true;
                    break;
                }
                GoogleSearchResult::Founders(_) => {
                    log::error!("Returning founders from domain google search");
                    break;
                }
                GoogleSearchResult::Domains {
                    domain_urls,
                    next_page_url,
                } => {
                    domain_urls_list.extend(domain_urls);
                    match next_page_url {
                        Some(next_page_url) => query = next_page_url,
                        None => break,
                    }
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
            .map(|dom| dom.map(build_founder_seach_query))
            .collect();

        // Remove None domains
        let valid_domains: Vec<String> = domains.clone().into_iter().flatten().collect();
        all_domains.extend(valid_domains);

        // Save domain entries
        if let Err(e) = lead_db::insert_domain_candidate_urls(
            domain_urls_list,
            domains,
            founder_search_queries,
            &query,
            not_found,
            pool,
        )
        .await
        {
            log::error!(
                "Error inserting domain candidate urls in db for url: {} and error: {:?}",
                query,
                e
            )
        }
    }

    // Remove duplicate domains
    let all_domains = all_domains.into_iter().unique().collect();
    Ok(all_domains)
}

enum GoogleSearchType {
    Domain,
    Founder(String),
}

enum GoogleSearchResult {
    NotFound,
    Domains {
        domain_urls: Vec<String>,
        next_page_url: Option<String>,
    },
    Founders(FounderTagCandidate),
}

#[derive(Serialize)]
struct GoogleQuery {
    q: String,
}

async fn extract_data_from_google_search_with_reqwest(
    query: String,
    search_type: GoogleSearchType,
) -> Result<GoogleSearchResult, WebDriverError> {
    const GOOGLE_URL: &str = "https://www.google.com/search";
    let a_tag_selector = Selector::parse("a").unwrap();
    let footer_selector = Selector::parse("footer").unwrap();
    let h3_selector = Selector::parse("h3").unwrap();

    let mut retry_count = 0;

    while retry_count < NUM_CAPTCHA_RETRIES {
        // TODO: Add rotation proxy here
        let client = reqwest::Client::new();
        let query = GoogleQuery { q: query.clone() };

        match client.get(GOOGLE_URL).query(&query).send().await {
            Ok(res) => {
                let html_content = res.text().await.unwrap();
                let html_document = Html::parse_document(&html_content);

                let headings: Vec<String> = html_document
                    .select(&h3_selector)
                    .map(|tag| tag.text().collect())
                    .collect();

                match headings.is_empty() {
                    true => match html_content.contains("did not match any documents") {
                        true => {
                            log::error!("Found no results on query: {}", query.q);
                            return Ok(GoogleSearchResult::NotFound);
                        }
                        false => {
                            log::error!("Blocked by captcha on query: {}", query.q);
                            retry_count += 1;
                        }
                    },
                    false => match search_type {
                        GoogleSearchType::Domain => {
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
                                            next_page_a_tag
                                                .attr("href")
                                                .map(|url| format!("https://www.google.com{}", url))
                                        },
                                    )
                                });

                            log::info!(
                                "Found {} urls with next page? {} | Potential domains",
                                links.len(),
                                next_page_url.is_some()
                            );

                            return Ok(GoogleSearchResult::Domains {
                                domain_urls: links,
                                next_page_url,
                            });
                        }
                        GoogleSearchType::Founder(ref domain) => {
                            log::info!("Found {} h3_tags| Potential founder names", headings.len(),);

                            let elements = headings.into_iter().map(FounderElement::H3).collect();

                            return Ok(GoogleSearchResult::Founders(FounderTagCandidate {
                                elements,
                                domain: domain.to_string(),
                            }));
                        }
                    },
                }
            }
            Err(e) => {
                return Err(WebDriverError::RequestFailed(format!(
                    "No response from reqwest, error: {:?}",
                    e
                )))
            }
        }
    }

    Err(WebDriverError::RequestFailed(format!(
        "{} retries exceeded",
        NUM_CAPTCHA_RETRIES
    )))
}

async fn extract_data_from_google_search(
    droid: &web::Data<Droid>,
    url: String,
    search_type: GoogleSearchType,
) -> Result<GoogleSearchResult, WebDriverError> {
    let mut drivers = droid.drivers.lock().await;
    let mut rand = rand::thread_rng();
    let mut retry_count = 0;
    let mut captcha_blocked = false;

    let mut driver_index = rand.gen_range(0..drivers.len());
    let mut driver = drivers.get(driver_index).unwrap();

    while retry_count < NUM_CAPTCHA_RETRIES {
        if captcha_blocked {
            log::error!("Blocked by captcha on url: {}", url);

            driver.clone().quit().await.unwrap();
            drivers.remove(driver_index);

            let new_driver = make_new_driver().await;
            drivers.push(new_driver);

            retry_count += 1;
        }

        driver_index = rand.gen_range(0..drivers.len());
        driver = drivers.get(driver_index).unwrap();
        driver.goto(url.clone()).await?;

        match driver
            .query(By::XPath("//h3"))
            .wait(Duration::from_secs(11), Duration::from_secs(2))
            .exists()
            .await
        {
            // No h3 tag found
            Err(_) => match driver.source().await {
                Ok(source) => {
                    if source.contains("did not match any documents") {
                        log::error!("Found no results on url: {}", url);
                        return Ok(GoogleSearchResult::NotFound);
                    } else {
                        captcha_blocked = true
                    }
                }
                Err(_) => captcha_blocked = true,
            },
            Ok(exists) => {
                if exists {
                    match search_type {
                        GoogleSearchType::Domain => {
                            let mut domain_urls: Vec<String> = vec![];
                            let mut next_page_url = None;

                            for a_tag in driver.find_all(By::XPath("//a")).await? {
                                let href_attribute = a_tag.attr("href").await?;
                                if let Some(href) = href_attribute {
                                    domain_urls.push(href);
                                }
                            }

                            log::info!("Found {} urls | Potential domains", domain_urls.len(),);

                            if let Ok(next_page_element) =
                                driver.find(By::XPath(r#"//a[@id="pnnext"]"#)).await
                            {
                                if let Some(href_attribute) = next_page_element.attr("href").await?
                                {
                                    let next_url =
                                        format!("https://www.google.com{}", href_attribute);
                                    next_page_url = Some(next_url);
                                }
                            }

                            return Ok(GoogleSearchResult::Domains {
                                domain_urls,
                                next_page_url,
                            });
                        }
                        GoogleSearchType::Founder(ref domain) => {
                            let mut h3_tags = vec![];
                            let mut span_tags = vec![];

                            for h3_tag in driver.find_all(By::XPath("//h3")).await? {
                                let text = h3_tag.text().await?;
                                h3_tags.push(text);
                            }

                            // TODO: Update this query, returns no result
                            for span_tag in driver
                                .find_all(By::XPath(
                                    "//h3/following-sibling::div/div/div/div[1]/span",
                                ))
                                .await?
                            {
                                let text = span_tag.text().await?;
                                span_tags.push(text);
                            }

                            log::info!(
                                "Found {} h3_tags, {} span_tags | Potential founder names",
                                h3_tags.len(),
                                span_tags.len()
                            );

                            let elements = h3_tags
                                .into_iter()
                                .map(FounderElement::H3)
                                .chain(span_tags.into_iter().map(FounderElement::Span))
                                .collect();
                            return Ok(GoogleSearchResult::Founders(FounderTagCandidate {
                                elements,
                                domain: domain.to_string(),
                            }));
                        }
                    }
                } else {
                    match driver.source().await {
                        Ok(source) => {
                            if source.contains("did not match any documents") {
                                log::error!("Found no results on url: {}", url);
                                return Ok(GoogleSearchResult::NotFound);
                            } else {
                                captcha_blocked = true
                            }
                        }
                        Err(_) => captcha_blocked = true,
                    }
                }
            }
        }
    }

    Err(WebDriverError::RequestFailed(format!(
        "{} retries exceeded",
        NUM_CAPTCHA_RETRIES
    )))
}

#[derive(Debug, PartialEq, Clone)]
pub enum FounderElement {
    Span(String),
    H3(String),
}

#[derive(Debug, PartialEq, Clone)]
pub struct FounderTagCandidate {
    pub elements: Vec<FounderElement>, // TODO: Change this to return vec of names
    pub domain: String,
}

async fn get_founders_from_google_searches(
    pool: &PgPool,
    domains: Vec<String>,
) -> Result<Vec<FounderTagCandidate>, WebDriverError> {
    let mut founder_candidate: Vec<FounderTagCandidate> = vec![];

    for domain in domains {
        if let Ok(Some(founder_tags)) = lead_db::get_founder_tags(&domain, pool).await {
            founder_candidate.push(FounderTagCandidate {
                elements: founder_tags,
                domain,
            });
            continue;
        }

        // TODO: Fetch query / url from db instead
        let query = build_founder_seach_query(domain.clone());

        match extract_data_from_google_search_with_reqwest(
            query.to_string(),
            GoogleSearchType::Founder(domain.to_string()),
        )
        .await?
        {
            GoogleSearchResult::NotFound => {
                let _ = lead_db::insert_domain_no_results(&domain, pool).await;
                continue;
            }
            GoogleSearchResult::Domains { .. } => {
                log::error!("Returning domains from founder google search");
                continue;
            }
            GoogleSearchResult::Founders(tag_candidate) => {
                founder_candidate.push(tag_candidate.clone());

                // Save results to db
                let founder_names = extract_founder_names(tag_candidate.clone());
                lead_db::insert_founders(tag_candidate.clone(), founder_names, &domain, pool).await;
            }
        }
    }

    Ok(founder_candidate)
}

fn build_seach_query(product: String) -> String {
    format!(r#""{}" AND "buy now""#, product)
}

// TODO: Add more build search query permutations as needed
pub fn build_founder_seach_query(domain: String) -> String {
    format!(r#"site:linkedin.com "{}" AND "founder""#, domain)
}

pub fn get_domain_from_url(url: &str) -> Option<String> {
    match url.strip_prefix("/url?q=") {
        Some(url) => match Url::parse(url) {
            Ok(parsed_url) => match parsed_url.host_str() {
                Some("support.google.com") => None,
                Some("www.google.com") => None,
                Some("accounts.google.com") => None,
                Some("policies.google.com") => None,
                Some("www.amazon.com") => None,
                Some("") => None,
                None => None,
                Some(any_host) => {
                    if any_host.contains("google.com") {
                        None
                    } else {
                        match any_host.strip_prefix("www.") {
                            Some(h) => Some(h.to_string()),
                            None => Some(any_host.to_string()),
                        }
                    }
                }
            },
            Err(_) => None,
        },
        None => None,
    }
}

// TODO: Pass list of elements instead
pub fn extract_founder_names(founder_candidate: FounderTagCandidate) -> Vec<Option<String>> {
    founder_candidate
        .elements
        .iter()
        .map(|t| match t {
            FounderElement::Span(t) => match t.strip_prefix("LinkedIn Â· ") {
                Some(right_word) => {
                    let right_word_original = right_word.to_string();

                    let result = match right_word.split(",").collect::<Vec<&str>>().as_slice() {
                        [name, ..] => name.to_string(),
                        _ => right_word_original,
                    };

                    let result = match result.contains("Dr.") {
                        true => result.strip_prefix("Dr.").unwrap().trim().to_string(),
                        false => result,
                    };
                    let result = match result.contains("Dr") {
                        true => result.strip_prefix("Dr").unwrap().trim().to_string(),
                        false => result,
                    };

                    Some(result)
                }
                None => None,
            },
            FounderElement::H3(content) => {
                /*
                 Match with both in lowercase
                 1. Split by "'s Post -" and get content before the split
                 3. Split by "on LinkedIn" and get content before the split
                 4. Split by "posted on" and get content before the split
                 2. Split by "-" and get content before the split
                 5. Split by "|" and get content before the split
                */
                let strategies = [
                    "'s Post -",
                    "posted on",
                    "on LinkedIn",
                    "en LinkedIn",
                    "auf LinkedIn",
                    "sur LinkedIn",
                    "-",
                    "–", // I know, this is a different character
                    "|",
                ];

                let strategies: Vec<String> =
                    strategies.iter().map(|st| st.to_lowercase()).collect();
                let content = content.to_lowercase();

                let first_match = strategies
                    .iter()
                    .filter_map(|st| {
                        content
                            .split_once(st)
                            .map(|parts| parts.0.trim().to_string())
                    })
                    .next();

                first_match
            }
        })
        .collect()
}

#[derive(Clone)]
pub struct FounderDomain {
    pub founder_name: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FounderDomainEmail {
    pub founder_name: String,
    pub domain: String,
    pub email: String,
}

pub async fn construct_emails(pool: &PgPool, domains: Vec<String>) -> Vec<String> {
    if let Ok(Some(founder_domains)) = lead_db::get_founder_domains(domains, pool).await {
        let mut all_emails: Vec<String> = vec![];

        for fd in founder_domains {
            // Verify if already run
            if let Ok(Some(emails)) = lead_db::get_raw_emails(fd.clone(), pool).await {
                all_emails.extend(emails);
                continue;
            }

            let emails_db = get_email_permutations(&fd.founder_name, &fd.domain);
            if emails_db.is_empty() {
                continue;
            }

            all_emails.extend(emails_db.iter().map(|e| e.email.clone()));

            // Save emails in db
            lead_db::insert_emails(emails_db.clone(), pool).await;
        }

        return all_emails;
    }
    vec![]
}

fn get_email_permutations(name: &str, domain: &str) -> Vec<FounderDomainEmail> {
    let mut emails_db: Vec<FounderDomainEmail> = vec![];

    let name_pieces: Vec<&str> = name.split(" ").collect();
    if name_pieces.len() == 2 {
        let name_vec: Vec<&str> = name.split(" ").collect();
        let first_name = name_vec.first().unwrap().to_lowercase();
        let last_name = name_vec.get(1).unwrap().to_lowercase();

        emails_db.push(FounderDomainEmail {
            email: format!("{}@{}", first_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}@{}", last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}{}@{}", first_name, last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}.{}@{}", first_name, last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!(
                "{}{}@{}",
                first_name,
                last_name.chars().next().unwrap(),
                domain
            ),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!(
                "{}{}@{}",
                first_name.chars().next().unwrap(),
                last_name,
                domain
            ),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
    }

    emails_db
}

async fn verify_emails(
    pool: web::Data<PgPool>,
    sentinel: web::Data<Sentinel>,
    emails: Vec<String>,
) {
    const BATCH_SIZE: usize = 1000;

    for batch in emails.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();

        for em in batch {
            let pool = pool.clone();
            let sentinel = sentinel.clone();
            let em = em.clone();

            handles.push(tokio::spawn(async move {
                let reachable = sentinel.get_email_verification_status(&em).await;
                let status = match reachable {
                    Reachable::Safe => EmailVerifiedStatus::Verified,
                    _ => EmailVerifiedStatus::Invalid,
                };
                let reachable: EmailReachability = reachable.into();
                _ = lead_db::set_email_verification_reachability(&em, reachable, status, &pool)
                    .await;
            }));
        }

        for handle in handles {
            _ = handle.await;
        }
    }
}

pub async fn filter_verified_emails(
    sentinel: web::Data<Sentinel>,
    emails: Vec<String>,
) -> Vec<String> {
    let mut verified_emails: Vec<String> = vec![];

    for em in emails {
        if sentinel.verfiy_email(em.clone()).await {
            verified_emails.push(em);
        }
    }

    verified_emails
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use crate::routes::lead_route::{
        extract_founder_names, get_domain_from_url, get_email_permutations, FounderElement,
        FounderTagCandidate,
    };

    #[test]
    fn get_domain_from_url_valid() {
        let raw_urls = [
            "https://support.google.com/websearch/answer/181196?hl=en-PK",
            "https://www.google.com/webhp?hl=en&sa=X&ved=0ahUKEwi2j67hto6KAxWkyDgGHXxuE0wQPAgI",
            "https://www.google.com.pk/intl/en/about/products?tab=wh",
            "https://accounts.google.com/ServiceLogin?hl=en&passive=true&continue=https://www.google.com/search%3Fq%3D%2522Organic%2520Green%2520Tea%2522%2520AND%2520%2522buy%2520now%2522&ec=GAZAAQ",
            "/search?sca_esv=0c2f7fc6ddd47e94&q=%22Organic+Green+Tea%22+AND+%22buy+now%22&udm=2&fbs=AEQNm0Aa4sjWe7Rqy32pFwRj0UkWd8nbOJfsBGGB5IQQO6L3JyJJclJuzBPl12qJyPx7ESJehObpS5jg6J88CCM-RK72sNV8xvbUxy-SoOtM-WmPLIjZzuRzEJJ0u2V8OeDS2QzrFq0l6uL0u5ydk68vXkBqxln9Kbinx1HZnJEg4P6VfVQ98eE&sa=X&ved=2ahUKEwi2j67hto6KAxWkyDgGHXxuE0wQtKgLegQIFhAB",
            "/finance?sca_esv=0c2f7fc6ddd47e94&output=search&q=%22Organic+Green+Tea%22+AND+%22buy+now%22&source=lnms&fbs=AEQNm0Aa4sjWe7Rqy32pFwRj0UkWd8nbOJfsBGGB5IQQO6L3JyJJclJuzBPl12qJyPx7ESJehObpS5jg6J88CCM-RK72sNV8xvbUxy-SoOtM-WmPLIjZzuRzEJJ0u2V8OeDS2QzrFq0l6uL0u5ydk68vXkBqxln9Kbinx1HZnJEg4P6VfVQ98eE&sa=X&ved=2ahUKEwi2j67hto6KAxWkyDgGHXxuE0wQ0pQJegQIExAB",
            "https://policies.google.com/privacy?hl=en-PK&fg=1",
            "https://policies.google.com/terms?hl=en-PK&fg=1",
            "https://accounts.google.com/ServiceLogin?hl=en&passive=true&continue=https://www.google.com/search%3Fq%3D%2522Organic%2BAgave%2BNectar%2522%2BAND%2B%2522buy%2Bnow%2522%26sca_esv%3D0c2f7fc6ddd47e94%26ei%3DZHVQZ6CXDqCo4-EPlJeE4AM%26start%3D40%26sa%3DN%26ved%3D2ahUKEwig2YKat46KAxUg1DgGHZQLATw4HhDw0wN6BAgJEBU&ec=GAZAAQ",
            "#",
            "https://www.amazon.com/Organic-Pure-Green-Tea-Bags/dp/B00FTAYNKE",
        ];
        for url in raw_urls {
            let result = get_domain_from_url(url);
            assert!(result.is_none());
        }
    }

    #[test]
    fn filter_raw_urls_valid() {
        let raw_urls = [
            "https://www.znaturalfoods.com/products/green-tea-organic",
            "https://dallosell.com/product_detail/organic-green-tea-bag",
            "https://www.verywellfit.com/best-green-teas-5115813#:~:text=Certified%20organic%2C%20non%2DGMO%2C,Kyushu%20Island%20in%20southern%20Japan.",
            "https://www.medicalnewstoday.com/articles/269538#:~:text=Research%20suggests%20it%20is%20safe,or%20interact%20with%20certain%20medications.",
            "https://www.healthline.com/nutrition/top-10-evidence-based-health-benefits-of-green-tea#:~:text=A%202017%20research%20paper%20found,middle%2Daged%20and%20older%20adults.",
            "https://organicindia.com/collections/green-tea?srsltid=AfmBOopzdn4oOzfSwiaITNekbORRUG_MoVF67dULVE9IEHV6zlvZL0Qc",
            "https://www.traditionalmedicinals.com/products/green-tea-matcha?srsltid=AfmBOoqwv1CiL0XV_zNFmIWU1biT3S4xa-7KkOLzgXN4BkSCscGZFXzS",
        ];

        let expected = [
            "znaturalfoods.com",
            "dallosell.com",
            "verywellfit.com",
            "medicalnewstoday.com",
            "healthline.com",
            "organicindia.com",
            "traditionalmedicinals.com",
        ];
        for (url, expected) in raw_urls.iter().zip(expected.iter()) {
            let result = get_domain_from_url(url);
            assert!(result.is_some());
            assert_eq!(result.unwrap(), expected.to_string());
        }
    }

    #[test]
    fn extract_founder_names_valid() {
        let candidate = FounderTagCandidate {
            elements: vec![
                // FounderElement::Span("LinkedIn Â· Dan Go".to_string()),
                // FounderElement::Span("LinkedIn Â· Dan Go".to_string()),
                // FounderElement::Span("LinkedIn Â· HÃ©lÃ¨ne de Troostembergh".to_string()),
                // FounderElement::Span("LinkedIn Â· Samina Qureshi, RDN LD".to_string()),
                // FounderElement::Span("LinkedIn Â· Wondercise Technology Corp.".to_string()),
                // FounderElement::Span("LinkedIn Â· Dr Veer Pushpak Gupta".to_string()),
                // FounderElement::Span("LinkedIn Â· Hasnain Sajjad".to_string()),
                // FounderElement::Span("LinkedIn Â· Deepak L. Bhatt, MD, MPH, MBA".to_string()),
                // FounderElement::Span("LinkedIn Â· Dr. Ronald Klatz, MD, DO".to_string()),
                // FounderElement::Span("LinkedIn Â· WellTheory".to_string()),
                // FounderElement::Span("LinkedIn Â· WellTheory".to_string()),
                // FounderElement::Span("LinkedIn Â· West Shell III".to_string()),
                // FounderElement::Span("LinkedIn Â· Cathy Cassata".to_string()),
                // FounderElement::Span("LinkedIn Â· Shravan Verma".to_string()),
                // FounderElement::Span("LinkedIn Â· anwar khan".to_string()),
                // FounderElement::Span("LinkedIn Â· Christopher Dean".to_string()),
                // FounderElement::Span("LinkedIn India".to_string()),
                // FounderElement::Span("LinkedIn".to_string()),
                // FounderElement::H3("Dan Go's Post".to_string()),
                // FounderElement::H3("Eric Chuang on LinkedIn: Putting up the sign!".to_string()),
                // FounderElement::H3("Dan Buettner's Post".to_string()),
                // FounderElement::H3("Sarah Garone's Post".to_string()),
                // FounderElement::H3(
                //     "HÃ©lÃ¨ne de Troostembergh - Truly inspiring Tanguy Goretti".to_string(),
                // ),
                // FounderElement::H3("Samina Qureshi, RDN LD's Post".to_string()),
                // FounderElement::H3("Tanguy Goretti's Post".to_string()),
                // FounderElement::H3("Wondercise Technology Corp.".to_string()),
                // FounderElement::H3("Dr. Gwilym Roddick's Post".to_string()),
                // FounderElement::H3(
                //     "Honor Whiteman - Senior Editorial Director - RVO Health".to_string(),
                // ),
                // FounderElement::H3(
                //     "Tim Snaith - Newsletter Editor II - Medical News Today".to_string(),
                // ),
                // FounderElement::H3("Hasnain Sajjad on LinkedIn: #al".to_string()),
                // FounderElement::H3(
                //     "Dr Veer Pushpak Gupta - nhs #healthcare #unitedkingdom".to_string(),
                // ),
                // FounderElement::H3("Beth Frates, MD's Post".to_string()),
                // FounderElement::H3("Deepak L. Bhatt, MD, MPH, MBA's Post".to_string()),
                // FounderElement::H3("Dr. Ronald Klatz, MD, DO's Post".to_string()),
                // FounderElement::H3("WellTheory".to_string()),
                // FounderElement::H3("Uma Naidoo, MD".to_string()),
                // FounderElement::H3("Dr William Bird MBE's Post".to_string()),
                // FounderElement::H3("Georgette Smart - CEO E*HealthLine".to_string()),
                // FounderElement::H3("David Kopp's Post".to_string()),
                // FounderElement::H3(
                //     "West Shell III - GOES (Global Outdoor Emergency Support)".to_string(),
                // ),
                // FounderElement::H3(
                //     "Cathy Cassata - Freelance Writer - Healthline Networks, Inc.".to_string(),
                // ),
                // FounderElement::H3("Healthline Media".to_string()),
                // FounderElement::H3("Health Line - Healthline Team Member".to_string()),
                // FounderElement::H3("David Mills - Associate editor - healthline.com".to_string()),
                // FounderElement::H3("Kevin Yoshiyama - Healthline Media".to_string()),
                // FounderElement::H3("Cortland Dahl's Post".to_string()),
                // FounderElement::H3("Kelsey Costa, MS, RDN's Post".to_string()),
                // FounderElement::H3("babulal parashar - great innovation".to_string()),
                // FounderElement::H3("Shravan Verma - Manager - PANI".to_string()),
                // FounderElement::H3("anwar khan's Post".to_string()),
                // FounderElement::H3(
                //     "Christopher Dean - Sculptor Marble dreaming. collaborator ...".to_string(),
                // ),
                // FounderElement::H3("Manish Ambast's Post".to_string()),
                // FounderElement::H3("Mark Balderman Highlove - Installation Specialist".to_string()),
                // FounderElement::H3("100+ \"Partho Roy\" profiles".to_string()),
                // FounderElement::H3(
                //     "James Weisz on LinkedIn: #website #developer #film".to_string(),
                // ),
                // FounderElement::H3(
                //     "Ravindra Prakash - Plant Manager - Shree Dhanwantri ...".to_string(),
                // ),
                // FounderElement::H3("Traditional Medicinals".to_string()),
                // FounderElement::H3("Caitlin Landesberg on LinkedIn: Home".to_string()),
                // FounderElement::H3("Traditional Medicinals".to_string()),
                // FounderElement::H3("Joe Stanziano's Post".to_string()),
                // FounderElement::H3("Traditional Medicinals | à¦²à¦¿à¦‚à¦•à¦¡à¦‡à¦¨".to_string()),
                // FounderElement::H3("Kathy Avilla - Traditional Medicinals, Inc.".to_string()),
                // FounderElement::H3("Ben Hindman's Post - sxsw".to_string()),
                // FounderElement::H3("David Templeton - COMMUNITY ACTION OF NAPA VALLEY".to_string()),
                FounderElement::H3("Swati Bhargava - CashKaro.com - LinkedIn".to_string()),
                FounderElement::H3("Rohan Bhargava - CashKaro.com - LinkedIn".to_string()),
                // FounderElement::H3("Yatinn Ram Garg - CashKaro.com - LinkedIn".to_string()),
                // FounderElement::H3(
                //     "Swati Bhargava's Post - Co-founder of CashKaro.com - LinkedIn".to_string(),
                // ),
                // FounderElement::H3(
                //     "Piyush Sood - Senior Manager (Entrepreneur In Residence) - LinkedIn"
                //         .to_string(),
                // ),
                // FounderElement::H3("Ishan Agarwal - CashKaro.com - LinkedIn".to_string()),
                // FounderElement::H3(
                //     "Swati Bhargava - How we launched CashKaro.com in India - LinkedIn".to_string(),
                // ),
                // FounderElement::H3("Swati Bhargava on LinkedIn: April Case Study".to_string()),
                // FounderElement::H3(
                //     "Swati Bhargava on LinkedIn: #valentinesday | 24 comments".to_string(),
                // ),
                // FounderElement::H3(
                //     "BusinessOnBot on LinkedIn: CashKaro's Founder Swati Bhargava ...".to_string(),
                // ),
                // FounderElement::H3("Michael Moor - Foods Alive | LinkedIn".to_string()),
                // FounderElement::H3("BAGHIR GULIYEV - Packer - FOOD TO LIVE - LinkedIn".to_string()),
                // FounderElement::H3("Michael Moor - Foods Alive | LinkedIn".to_string()),
                // FounderElement::H3(
                //     "Jeremy Hinds on LinkedIn: #experience #future #food #brand ...".to_string(),
                // ),
                // FounderElement::H3(
                //     "Gagandeep Singh - Co-Founder and CEO - G9 Fresh | LinkedIn".to_string(),
                // ),
                // FounderElement::H3(
                //     "Linda Boardman - Bragg Live Food Products, LLC | LinkedIn".to_string(),
                // ),
                // FounderElement::H3(
                //     "Kate K - Graphic Designer/SMM - Food To Live | LinkedIn".to_string(),
                // ),
                // FounderElement::H3("Food for Life - LinkedIn".to_string()),
                // FounderElement::H3("Khaled Elithy's Post - LinkedIn".to_string()),
                // FounderElement::H3(
                //     "James Rickert on LinkedIn: #foodsystem #investment #partnership ..."
                //         .to_string(),
                // ),
                // FounderElement::H3(
                //     "Alexis Eyre on LinkedIn: #marketing #advertising #foodmarketing ..."
                //         .to_string(),
                // ),
            ],
            domain: "verywellfit.com".to_string(),
        };

        let expected = vec![
            // "Dan Go".to_string(),
            // "HÃ©lÃ¨ne de Troostembergh".to_string(),
            // "Samina Qureshi".to_string(),
            // "Wondercise Technology Corp.".to_string(),
            // "Veer Pushpak Gupta".to_string(),
            // "Hasnain Sajjad".to_string(),
            // "Deepak L. Bhatt".to_string(),
            // "Ronald Klatz".to_string(),
            // "WellTheory".to_string(),
            // "West Shell III".to_string(),
            // "Cathy Cassata".to_string(),
            // "Shravan Verma".to_string(),
            // "anwar khan".to_string(),
            // "Christopher Dean".to_string(),
            "swati bhargava".to_string(),
            "rohan bhargava".to_string(),
        ];

        let results = extract_founder_names(candidate);
        let results: Vec<String> = results.into_iter().flatten().collect();
        let results: Vec<String> = results.into_iter().unique().collect();
        assert_eq!(results, expected)
    }

    #[test]
    fn construct_email_permutations_valid() {
        let names = [
            "Dan Go".to_string(),
            "HÃ©lÃ¨ne de Troostembergh".to_string(),
            "Samina Qureshi".to_string(),
            "Wondercise Technology Corp.".to_string(),
            "Veer Pushpak Gupta".to_string(),
            "Deepak L. Bhatt".to_string(),
            "WellTheory".to_string(),
            "West Shell III".to_string(),
        ];

        let expected = vec![
            "dan@verywellfit.com".to_string(),
            "go@verywellfit.com".to_string(),
            "dango@verywellfit.com".to_string(),
            "dan.go@verywellfit.com".to_string(),
            "dang@verywellfit.com".to_string(),
            "dgo@verywellfit.com".to_string(),
            "samina@verywellfit.com".to_string(),
            "qureshi@verywellfit.com".to_string(),
            "saminaqureshi@verywellfit.com".to_string(),
            "samina.qureshi@verywellfit.com".to_string(),
            "saminaq@verywellfit.com".to_string(),
            "squreshi@verywellfit.com".to_string(),
        ];

        let mut results: Vec<String> = vec![];
        for name in names {
            let emails = get_email_permutations(&name, "verywellfit.com");
            results.extend(emails.into_iter().map(|e| e.email));
        }

        assert_eq!(results, expected)
    }
}
