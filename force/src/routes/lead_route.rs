use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use thirtyfour::{error::WebDriverError, By, WebDriver};

use crate::services::{Droid, OpenaiClient};

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

    let urls = get_urls_from_google_searches(&droid.driver, products).await;

    match urls {
        Ok(urls) => log::info!("Got urls: {:?}", urls),
        Err(e) => log::error!("Error: {}", e),
    }

    HttpResponse::Ok().body("Works!")
}

async fn get_urls_from_google_searches(
    driver: &WebDriver,
    products: Vec<String>,
) -> Result<Vec<String>, WebDriverError> {
    let search_urls: Vec<String> = products
        .iter()
        .map(|st| build_seach_url(st.to_string()))
        .collect();

    let mut urls: Vec<String> = vec![];
    let mut next_search_urls: Vec<String> = vec![];

    for url in search_urls.iter() {
        driver.goto(url).await?;

        // TODO: Check and combine with the below selector
        if driver.find(By::XPath("//a")).await.is_err() {
            continue;
        }

        for a_tag in driver.find_all(By::XPath("//a")).await? {
            let href_attribute = a_tag.attr("href").await?;
            if let Some(href) = href_attribute {
                log::info!("Added url: {}", href);
                urls.push(href);
            }
        }

        if let Ok(next_page_element) = driver.find(By::XPath(r#"//a[@id="pnnext"]"#)).await {
            if let Some(href_attribute) = next_page_element.attr("href").await? {
                next_search_urls.push(href_attribute);
            }
        }
    }

    Ok(urls)
}

fn build_seach_url(product: String) -> String {
    let boolean_query = format!(r#""{}" AND "buy now""#, product);
    format!("https://www.google.com/search?q={}", boolean_query)
}
