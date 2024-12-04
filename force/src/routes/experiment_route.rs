use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};

use crate::services::OpenaiClient;

#[derive(Deserialize)]
struct NicheQuery {
    niche: String,
}

#[get("/gpt")]
async fn get_gpt_results(
    body: web::Query<NicheQuery>,
    openai_client: web::Data<OpenaiClient>,
) -> HttpResponse {
    let products = openai_client
        .get_boolean_searches_from_niche(&body.niche)
        .await;

    match products {
        Ok(products) => HttpResponse::Ok().json(products),
        Err(e) => HttpResponse::Ok().body(format!("Got error: {}", e)),
    }
}

#[get("/multiple-browsers")]
async fn open_multiple_browsers() -> HttpResponse {
    let mut caps = DesiredCapabilities::chrome();
    let proxy = Proxy::Manual {
        ftp_proxy: None,
        http_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
        ssl_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
        socks_proxy: None,
        socks_version: None,
        socks_username: None,
        socks_password: None,
        no_proxy: None,
    };
    caps.set_proxy(proxy).unwrap();

    let mut browsers: Vec<WebDriver> = vec![];
    for _ in 0..5 {
        let driver = WebDriver::new("http://localhost:62510", caps.clone())
            .await
            .unwrap();
        driver.goto("https://www.rust-lang.org").await.unwrap();
        browsers.push(driver);
    }

    for ele in browsers {
        ele.quit().await.unwrap();
    }

    HttpResponse::Ok().body("Opened multiple browsers")
}
