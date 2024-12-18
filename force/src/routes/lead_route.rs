use actix_web::{get, web, HttpResponse};
use check_if_email_exists::Reachable;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sqlx::{Acquire, PgPool};
use url::Url;

use crate::{
    dal::{
        config_db,
        lead_db::{self, EmailReachability, EmailVerifiedStatus},
    },
    services::{get_random_proxy, OpenaiClient, Sentinel},
};

const NUM_CAPTCHA_RETRIES: u8 = 10; // Should be > 0
pub const FRESH_RESULTS: bool = true; // Default to false
const BLACK_LIST_DOMAINS: [&str; 5] = ["reddit", "youtube", "pinterest", "amazon", "linkedin"];

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

    let niche = body.niche.trim().to_lowercase();

    save_product_search_queries(&pool, &openai_client, &niche).await;

    let domain_search_queries = lead_db::get_unscraped_products(&niche, &pool)
        .await
        .unwrap();

    let page_depth = config_db::get_google_search_page_depth(&pool)
        .await
        .unwrap_or(Some("1".to_string()))
        .unwrap_or("1".to_string())
        .parse()
        .unwrap_or(1);

    save_urls_from_google_searche_batch(&pool, domain_search_queries, page_depth).await;

    let domains_result = lead_db::get_domains_for_niche(&niche, &pool).await;
    if let Err(error) = domains_result {
        return HttpResponse::Ok().body(format!("Got error while fetching domains: {:?}", error));
    }
    let domains = domains_result.unwrap();

    let domains = lead_db::get_unscraped_domains(domains, &pool)
        .await
        .unwrap();

    // Remove blacklisted domains
    let domains: Vec<String> = domains
        .into_iter()
        .filter(|d| {
            !BLACK_LIST_DOMAINS
                .iter()
                .any(|&blacklist| d.contains(blacklist))
        })
        .collect();

    log::info!(
        "Finding founders for {} unique domains for niche {}",
        domains.len(),
        &niche
    );

    save_founders_from_google_searches_batch(&pool, domains.clone()).await;

    construct_emails(&pool, domains).await;

    let raw_emails_result = lead_db::get_raw_pending_emails_for_niche(&niche, &pool).await;
    if let Err(error) = raw_emails_result {
        return HttpResponse::Ok()
            .body(format!("Got error while fetching raw emails: {:?}", error));
    }
    let raw_emails = raw_emails_result.unwrap();

    log::info!("Emails to verify: {}", raw_emails.len());

    verify_emails(&pool, sentinel, raw_emails).await;

    match lead_db::get_verified_emails_for_niche(&niche, &pool).await {
        Ok(verified_emails) => match verified_emails.is_empty() {
            true => HttpResponse::Ok().body("No verified emails found"),
            false => {
                let catch_all_emails = lead_db::get_catch_all_emails_for_niche(&niche, &pool)
                    .await
                    .unwrap();

                log::info!("Found {} total verified emails", verified_emails.len());
                log::info!("Found {} catch all emails", catch_all_emails.len());
                log::info!(
                    "Found {} valid verified emails",
                    verified_emails.len() - catch_all_emails.len()
                );

                let valid_emails: Vec<String> = verified_emails
                    .into_iter()
                    .filter(|e| !catch_all_emails.contains(e))
                    .collect();

                HttpResponse::Ok().json(valid_emails)
            }
        },
        Err(e) => {
            log::error!("Error getting verified emails from db: {:?}", e);
            HttpResponse::Ok().body("Done!")
        }
    }
}

async fn save_product_search_queries(pool: &PgPool, openai_client: &OpenaiClient, niche: &str) {
    if !FRESH_RESULTS {
        if let Ok(Some(_)) = lead_db::get_product_search_queries(niche, pool).await {
            return;
        }
    }

    let (left_prompt, right_prompt) = config_db::get_gippity_prompt(pool).await.unwrap();
    let prompt = format!(
        "{} {} {}",
        left_prompt.unwrap_or("Give different names for the following product:".to_string()),
        niche,
        right_prompt.unwrap_or(r#"
            For example for product "yoga mat" similar products will be like: yoga block, silk yoga mat, yellow yoga mat, yoga mat bag, workout mat.
            Only return 10 product names in a list but don't start with a bullet point.
            Do not give numbers to products.
            Give each product on a new line.
        "#.to_string())
    );

    let products = openai_client
        .get_boolean_searches_from_niche(&prompt)
        .await
        .unwrap();

    let search_queries: Vec<String> = products.iter().map(|p| build_seach_query(p)).collect();

    _ = lead_db::insert_niche_products(products, search_queries, niche, pool).await;
}

async fn save_urls_from_google_searche_batch(
    pool: &PgPool,
    search_queries: Vec<String>,
    page_depth: u8,
) {
    const BATCH_SIZE: usize = 100;

    for batch in search_queries.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();

        for query in batch {
            let query = query.clone();

            handles.push(tokio::spawn(async move {
                // Fetch domain urls for url, if exist don't search

                let mut current_url = None;
                let mut domain_urls_list: Vec<String> = vec![];
                let mut not_found = false;

                for _ in 0..page_depth {
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

                (
                    domain_urls_list,
                    domains,
                    founder_search_queries,
                    query,
                    not_found,
                )
            }));
        }

        let mut handler_results = vec![];

        for handle in handles {
            let res = handle.await;
            if let Ok(r) = res {
                handler_results.push(r);
            }
        }

        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();
        for params in handler_results {
            // Save domain entries
            if let Err(e) = lead_db::insert_domain_candidate_urls(
                params.0, params.1, params.2, &params.3, params.4, con,
            )
            .await
            {
                log::error!(
                    "Error inserting domain candidate urls in db for url: {} and error: {:?}",
                    params.3,
                    e,
                )
            }
        }
    }
}

enum GoogleSearchType {
    Domain(Option<String>),
    Founder(String),
}

enum GoogleSearchResult {
    NotFound,
    Domains {
        domain_urls: Vec<String>,
        next_page_url: Option<String>,
    },
    Founders(FounderTagCandidate),
    CaptchaBlocked,
}

#[derive(Serialize)]
struct GoogleQuery {
    q: String,
}

async fn extract_data_from_google_search_with_reqwest(
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
                                domain_urls: links,
                                next_page_url,
                            };
                        }
                        GoogleSearchType::Founder(ref domain) => {
                            log::info!("Found {} h3_tags| Potential founder names", headings.len(),);

                            let elements = headings.into_iter().map(FounderElement::H3).collect();

                            return GoogleSearchResult::Founders(FounderTagCandidate {
                                elements,
                                domain: domain.to_string(),
                            });
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

enum FounderThreadResult {
    Insert(FounderTagCandidate, Vec<Option<String>>, String),
    NotFounder(String),
    Ignore,
}

async fn save_founders_from_google_searches_batch(pool: &PgPool, domains: Vec<String>) {
    const BATCH_SIZE: usize = 100;

    for batch in domains.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();

        for domain in batch {
            let domain = domain.clone();

            handles.push(tokio::spawn(async move {
                // TODO: Fetch query / url from db instead
                let query = build_founder_seach_query(&domain);

                let google_search_result = extract_data_from_google_search_with_reqwest(
                    query.to_string(),
                    GoogleSearchType::Founder(domain.to_string()),
                )
                .await;

                match google_search_result {
                    GoogleSearchResult::NotFound => FounderThreadResult::NotFounder(domain),
                    GoogleSearchResult::Domains { .. } => {
                        log::error!("Returning domains from founder google search");
                        FounderThreadResult::Ignore
                    }
                    GoogleSearchResult::Founders(tag_candidate) => {
                        let founder_names = extract_founder_names(tag_candidate.clone());

                        FounderThreadResult::Insert(tag_candidate, founder_names, domain)
                    }
                    GoogleSearchResult::CaptchaBlocked => {
                        log::error!("Returning from captcha blocked on url {}", query);
                        FounderThreadResult::Ignore
                    }
                }
            }));
        }

        let mut handler_results = vec![];

        for handle in handles {
            let res = handle.await;
            if let Ok(r) = res {
                handler_results.push(r);
            }
        }

        // Save results to db
        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        for params in handler_results {
            match params {
                FounderThreadResult::Insert(tag_candidate, founder_names, domain) => {
                    _ = lead_db::insert_founders(tag_candidate, founder_names, &domain, con).await;
                }
                FounderThreadResult::NotFounder(domain) => {
                    let _ = lead_db::insert_domain_no_results(&domain, con).await;
                }
                FounderThreadResult::Ignore => (),
            }
        }
    }
}

pub fn build_seach_query(product: &str) -> String {
    format!(r#""{}" AND "buy now""#, product.to_lowercase())
}

// TODO: Add more build search query permutations as needed
pub fn build_founder_seach_query(domain: &str) -> String {
    format!(
        r#"site:linkedin.com "{}" AND "founder""#,
        domain.to_lowercase()
    )
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
                            Some(h) => Some(h.to_lowercase()),
                            None => Some(any_host.to_lowercase()),
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

                strategies
                    .iter()
                    .filter_map(|st| {
                        content
                            .split_once(st)
                            .map(|parts| parts.0.trim().to_string())
                    })
                    .next()
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

async fn verify_emails(pool: &PgPool, sentinel: web::Data<Sentinel>, emails: Vec<String>) {
    const BATCH_SIZE: usize = 1000;

    for batch in emails.chunks(BATCH_SIZE) {
        let mut handles = Vec::new();

        for em in batch {
            let sentinel = sentinel.clone();
            let em = em.clone();

            handles.push(tokio::spawn(async move {
                let reachable = sentinel.get_email_verification_status(&em).await;
                let status = match reachable {
                    Reachable::Safe => EmailVerifiedStatus::Verified,
                    _ => EmailVerifiedStatus::Invalid,
                };
                let reachable: EmailReachability = reachable.into();

                (em, status, reachable)
            }));
        }

        let mut handler_results = vec![];

        for handle in handles {
            let res = handle.await;
            if let Ok(r) = res {
                handler_results.push(r);
            }
        }

        // update in lead db
        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        for params in handler_results {
            _ = lead_db::set_email_verification_reachability(&params.0, params.1, params.2, con)
                .await;
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
