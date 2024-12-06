use std::{thread, time::Duration};

use actix_web::{get, web, HttpResponse};
use rand::seq::SliceRandom;
use serde::Deserialize;
use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};

use crate::services::{Droid, OpenaiClient, Sentinel};

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

#[get("/next-search")]
async fn next_search(droid: web::Data<Droid>) -> HttpResponse {
    let url = "/search?q=Organic+Tea+Tree+Hand+Cream+AND+buy+now&sca_esv=293021c43ebdc58d&sxsrf=ADLYWILkv5MxD0NCkSm12R2B4ekP8njTwA:1733322479740&ei=72ZQZ8HuLKmX4-EPo6y06Q4&start=10&sa=N&sstk=ATObxK6-74Hr_V35WxL_uX774bmYXqXFtGbrolRqun70NhRsGGFP9SyzYYM8dQQuqwfJm8YX9ldgm7sHk5iWYzQAGbfa-eofR4tDig&ved=2ahUKEwiBor61qY6KAxWpyzgGHSMWLe0Q8NMDegQIDBAW";

    let url = format!("https://www.google.com{}", url);
    let driver = droid.drivers.choose(&mut rand::thread_rng()).unwrap();
    driver.goto(url).await.unwrap();

    HttpResponse::Ok().body("Ok")
}

#[get("/verify-emails")]
async fn verify_emails(sentinel: web::Data<Sentinel>) -> HttpResponse {
    let emails: Vec<String> = vec![
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
        "suleman@mazlo.com".to_string(),
    ];
    let mut verified_emails: Vec<String> = vec![];

    for em in emails {
        if sentinel.verfiy_email(em.clone()).await {
            verified_emails.push(em);
        }
    }

    HttpResponse::Ok().json(verified_emails)
}

#[get("/check-user-agent")]
async fn check_user_agent(droid: web::Data<Droid>) -> HttpResponse {
    let driver = droid.drivers.choose(&mut rand::thread_rng()).unwrap();
    driver
        .goto("https://www.whatismybrowser.com/detect/what-is-my-user-agent/")
        .await
        .unwrap();

    HttpResponse::Ok().body("Ok!")
}

#[get("/check-ip-address")]
async fn check_ip_address(droid: web::Data<Droid>) -> HttpResponse {
    for _ in 0..1 {
        let driver = droid.drivers.choose(&mut rand::thread_rng()).unwrap();
        driver.goto("https://whatismyipaddress.com/").await.unwrap();
    }

    HttpResponse::Ok().body("Ok!")
}
