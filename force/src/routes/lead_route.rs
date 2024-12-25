use actix_web::{get, web, HttpResponse};
use check_if_email_exists::Reachable;
use serde::Deserialize;
use sqlx::{Acquire, PgPool};
use tokio::task::JoinSet;

use crate::{
    dal::{
        config_db, google_webpage_db, html_tag_db,
        lead_db::{self, EmailReachability, EmailVerifiedStatus},
        niche_db,
    },
    domain::{
        email::construct_email_permutations,
        google_webpage::{DataExtractionIntent, GoogleWebPage},
        html_tag::{extract_domain, extract_founder_name, HtmlTag},
    },
    services::{
        extract_data_from_google_search_with_reqwest, save_product_search_queries,
        GoogleSearchResult, GoogleSearchType, OpenaiClient, ProductQuerySender, Sentinel,
    },
};

pub const BLACK_LIST_DOMAINS: [&str; 7] = [
    "reddit",
    "youtube",
    "pinterest",
    "amazon",
    "linkedin",
    "github",
    "microsoft",
];

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
    product_query_sender: web::Data<ProductQuerySender>,
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

    let niche_obj = niche_db::get_niche(&pool, &niche).await.unwrap();
    let queries: Vec<String> = niche_obj
        .generated_products
        .into_iter()
        .map(|p| build_seach_query(&p))
        .collect();
    let product_queries = google_webpage_db::filter_unscraped_product_queries(&pool, queries)
        .await
        .unwrap();

    let product_query_sender = product_query_sender.sender.clone();
    product_queries
        .iter()
        .for_each(|q| product_query_sender.send(q.to_string()).unwrap());

    let page_depth = config_db::get_google_search_page_depth(&pool)
        .await
        .unwrap_or(Some("1".to_string()))
        .unwrap_or("1".to_string())
        .parse()
        .unwrap_or(1);

    save_urls_from_google_searche_batch(&pool, product_queries, page_depth).await;

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

async fn save_urls_from_google_searche_batch(
    pool: &PgPool,
    search_queries: Vec<String>,
    page_depth: u8,
) {
    const BATCH_SIZE: usize = 1000;

    for batch in search_queries.chunks(BATCH_SIZE) {
        let mut set = JoinSet::new();

        for query in batch {
            let query = query.clone();

            set.spawn(async move {
                // Fetch domain urls for url, if exist don't search

                let mut current_url = None;
                let mut domain_urls_list: Vec<HtmlTag> = vec![];
                let mut page_source_list: Vec<(String, u8)> = vec![];
                let mut not_found = false;

                for current_page_index in 0..page_depth {
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
                    .map(|tag| extract_domain(tag.clone()))
                    .collect();
                let founder_search_queries: Vec<Option<String>> = domains
                    .clone()
                    .into_iter()
                    .map(|dom| {
                        dom.as_deref()
                            .map(|d| build_founder_seach_queries(d).first().unwrap().to_string())
                    })
                    .collect();

                (
                    domain_urls_list,
                    domains,
                    founder_search_queries,
                    query,
                    not_found,
                    page_source_list,
                )
            });
        }

        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        while let Some(res) = set.join_next().await {
            if let Ok(r) = res {
                for (page_source, page_number) in r.5 {
                    let webpage = GoogleWebPage {
                        search_query: r.3.clone(),
                        page_source,
                        data_extraction_intent: DataExtractionIntent::Domain,
                        page_number,
                        any_result: r.4,
                    };
                    let page_id = google_webpage_db::insert_web_page(con, webpage)
                        .await
                        .unwrap();

                    for tag in r.0.clone() {
                        _ = html_tag_db::insert_html_tag(con, tag, page_id).await;
                    }
                }
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct FounderTagCandidate {
    pub elements: Vec<HtmlTag>, // TODO: Change this to return vec of names
    pub domain: String,
}

pub enum FounderThreadResult {
    Insert(FounderTagCandidate, Vec<Option<String>>, String, String),
    NotFounder(String),
    Ignore,
}

async fn save_founders_from_google_searches_batch(pool: &PgPool, domains: Vec<String>) {
    const BATCH_SIZE: usize = 1000;

    let mut domain_queries = Vec::new();
    for d in domains.iter() {
        let founder_queries = build_founder_seach_queries(d);
        for query in founder_queries {
            domain_queries.push((d.to_string(), query));
        }
    }

    for batch in domain_queries.chunks(BATCH_SIZE) {
        let mut set = JoinSet::new();

        for (domain, query) in batch {
            let domain = domain.clone();
            let query = query.clone();

            set.spawn(async move {
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
                    GoogleSearchResult::Founders(tag_candidate, page_source) => {
                        let founder_names = tag_candidate
                            .elements
                            .iter()
                            .map(|ele| extract_founder_name(ele.clone()))
                            .collect();

                        FounderThreadResult::Insert(
                            tag_candidate,
                            founder_names,
                            domain,
                            page_source,
                        )
                    }
                    GoogleSearchResult::CaptchaBlocked => {
                        log::error!("Returning from captcha blocked on url {}", query);
                        FounderThreadResult::Ignore
                    }
                }
            });
        }

        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        while let Some(res) = set.join_next().await {
            if let Ok(params) = res {
                // Save results to db
                match params {
                    FounderThreadResult::Insert(
                        tag_candidate,
                        founder_names,
                        domain,
                        page_source,
                    ) => {
                        _ = lead_db::insert_founders(tag_candidate, founder_names, &domain, con)
                            .await;
                        _ = lead_db::insert_domain_page_source(&page_source, &domain, con).await;
                    }
                    FounderThreadResult::NotFounder(domain) => {
                        _ = lead_db::insert_domain_no_results(&domain, con).await;
                    }
                    FounderThreadResult::Ignore => (),
                }
            }
        }
    }
}

pub fn build_seach_query(product: &str) -> String {
    product.to_lowercase()
    // format!(r#""{}" AND "buy now""#, product.to_lowercase())
}

pub fn build_founder_seach_queries(domain: &str) -> Vec<String> {
    let titles = ["founder", "ceo", "owner"];
    let domain = domain.to_lowercase();

    titles
        .into_iter()
        .map(|t| format!(r#"site:linkedin.com "{}" AND "{}""#, domain, t))
        .collect()
}

#[derive(Clone)]
pub struct FounderDomain {
    pub founder_name: String,
    pub domain: String,
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

            let emails_db = construct_email_permutations(&fd.founder_name, &fd.domain);
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

async fn verify_emails(pool: &PgPool, sentinel: web::Data<Sentinel>, emails: Vec<String>) {
    const BATCH_SIZE: usize = 10000;

    for batch in emails.chunks(BATCH_SIZE) {
        let mut set = JoinSet::new();

        for em in batch {
            let sentinel = sentinel.clone();
            let em = em.clone();

            set.spawn(async move {
                let reachable = sentinel.get_email_verification_status(&em).await;
                let status = match reachable {
                    Reachable::Safe => EmailVerifiedStatus::Verified,
                    _ => EmailVerifiedStatus::Invalid,
                };
                let reachable: EmailReachability = reachable.into();

                (em, status, reachable)
            });
        }

        let mut pool_con = pool.acquire().await.unwrap();
        let con = pool_con.acquire().await.unwrap();

        while let Some(res) = set.join_next().await {
            if let Ok(r) = res {
                // update in lead db
                _ = lead_db::set_email_verification_reachability(&r.0, r.1, r.2, con).await;
            }
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

    use crate::{
        domain::{
            email::construct_email_permutations,
            html_tag::{extract_domain, HtmlTag},
        },
        routes::lead_route::{extract_founder_name, FounderTagCandidate},
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
            let result = extract_domain(HtmlTag::ATag(url.to_string()));
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
            let result = extract_domain(HtmlTag::ATag(url.to_string()));
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
                HtmlTag::H3Tag("Swati Bhargava - CashKaro.com - LinkedIn".to_string()),
                HtmlTag::H3Tag("Rohan Bhargava - CashKaro.com - LinkedIn".to_string()),
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

        let results: Vec<String> = candidate
            .elements
            .iter()
            .map(|ele| extract_founder_name(ele.clone()))
            .flatten()
            .collect();
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
            let emails = construct_email_permutations(&name, "verywellfit.com");
            results.extend(emails.into_iter().map(|e| e.email));
        }

        assert_eq!(results, expected)
    }
}
