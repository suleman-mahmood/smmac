use actix_web::{get, web, HttpResponse};
use rand::seq::SliceRandom;
use serde::Deserialize;
use sqlx::PgPool;
use thirtyfour::{error::WebDriverError, By, WebDriver};
use url::Url;

use crate::services::{Droid, OpenaiClient};

const DEPTH_GOOGLE_SEACH_PAGES: u8 = 5;

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

    let products = openai_client
        .get_boolean_searches_from_niche(&body.niche)
        .await
        .unwrap();

    let raw_urls = get_urls_from_google_searches(&droid.drivers, products)
        .await
        .unwrap();

    let urls = filter_raw_urls(raw_urls);
    let domains = extract_domains_from_urls(urls);
    // TODO: remove duplicate domains

    let raw_founders = get_founders_from_google_searches(&droid.drivers, domains)
        .await
        .unwrap();

    // TODO: Extract founder names from the tags scraped
    let founders = extract_founder_names(raw_founders);

    HttpResponse::Ok().body(format!("Founders: {:?}", founders))
}

async fn get_urls_from_google_searches(
    drivers: &Vec<WebDriver>,
    products: Vec<String>,
) -> Result<Vec<String>, WebDriverError> {
    /*
     * For each url:
     ** Randomly select one browser from pool
     ** Scrape the link
     * */
    let mut search_urls: Vec<String> = products
        .iter()
        .map(|st| build_seach_url(st.to_string()))
        .collect();

    let mut domain_urls: Vec<String> = vec![];

    for _ in 0..DEPTH_GOOGLE_SEACH_PAGES {
        let mut next_page_urls: Vec<String> = vec![];

        for url in search_urls.iter() {
            let driver = drivers.choose(&mut rand::thread_rng()).unwrap();

            driver.goto(url).await?;

            // Check if no results found
            if driver.find(By::XPath("//a")).await.is_err() {
                log::error!("Found no results on url: {}", url);
                continue;
            }

            for a_tag in driver.find_all(By::XPath("//a")).await? {
                let href_attribute = a_tag.attr("href").await?;
                if let Some(href) = href_attribute {
                    domain_urls.push(href);
                }
            }

            log::info!("Found {} urls", domain_urls.len(),);

            if let Ok(next_page_element) = driver.find(By::XPath(r#"//a[@id="pnnext"]"#)).await {
                if let Some(href_attribute) = next_page_element.attr("href").await? {
                    let next_url = format!("https://www.google.com{}", href_attribute);
                    next_page_urls.push(next_url);
                }
            }
        }

        search_urls = next_page_urls;
    }

    Ok(domain_urls)
}

#[derive(Debug, PartialEq)]
struct FounderTagCandidate {
    h3_tags: Vec<String>,
    span_tags: Vec<String>,
    domain: String,
}

async fn get_founders_from_google_searches(
    drivers: &Vec<WebDriver>,
    domains: Vec<String>,
) -> Result<Vec<FounderTagCandidate>, WebDriverError> {
    // TODO: Add more build search url permutations as needed

    let search_urls: Vec<String> = domains
        .iter()
        .map(|d| build_founder_seach_url(d.to_string()))
        .collect();

    let mut founder_candidate: Vec<FounderTagCandidate> = vec![];

    for (url, domain) in search_urls.iter().zip(domains.iter()) {
        let mut h3_tags = vec![];
        let mut span_tags = vec![];
        let driver = drivers.choose(&mut rand::thread_rng()).unwrap();

        driver.goto(url).await?;

        // Check if no results found
        if driver.find(By::XPath("//h3")).await.is_err() {
            log::error!("Found no results on url: {}", url);
            continue;
        }

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
            "Found {} h3_tags, {} span_tags",
            h3_tags.len(),
            span_tags.len()
        );

        founder_candidate.push(FounderTagCandidate {
            h3_tags,
            span_tags,
            domain: domain.to_string(),
        });
    }

    Ok(founder_candidate)
}

fn build_seach_url(product: String) -> String {
    let boolean_query = format!(r#""{}" AND "buy now""#, product);
    format!("https://www.google.com/search?q={}", boolean_query)
}

fn build_founder_seach_url(domain: String) -> String {
    let boolean_query = format!(r#"site:linkedin.com "{}" AND "founder""#, domain);
    format!("https://www.google.com/search?q={}", boolean_query)
}

fn filter_raw_urls(urls: Vec<String>) -> Vec<String> {
    urls.iter()
        .filter(|u| match Url::parse(u) {
            Ok(parsed_url) => match parsed_url.host_str() {
                Some("support.google.com") => false,
                Some("www.google.com") => false,
                Some("accounts.google.com") => false,
                Some("policies.google.com") => false,
                Some("www.amazon.com") => false,
                Some("") => false,
                None => false,
                Some(any_host) => !any_host.contains("google.com"),
            },
            Err(_) => false,
        })
        .map(|u| u.to_string())
        .collect()
}

fn extract_domains_from_urls(urls: Vec<String>) -> Vec<String> {
    urls.iter()
        .map(|u| {
            let host = Url::parse(u).unwrap().host_str().unwrap().to_string();
            match host.strip_prefix("www.") {
                Some(h) => h.to_string(),
                None => host.to_string(),
            }
        })
        .collect()
}

fn extract_founder_names(founder_candidates: Vec<FounderTagCandidate>) -> Vec<FounderTagCandidate> {
    founder_candidates
        .iter()
        .map(|fc| {
            let h3_tags = fc
                .h3_tags
                .iter()
                .map(|t| {
                    todo!();
                })
                .collect();
            let span_tags = fc
                .span_tags
                .iter()
                .filter_map(|t| match t.strip_prefix("LinkedIn Â· ") {
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
                })
                .collect();

            FounderTagCandidate {
                h3_tags,
                span_tags,
                domain: fc.domain.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::routes::lead_route::{
        extract_domains_from_urls, extract_founder_names, filter_raw_urls, FounderTagCandidate,
    };

    #[test]
    fn filter_raw_urls_invalid() {
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
        let raw_urls = raw_urls.iter().map(|u| u.to_string()).collect();
        let results = filter_raw_urls(raw_urls);

        assert!(results.is_empty())
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
        let raw_urls: Vec<String> = raw_urls.iter().map(|u| u.to_string()).collect();
        let results = filter_raw_urls(raw_urls.clone());

        assert_eq!(results, raw_urls)
    }

    #[test]
    fn extract_domains_valid() {
        let urls = [
            "https://www.znaturalfoods.com/products/green-tea-organic",
            "https://dallosell.com/product_detail/organic-green-tea-bag",
            "https://www.verywellfit.com/best-green-teas-5115813#:~:text=Certified%20organic%2C%20non%2DGMO%2C,Kyushu%20Island%20in%20southern%20Japan.",
            "https://www.medicalnewstoday.com/articles/269538#:~:text=Research%20suggests%20it%20is%20safe,or%20interact%20with%20certain%20medications.",
            "https://www.healthline.com/nutrition/top-10-evidence-based-health-benefits-of-green-tea#:~:text=A%202017%20research%20paper%20found,middle%2Daged%20and%20older%20adults.",
            "https://organicindia.com/collections/green-tea?srsltid=AfmBOopzdn4oOzfSwiaITNekbORRUG_MoVF67dULVE9IEHV6zlvZL0Qc",
            "https://www.traditionalmedicinals.com/products/green-tea-matcha?srsltid=AfmBOoqwv1CiL0XV_zNFmIWU1biT3S4xa-7KkOLzgXN4BkSCscGZFXzS",
        ];
        let urls: Vec<String> = urls.iter().map(|u| u.to_string()).collect();
        let results = extract_domains_from_urls(urls);

        assert_eq!(
            results,
            vec![
                "znaturalfoods.com",
                "dallosell.com",
                "verywellfit.com",
                "medicalnewstoday.com",
                "healthline.com",
                "organicindia.com",
                "traditionalmedicinals.com",
            ]
        )
    }

    #[test]
    fn extract_founder_names_valid() {
        let candidates = vec![FounderTagCandidate {
            h3_tags: vec![
                // "Dan Go's Post".to_string(),
                // "Eric Chuang on LinkedIn: Putting up the sign!".to_string(),
                // "Dan Buettner's Post".to_string(),
                // "Sarah Garone's Post".to_string(),
                // "HÃ©lÃ¨ne de Troostembergh - Truly inspiring Tanguy Goretti".to_string(),
                // "Samina Qureshi, RDN LD's Post".to_string(),
                // "Tanguy Goretti's Post".to_string(),
                // "Wondercise Technology Corp.".to_string(),
                // "Dr. Gwilym Roddick's Post".to_string(),
                // "Honor Whiteman - Senior Editorial Director - RVO Health".to_string(),
                // "Tim Snaith - Newsletter Editor II - Medical News Today".to_string(),
                // "Hasnain Sajjad on LinkedIn: #al".to_string(),
                // "Dr Veer Pushpak Gupta - nhs #healthcare #unitedkingdom".to_string(),
                // "Beth Frates, MD's Post".to_string(),
                // "Deepak L. Bhatt, MD, MPH, MBA's Post".to_string(),
                // "Dr. Ronald Klatz, MD, DO's Post".to_string(),
                // "WellTheory".to_string(),
                // "Uma Naidoo, MD".to_string(),
                // "Dr William Bird MBE's Post".to_string(),
                // "Georgette Smart - CEO E*HealthLine".to_string(),
                // "David Kopp's Post".to_string(),
                // "West Shell III - GOES (Global Outdoor Emergency Support)".to_string(),
                // "Cathy Cassata - Freelance Writer - Healthline Networks, Inc.".to_string(),
                // "Healthline Media".to_string(),
                // "Health Line - Healthline Team Member".to_string(),
                // "David Mills - Associate editor - healthline.com".to_string(),
                // "Kevin Yoshiyama - Healthline Media".to_string(),
                // "Cortland Dahl's Post".to_string(),
                // "Kelsey Costa, MS, RDN's Post".to_string(),
                // "babulal parashar - great innovation".to_string(),
                // "Shravan Verma - Manager - PANI".to_string(),
                // "anwar khan's Post".to_string(),
                // "Christopher Dean - Sculptor Marble dreaming. collaborator ...".to_string(),
                // "Manish Ambast's Post".to_string(),
                // "Mark Balderman Highlove - Installation Specialist".to_string(),
                // "100+ \"Partho Roy\" profiles".to_string(),
                // "James Weisz on LinkedIn: #website #developer #film".to_string(),
                // "Ravindra Prakash - Plant Manager - Shree Dhanwantri ...".to_string(),
                // "Traditional Medicinals".to_string(),
                // "Caitlin Landesberg on LinkedIn: Home".to_string(),
                // "Traditional Medicinals".to_string(),
                // "Joe Stanziano's Post".to_string(),
                // "Traditional Medicinals | à¦²à¦¿à¦‚à¦•à¦¡à¦‡à¦¨".to_string(),
                // "Kathy Avilla - Traditional Medicinals, Inc.".to_string(),
                // "Ben Hindman's Post - sxsw".to_string(),
                // "David Templeton - COMMUNITY ACTION OF NAPA VALLEY".to_string(),
            ],
            span_tags: vec![
                "LinkedIn Â· Dan Go".to_string(),
                "LinkedIn Â· HÃ©lÃ¨ne de Troostembergh".to_string(),
                "LinkedIn Â· Samina Qureshi, RDN LD".to_string(),
                "LinkedIn Â· Wondercise Technology Corp.".to_string(),
                "LinkedIn Â· Dr Veer Pushpak Gupta".to_string(),
                "LinkedIn Â· Hasnain Sajjad".to_string(),
                "LinkedIn Â· Deepak L. Bhatt, MD, MPH, MBA".to_string(),
                "LinkedIn Â· Dr. Ronald Klatz, MD, DO".to_string(),
                "LinkedIn Â· WellTheory".to_string(),
                "LinkedIn Â· West Shell III".to_string(),
                "LinkedIn Â· Cathy Cassata".to_string(),
                "LinkedIn Â· Shravan Verma".to_string(),
                "LinkedIn Â· anwar khan".to_string(),
                "LinkedIn Â· Christopher Dean".to_string(),
                "LinkedIn India".to_string(),
                "LinkedIn".to_string(),
            ],
            domain: "verywellfit.com".to_string(),
        }];

        let expected = vec![FounderTagCandidate {
            h3_tags: vec![
                // "Dan Go".to_string(),
                // "Dan Gods".to_string(),
                // "Dan Godsfj".to_string(),
            ],
            span_tags: vec![
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
}
