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

    let founders = get_founders_from_google_searches(&droid.drivers, domains)
        .await
        .unwrap();

    // TODO: Extract founder names from the tags scraped

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

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use crate::routes::lead_route::{extract_domains_from_urls, filter_raw_urls};

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
}
