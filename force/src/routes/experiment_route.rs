use actix_web::{get, web, HttpResponse};
use itertools::Itertools;
use rand::seq::SliceRandom;
use serde::Deserialize;
use sqlx::PgPool;
use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};
use uuid::Uuid;

use crate::{
    dal::lead_db::ElementType,
    services::{Droid, OpenaiClient, Sentinel},
};

use super::lead_route;

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
    let drivers = droid.drivers.lock().await;
    let driver = drivers.choose(&mut rand::thread_rng()).unwrap();
    driver.goto(url).await.unwrap();

    HttpResponse::Ok().body("Ok")
}

#[get("/verify-emails")]
async fn verify_emails(sentinel: web::Data<Sentinel>) -> HttpResponse {
    let emails: Vec<String> = vec![
        // "dan@verywellfit.com".to_string(),
        // "go@verywellfit.com".to_string(),
        // "dango@verywellfit.com".to_string(),
        // "dan.go@verywellfit.com".to_string(),
        // "dang@verywellfit.com".to_string(),
        // "dgo@verywellfit.com".to_string(),
        // "samina@verywellfit.com".to_string(),
        // "qureshi@verywellfit.com".to_string(),
        // "saminaqureshi@verywellfit.com".to_string(),
        // "samina.qureshi@verywellfit.com".to_string(),
        // "saminaq@verywellfit.com".to_string(),
        // "squreshi@verywellfit.com".to_string(),
        // "suleman@mazlo.com".to_string(),
        "sulemanmahmood99@gmail.com".to_string(),
        "sulemanmahmood9988347@gmail.com".to_string(),
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
    let drivers = droid.drivers.lock().await;
    let driver = drivers.choose(&mut rand::thread_rng()).unwrap();
    driver
        .goto("https://www.whatismybrowser.com/detect/what-is-my-user-agent/")
        .await
        .unwrap();

    HttpResponse::Ok().body("Ok!")
}

#[get("/check-ip-address")]
async fn check_ip_address(droid: web::Data<Droid>) -> HttpResponse {
    let drivers = droid.drivers.lock().await;
    for _ in 0..1 {
        let driver = drivers.choose(&mut rand::thread_rng()).unwrap();
        driver.goto("https://whatismyipaddress.com/").await.unwrap();
    }

    HttpResponse::Ok().body("Ok!")
}

#[derive(Deserialize)]
struct AppScriptQuery {
    key: String,
}
#[get("/fake-emails")]
async fn get_fake_emails(query: web::Query<AppScriptQuery>) -> HttpResponse {
    if query.key != "smmac-scraper-sandbox-api-key" {
        return HttpResponse::Ok().json(["Invlid Api"]);
    }

    let emails = [
        "dan@verywellfit.com",
        "go@verywellfit.com",
        "dango@verywellfit.com",
        "dan.go@verywellfit.com",
        "dang@verywellfit.com",
        "dgo@verywellfit.com",
        "samina@verywellfit.com",
        "qureshi@verywellfit.com",
        "saminaqureshi@verywellfit.com",
        "samina.qureshi@verywellfit.com",
        "saminaq@verywellfit.com",
        "squreshi@verywellfit.com",
    ];
    HttpResponse::Ok().json(emails)
}

#[get("/re-calculate-domains")]
async fn extract_domain_from_candidate_url(pool: web::Data<PgPool>) -> HttpResponse {
    let candidate_urls = sqlx::query_scalar!("select domain_candidate_url from domain")
        .fetch_all(pool.as_ref())
        .await
        .unwrap();

    let domains: Vec<Option<String>> = candidate_urls
        .iter()
        .map(|url| lead_route::get_domain_from_url(url))
        .collect();

    let founder_search_urls: Vec<Option<String>> = domains
        .clone()
        .into_iter()
        .map(|dom| dom.map(lead_route::build_founder_seach_url))
        .collect();

    for ((url, dom), new_url) in candidate_urls
        .into_iter()
        .zip(domains.into_iter())
        .zip(founder_search_urls.into_iter())
    {
        sqlx::query!(
            r#"
            update domain set
                domain = $2,
                founder_search_url = $3
            where
                domain_candidate_url = $1
            "#,
            url,
            dom,
            new_url,
        )
        .execute(pool.as_ref())
        .await
        .unwrap();
    }

    HttpResponse::Ok().body("Done!")
}

struct element_with_id {
    fe: lead_route::FounderElement,
    id: Uuid,
}

#[get("/re-calculate-founder-names")]
async fn recalculate_founder_names(pool: web::Data<PgPool>) -> HttpResponse {
    let elements: Vec<element_with_id> = sqlx::query!(
        r#"
        select
            id,
            element_content,
            element_type as "element_type: ElementType"
        from
            founder
        "#,
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap()
    .into_iter()
    .map(|r| match r.element_type {
        ElementType::Span => element_with_id {
            fe: lead_route::FounderElement::Span(r.element_content),
            id: r.id,
        },
        ElementType::HThree => element_with_id {
            fe: lead_route::FounderElement::H3(r.element_content),
            id: r.id,
        },
    })
    .collect();
    let founders = lead_route::FounderTagCandidate {
        elements: elements.iter().map(|e| e.fe.clone()).collect(),
        domain: "random.domain".to_string(),
    };

    let founder_names = lead_route::extract_founder_names(founders);

    for (ele, name) in elements.into_iter().zip(founder_names.into_iter()) {
        sqlx::query!(
            r#"
            update founder set
                founder_name = $2
            where
                id = $1
            "#,
            ele.id,
            name,
        )
        .execute(pool.as_ref())
        .await
        .unwrap();
    }

    HttpResponse::Ok().body("Done!")
}

#[get("/valid-founder-names")]
async fn get_valid_founder_names(pool: web::Data<PgPool>) -> HttpResponse {
    let elements = sqlx::query_scalar!(
        r#"
        select
            founder_name
        from
            founder
        where
            founder_name is not null
        "#,
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    let elements: Vec<String> = elements.into_iter().flatten().collect();
    let elements: Vec<String> = elements
        .into_iter()
        .filter(|name| name.split(" ").collect_vec().len() == 2)
        .collect();
    let elements: Vec<String> = elements.into_iter().unique().collect();

    HttpResponse::Ok().json(elements)
}

#[derive(Deserialize)]
struct VerifyEmailQuery {
    email: String,
}

#[get("/verify-email")]
async fn verify_email(
    query: web::Query<VerifyEmailQuery>,
    sentinel: web::Data<Sentinel>,
) -> HttpResponse {
    let email_verified = sentinel.verfiy_email(query.email.clone()).await;

    HttpResponse::Ok().body(format!(
        "Email {:?} was verfied? {}",
        query.email, email_verified
    ))
}
