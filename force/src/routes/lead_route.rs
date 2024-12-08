use actix_web::{get, web, HttpResponse};
use itertools::Itertools;
use rand::Rng;
use serde::Deserialize;
use sqlx::PgPool;
use thirtyfour::{error::WebDriverError, By};
use url::Url;

use crate::{
    dal::lead_db,
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
    droid: web::Data<Droid>,
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

    let domain_search_urls = match lead_db::get_product_search_queries(&body.niche, &pool).await {
        Ok(search_queries) => search_queries,
        Err(_) => {
            let products = openai_client
                .get_boolean_searches_from_niche(&body.niche)
                .await
                .unwrap();

            let search_queries: Vec<String> = products
                .iter()
                .map(|p| build_seach_url(p.to_string()))
                .collect();

            lead_db::insert_niche_products(
                products.clone(),
                search_queries.clone(),
                &body.niche,
                &pool,
            )
            .await
            .unwrap();

            search_queries
        }
    };

    let domains_result = get_urls_from_google_searches(&droid, &pool, domain_search_urls).await;
    if let Err(error) = domains_result {
        return HttpResponse::Ok().body(format!(
            "Got webdriver error from domain google searches: {:?}",
            error
        ));
    }
    let domains = domains_result.unwrap();

    let raw_founders_result = get_founders_from_google_searches(&droid, &pool, domains).await;
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

    let founders = extract_founder_names(raw_founders);

    let count = founders.iter().fold(0, |acc, x| acc + x.names.len());

    log::info!(">>> >>> >>>");
    log::info!("Total Qualified Founders: {}", count);
    log::info!(">>> >>> >>>");

    let raw_emails = construct_emails(founders);

    log::info!(">>> >>> >>>");
    log::info!("Constructed emails: {}", raw_emails.len());
    log::info!(">>> >>> >>>");

    // let emails = filter_verified_emails(sentinel, raw_emails).await;

    // HttpResponse::Ok().body(format!("Verified emails: {:?}", emails))
    HttpResponse::Ok().json(raw_emails)
}

async fn get_urls_from_google_searches(
    droid: &web::Data<Droid>,
    pool: &PgPool,
    search_urls: Vec<String>,
) -> Result<Vec<String>, WebDriverError> {
    // TODO: Dont' return urls, just save them to db
    let mut all_domains: Vec<String> = vec![];

    for url in search_urls.into_iter() {
        // Fetch domain urls for url, if exist don't search
        if let Ok(domains) = lead_db::get_domains(&url, pool).await {
            if !domains.is_empty() {
                all_domains.extend(domains);
                continue;
            }
        };

        let mut current_url = url.clone();
        let mut domain_urls_list: Vec<String> = vec![];

        for _ in 0..DEPTH_GOOGLE_SEACH_PAGES {
            match extract_data_from_google_search(
                droid,
                current_url.clone(),
                GoogleSearchType::Domain,
            )
            .await?
            {
                GoogleSearchResult::NotFound => break,
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
                        Some(next_page_url) => current_url = next_page_url,
                        None => break,
                    }
                }
            }
        }

        let domains: Vec<Option<String>> = domain_urls_list
            .iter()
            .map(|url| get_domain_from_url(url))
            .collect();
        let founder_search_urls: Vec<Option<String>> = domains
            .clone()
            .into_iter()
            .map(|dom| dom.map(build_founder_seach_url))
            .collect();

        // Remove None founders
        let valid_domains: Vec<String> = domains.clone().into_iter().flatten().collect();
        all_domains.extend(valid_domains);

        // Save domain entries
        if let Err(e) = lead_db::insert_domain_candidate_urls(
            domain_urls_list,
            domains,
            founder_search_urls,
            &url,
            pool,
        )
        .await
        {
            log::error!(
                "Error inserting domain candidate urls in db for url: {} and error: {:?}",
                url,
                e
            )
        }
    }

    // Remove duplicate search urls
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

        match driver.find(By::XPath("//h3")).await {
            // No h3 tag found
            Err(_) => match driver
                .find(By::XPath("//div[contains(@class, 'card-section')]/ul"))
                .await
            {
                // There are no results on page
                Ok(_) => {
                    log::error!("Found no results on url: {}", url);
                    return Ok(GoogleSearchResult::NotFound);
                }
                // You have been blocked by captcha
                Err(_) => captcha_blocked = true,
            },
            Ok(_) => match search_type {
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
                        if let Some(href_attribute) = next_page_element.attr("href").await? {
                            let next_url = format!("https://www.google.com{}", href_attribute);
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

                    for span_tag in driver
                        .find_all(By::XPath("//h3/following-sibling::div/div/div/div[1]/span"))
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
            },
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
    pub elements: Vec<FounderElement>,
    pub domain: String,
}

#[derive(Debug, PartialEq)]
struct DomainFounderQualified {
    names: Vec<String>,
    domain: String,
}

async fn get_founders_from_google_searches(
    droid: &web::Data<Droid>,
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

        // TODO: Fetch url from db instead
        let url = build_founder_seach_url(domain.clone());

        match extract_data_from_google_search(
            droid,
            url.to_string(),
            GoogleSearchType::Founder(domain.to_string()),
        )
        .await?
        {
            GoogleSearchResult::NotFound => continue,
            GoogleSearchResult::Domains { .. } => {
                log::error!("Returning domains from founder google search");
                continue;
            }
            GoogleSearchResult::Founders(tag_candidate) => {
                founder_candidate.push(tag_candidate.clone());

                // Save results to db
                // TODO: insert founder_name as well
                if let Err(e) = lead_db::insert_founders(tag_candidate.clone(), &domain, pool).await
                {
                    log::error!(
                        "Error inserting domain founders in db for domain: {} and candidates: {:?} and error {:?}",
                        domain,
                        tag_candidate,
                        e
                    )
                }
            }
        }
    }

    Ok(founder_candidate)
}

fn build_seach_url(product: String) -> String {
    let boolean_query = format!(r#""{}" AND "buy now""#, product);
    format!("https://www.google.com/search?q={}", boolean_query)
}

fn build_founder_seach_url(domain: String) -> String {
    // TODO: Add more build search url permutations as needed
    let boolean_query = format!(r#"site:linkedin.com "{}" AND "founder""#, domain);
    format!("https://www.google.com/search?q={}", boolean_query)
}

fn get_domain_from_url(url: &str) -> Option<String> {
    match Url::parse(url) {
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
    }
}

fn extract_founder_names(
    founder_candidates: Vec<FounderTagCandidate>,
) -> Vec<DomainFounderQualified> {
    founder_candidates
        .iter()
        .map(|fc| {
            let tags: Vec<String> = fc
                .elements
                .iter()
                .filter_map(|t| match t {
                    FounderElement::Span(t) => match t.strip_prefix("LinkedIn Â· ") {
                        Some(right_word) => {
                            let right_word_original = right_word.to_string();

                            let result =
                                match right_word.split(",").collect::<Vec<&str>>().as_slice() {
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
                    FounderElement::H3(_) => None,
                })
                .collect();

            let tags = tags.into_iter().unique().collect();
            DomainFounderQualified {
                names: tags,
                domain: fc.domain.clone(),
            }
        })
        .collect()
}

fn construct_emails(domain_founders: Vec<DomainFounderQualified>) -> Vec<String> {
    let mut emails: Vec<String> = vec![];

    for df in domain_founders {
        for name in df.names {
            let name_pieces: Vec<&str> = name.split(" ").collect();
            if name_pieces.len() == 2 {
                let name_vec: Vec<&str> = name.split(" ").collect();
                let first_name = name_vec.first().unwrap().to_lowercase();
                let last_name = name_vec.get(1).unwrap().to_lowercase();

                emails.push(format!("{}@{}", first_name, df.domain));
                emails.push(format!("{}@{}", last_name, df.domain));
                emails.push(format!("{}{}@{}", first_name, last_name, df.domain));
                emails.push(format!("{}.{}@{}", first_name, last_name, df.domain));
                emails.push(format!(
                    "{}{}@{}",
                    first_name,
                    last_name.chars().next().unwrap(),
                    df.domain
                ));
                emails.push(format!(
                    "{}{}@{}",
                    first_name.chars().next().unwrap(),
                    last_name,
                    df.domain
                ));
            }
        }
    }

    emails
}

async fn filter_verified_emails(sentinel: web::Data<Sentinel>, emails: Vec<String>) -> Vec<String> {
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
    use crate::routes::lead_route::{
        construct_emails, extract_founder_names, get_domain_from_url, DomainFounderQualified,
        FounderElement, FounderTagCandidate,
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
        let candidates = vec![FounderTagCandidate {
            elements: vec![
                FounderElement::Span("LinkedIn Â· Dan Go".to_string()),
                FounderElement::Span("LinkedIn Â· Dan Go".to_string()),
                FounderElement::Span("LinkedIn Â· HÃ©lÃ¨ne de Troostembergh".to_string()),
                FounderElement::Span("LinkedIn Â· Samina Qureshi, RDN LD".to_string()),
                FounderElement::Span("LinkedIn Â· Wondercise Technology Corp.".to_string()),
                FounderElement::Span("LinkedIn Â· Dr Veer Pushpak Gupta".to_string()),
                FounderElement::Span("LinkedIn Â· Hasnain Sajjad".to_string()),
                FounderElement::Span("LinkedIn Â· Deepak L. Bhatt, MD, MPH, MBA".to_string()),
                FounderElement::Span("LinkedIn Â· Dr. Ronald Klatz, MD, DO".to_string()),
                FounderElement::Span("LinkedIn Â· WellTheory".to_string()),
                FounderElement::Span("LinkedIn Â· WellTheory".to_string()),
                FounderElement::Span("LinkedIn Â· West Shell III".to_string()),
                FounderElement::Span("LinkedIn Â· Cathy Cassata".to_string()),
                FounderElement::Span("LinkedIn Â· Shravan Verma".to_string()),
                FounderElement::Span("LinkedIn Â· anwar khan".to_string()),
                FounderElement::Span("LinkedIn Â· Christopher Dean".to_string()),
                FounderElement::Span("LinkedIn India".to_string()),
                FounderElement::Span("LinkedIn".to_string()),
                FounderElement::H3("Dan Go's Post".to_string()),
                FounderElement::H3("Eric Chuang on LinkedIn: Putting up the sign!".to_string()),
                FounderElement::H3("Dan Buettner's Post".to_string()),
                FounderElement::H3("Sarah Garone's Post".to_string()),
                FounderElement::H3(
                    "HÃ©lÃ¨ne de Troostembergh - Truly inspiring Tanguy Goretti".to_string(),
                ),
                FounderElement::H3("Samina Qureshi, RDN LD's Post".to_string()),
                FounderElement::H3("Tanguy Goretti's Post".to_string()),
                FounderElement::H3("Wondercise Technology Corp.".to_string()),
                FounderElement::H3("Dr. Gwilym Roddick's Post".to_string()),
                FounderElement::H3(
                    "Honor Whiteman - Senior Editorial Director - RVO Health".to_string(),
                ),
                FounderElement::H3(
                    "Tim Snaith - Newsletter Editor II - Medical News Today".to_string(),
                ),
                FounderElement::H3("Hasnain Sajjad on LinkedIn: #al".to_string()),
                FounderElement::H3(
                    "Dr Veer Pushpak Gupta - nhs #healthcare #unitedkingdom".to_string(),
                ),
                FounderElement::H3("Beth Frates, MD's Post".to_string()),
                FounderElement::H3("Deepak L. Bhatt, MD, MPH, MBA's Post".to_string()),
                FounderElement::H3("Dr. Ronald Klatz, MD, DO's Post".to_string()),
                FounderElement::H3("WellTheory".to_string()),
                FounderElement::H3("Uma Naidoo, MD".to_string()),
                FounderElement::H3("Dr William Bird MBE's Post".to_string()),
                FounderElement::H3("Georgette Smart - CEO E*HealthLine".to_string()),
                FounderElement::H3("David Kopp's Post".to_string()),
                FounderElement::H3(
                    "West Shell III - GOES (Global Outdoor Emergency Support)".to_string(),
                ),
                FounderElement::H3(
                    "Cathy Cassata - Freelance Writer - Healthline Networks, Inc.".to_string(),
                ),
                FounderElement::H3("Healthline Media".to_string()),
                FounderElement::H3("Health Line - Healthline Team Member".to_string()),
                FounderElement::H3("David Mills - Associate editor - healthline.com".to_string()),
                FounderElement::H3("Kevin Yoshiyama - Healthline Media".to_string()),
                FounderElement::H3("Cortland Dahl's Post".to_string()),
                FounderElement::H3("Kelsey Costa, MS, RDN's Post".to_string()),
                FounderElement::H3("babulal parashar - great innovation".to_string()),
                FounderElement::H3("Shravan Verma - Manager - PANI".to_string()),
                FounderElement::H3("anwar khan's Post".to_string()),
                FounderElement::H3(
                    "Christopher Dean - Sculptor Marble dreaming. collaborator ...".to_string(),
                ),
                FounderElement::H3("Manish Ambast's Post".to_string()),
                FounderElement::H3("Mark Balderman Highlove - Installation Specialist".to_string()),
                FounderElement::H3("100+ \"Partho Roy\" profiles".to_string()),
                FounderElement::H3(
                    "James Weisz on LinkedIn: #website #developer #film".to_string(),
                ),
                FounderElement::H3(
                    "Ravindra Prakash - Plant Manager - Shree Dhanwantri ...".to_string(),
                ),
                FounderElement::H3("Traditional Medicinals".to_string()),
                FounderElement::H3("Caitlin Landesberg on LinkedIn: Home".to_string()),
                FounderElement::H3("Traditional Medicinals".to_string()),
                FounderElement::H3("Joe Stanziano's Post".to_string()),
                FounderElement::H3("Traditional Medicinals | à¦²à¦¿à¦‚à¦•à¦¡à¦‡à¦¨".to_string()),
                FounderElement::H3("Kathy Avilla - Traditional Medicinals, Inc.".to_string()),
                FounderElement::H3("Ben Hindman's Post - sxsw".to_string()),
                FounderElement::H3("David Templeton - COMMUNITY ACTION OF NAPA VALLEY".to_string()),
            ],
            domain: "verywellfit.com".to_string(),
        }];

        let expected = vec![DomainFounderQualified {
            names: vec![
                "Dan Go".to_string(),
                "HÃ©lÃ¨ne de Troostembergh".to_string(),
                "Samina Qureshi".to_string(),
                "Wondercise Technology Corp.".to_string(),
                "Veer Pushpak Gupta".to_string(),
                "Hasnain Sajjad".to_string(),
                "Deepak L. Bhatt".to_string(),
                "Ronald Klatz".to_string(),
                "WellTheory".to_string(),
                "West Shell III".to_string(),
                "Cathy Cassata".to_string(),
                "Shravan Verma".to_string(),
                "anwar khan".to_string(),
                "Christopher Dean".to_string(),
            ],
            domain: "verywellfit.com".to_string(),
        }];

        let results = extract_founder_names(candidates);
        assert_eq!(results, expected)
    }

    #[test]
    fn construct_emails_valid() {
        let domain_founders = vec![DomainFounderQualified {
            names: vec![
                "Dan Go".to_string(),
                "HÃ©lÃ¨ne de Troostembergh".to_string(),
                "Samina Qureshi".to_string(),
                "Wondercise Technology Corp.".to_string(),
                "Veer Pushpak Gupta".to_string(),
                "Deepak L. Bhatt".to_string(),
                "WellTheory".to_string(),
                "West Shell III".to_string(),
            ],
            domain: "verywellfit.com".to_string(),
        }];

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

        let results = construct_emails(domain_founders);
        assert_eq!(results, expected)
    }
}
