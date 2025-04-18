use actix_web::{get, web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dal::lead_db::{EmailReachability, EmailVerifiedStatus},
    domain::email::{FounderDomainEmail, Reachability, VerificationStatus},
    routes::lead_route::build_company_name_search_query,
    services::{
        extract_data_from_google_search_with_reqwest, EmailVerifierSender, GoogleSearchResult,
        GoogleSearchType, ProductQuerySender, Sentinel,
    },
};

#[get("/check-channel-works")]
async fn check_channel_works(domain_scraper_sender: web::Data<ProductQuerySender>) -> HttpResponse {
    let domain_scraper_sender = domain_scraper_sender.sender.clone();
    ["pro 1", "pro 2", "pro 999"].iter().for_each(|q| {
        match domain_scraper_sender.send(q.to_string()) {
            Ok(_) => {}
            Err(e) => log::error!("Found error while sending: {:?}", e),
        }
    });

    HttpResponse::Ok().body("Done")
}

struct EmailRow {
    email_address: String,
    verified_status: EmailVerifiedStatus,
    reachability: EmailReachability,
}

#[get("/migrate")]
async fn migrate(pool: web::Data<PgPool>) -> HttpResponse {
    let rows = sqlx::query!(r"select * from product")
        .fetch_all(pool.as_ref())
        .await
        .unwrap();

    for r in rows {
        if let Err(e) = sqlx::query!(
            r"
            insert into niche
                (user_niche, gippity_prompt, generated_product)
            values
                ($1, $2, $3)
            ",
            r.niche,
            "before migration prompt",
            r.product
        )
        .execute(pool.as_ref())
        .await
        {
            log::error!(
                "Error inserting into niche table from product table: {:?}",
                e
            );
        }
    }

    let rows = sqlx::query_as!(
        EmailRow,
        r#"select
            email_address,
            verified_status as "verified_status: EmailVerifiedStatus",
            reachability as "reachability: EmailReachability"
        from
            email_old
        "#
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    let total_rows = rows.len();
    let founder_names: Vec<String> = (0..total_rows)
        .map(|_| "before-migration-founder-name".to_string())
        .collect();
    let domains: Vec<String> = (0..total_rows)
        .map(|_| "before-migration-domain".to_string())
        .collect();

    let mut email_addresses = Vec::new();
    let mut statuses = Vec::new();
    let mut reaches = Vec::new();

    for r in rows {
        let status: VerificationStatus = r.verified_status.into();
        let reach: Reachability = r.reachability.into();

        email_addresses.push(r.email_address);
        statuses.push(status);
        reaches.push(reach);
    }

    if let Err(e) = sqlx::query!(
        r"
        insert into email
            (email_address, verification_status, reachability, founder_name, domain)
        select * from unnest (
            $1::text[],
            $2::VerificationStatus[],
            $3::Reachability[],
            $4::text[],
            $5::text[]
        )
        ",
        &email_addresses,
        statuses as Vec<VerificationStatus>,
        reaches as Vec<Reachability>,
        &founder_names,
        &domains,
    )
    .execute(pool.as_ref())
    .await
    {
        log::error!("Error inserting into new email table from old: {:?}", e);
    }

    HttpResponse::Ok().body("Done")
}

#[derive(Serialize, Deserialize)]
struct SSPayLoadNull {
    id: Option<i64>,
    name: Option<String>,
    primaryCategoryId: Option<i32>,
    primaryCategory: Option<String>,
    primarySubCategory: Option<String>,
    businessName: Option<String>,
    amazonSellerId: Option<String>,
    estimateSales: Option<f32>,
    avgPrice: Option<f32>,
    percentFba: Option<f32>,
    numberReviewsLifetime: Option<i32>,
    numberReviews30Days: Option<i32>,
    numberWinningBrands: Option<i32>,
    numberAsins: Option<i32>,
    numberTopAsins: Option<i32>,
    street: Option<String>,
    city: Option<String>,
    state: Option<String>,
    country: Option<String>,
    zipCode: Option<String>,
    numBrands1000: Option<i32>,
    moMGrowth: Option<f32>,
    moMGrowthCount: Option<i32>,
    startedSellingDate: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SSPayLoad {
    id: i64,
    name: String,
    primaryCategoryId: i32,
    primaryCategory: String,
    primarySubCategory: String,
    businessName: String,
    amazonSellerId: String,
    estimateSales: f32,
    avgPrice: f32,
    percentFba: f32,
    numberReviewsLifetime: i32,
    numberReviews30Days: i32,
    numberWinningBrands: i32,
    numberAsins: i32,
    numberTopAsins: i32,
    street: String,
    city: String,
    state: String,
    country: String,
    zipCode: String,
    numBrands1000: i32,
    moMGrowth: f32,
    moMGrowthCount: i32,
    startedSellingDate: String,
}

impl From<SSPayLoadNull> for SSPayLoad {
    fn from(value: SSPayLoadNull) -> Self {
        Self {
            id: value.id.unwrap_or(786786),
            name: value.name.unwrap_or("".to_string()),
            primaryCategoryId: value.primaryCategoryId.unwrap_or(786786),
            primaryCategory: value.primaryCategory.unwrap_or("".to_string()),
            primarySubCategory: value.primarySubCategory.unwrap_or("".to_string()),
            businessName: value.businessName.unwrap_or("".to_string()),
            amazonSellerId: value.amazonSellerId.unwrap_or("".to_string()),
            estimateSales: value.estimateSales.unwrap_or(786786.0),
            avgPrice: value.avgPrice.unwrap_or(786786.0),
            percentFba: value.percentFba.unwrap_or(786786.0),
            numberReviewsLifetime: value.numberReviewsLifetime.unwrap_or(786786),
            numberReviews30Days: value.numberReviews30Days.unwrap_or(786786),
            numberWinningBrands: value.numberWinningBrands.unwrap_or(786786),
            numberAsins: value.numberAsins.unwrap_or(786786),
            numberTopAsins: value.numberTopAsins.unwrap_or(786786),
            street: value.street.unwrap_or("".to_string()),
            city: value.city.unwrap_or("".to_string()),
            state: value.state.unwrap_or("".to_string()),
            country: value.country.unwrap_or("".to_string()),
            zipCode: value.zipCode.unwrap_or("".to_string()),
            numBrands1000: value.numBrands1000.unwrap_or(786786),
            moMGrowth: value.moMGrowth.unwrap_or(786786.0),
            moMGrowthCount: value.moMGrowthCount.unwrap_or(786786),
            startedSellingDate: value.startedSellingDate.unwrap_or("".to_string()),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SmartScoutResp {
    payload: Vec<SSPayLoadNull>,
}

#[get("/scrape-smart-scout")]
async fn scrape_smart_scout(pool: web::Data<PgPool>) -> HttpResponse {
    let url = "https://smartscoutapi-east.azurewebsites.net/api/sellers/search";

    let bearer_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJodHRwOi8vc2NoZW1hcy54bWxzb2FwLm9yZy93cy8yMDA1LzA1L2lkZW50aXR5L2NsYWltcy9uYW1lIjoiNTU1ODMiLCJodHRwOi8vc2NoZW1hcy54bWxzb2FwLm9yZy93cy8yMDA1LzA1L2lkZW50aXR5L2NsYWltcy9lbWFpbGFkZHJlc3MiOiJ0YWxoYS5sdW1zQGdtYWlsLmNvbSIsImV4cCI6MTczNTgxMTA3MiwiaXNzIjoiQkIiLCJhdWQiOiJodHRwczovL2xvY2FsaG9zdDo1MDAxIn0.VbTXWgkef2PoZ7wJDbvbyEHZhhzIQDe8keb2qORsmIw";

    let client = Client::new();

    // INFO: Total results: 2007346
    let mut start_index = 0;
    let mut end_index = 10000;

    for _ in 0..200 {
        let json_body = json!({
            "loadDefaultData": false,
            "filter": {},
            "pageFilter": {
                "startRow": start_index,
                "endRow": end_index,
                "includeTotalRowCount": false,
                "sortModel": [],
                "fields": [
                    "name", "amazonSellerId", "primaryCategoryId", "primarySubCategory",
                    "estimateSales", "percentFba", "numberWinningBrands", "numberAsins",
                    "numberTopAsins", "state", "country", "businessName", "numBrands1000",
                    "moMGrowth", "moMGrowthCount", "startedSellingDate", "amazonSellerId", "note"
                ]
            }
        });

        let trace_id = Uuid::new_v4().simple(); // Generate a unique trace ID
        let span_id = Uuid::new_v4().simple(); // Generate a unique span ID
        let req_id = format!("|{}.{}", trace_id, span_id);
        let tp_id = format!("00-{}-{}-01", trace_id, span_id);

        log::info!("Request-Id: {}", req_id);
        log::info!("Traceparent-Id: {}", tp_id);
        log::info!(
            "Making a req with start index: {} and end index: {}",
            start_index,
            end_index
        );

        let response = client
        .post(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:133.0) Gecko/20100101 Firefox/133.0",
        )
        .header("Accept", "text/plain")
        .header("Accept-Language", "en-US,en;q=0.5")
        .header("Content-Type", "application/json-patch+json")
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("X-SmartScout-Marketplace", "US")
        .header("Request-Id", req_id)
        .header("traceparent", tp_id)
        .header("Origin", "https://app.smartscout.com")
        .header("Connection", "keep-alive")
        .header("Referer", "https://app.smartscout.com/")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "cross-site")
        .json(&json_body)
        .send()
        .await
        .unwrap();

        log::info!("Status: {}", response.status());
        let response_json_result = response.json::<SmartScoutResp>().await;
        if let Err(e) = response_json_result {
            log::error!(
                "Skipping start index: {}, end index: {}",
                start_index,
                end_index
            );
            log::error!("Couldn't parse to json: {:?}", e);
            continue;
        }
        let response_json = response_json_result.unwrap();

        let mut id = Vec::new();
        let mut name = Vec::new();
        let mut primaryCategoryId = Vec::new();
        let mut primaryCategory = Vec::new();
        let mut primarySubCategory = Vec::new();
        let mut businessName = Vec::new();
        let mut amazonSellerId = Vec::new();
        let mut estimateSales = Vec::new();
        let mut avgPrice = Vec::new();
        let mut percentFba = Vec::new();
        let mut numberReviewsLifetime = Vec::new();
        let mut numberReviews30Days = Vec::new();
        let mut numberWinningBrands = Vec::new();
        let mut numberAsins = Vec::new();
        let mut numberTopAsins = Vec::new();
        let mut street = Vec::new();
        let mut city = Vec::new();
        let mut state = Vec::new();
        let mut country = Vec::new();
        let mut zipCode = Vec::new();
        let mut numBrands1000 = Vec::new();
        let mut moMGrowth = Vec::new();
        let mut moMGrowthCount = Vec::new();
        let mut startedSellingDate = Vec::new();

        let data: Vec<SSPayLoad> = response_json
            .payload
            .into_iter()
            .map(|r| r.into())
            .collect();

        for r in data {
            id.push(r.id);
            name.push(r.name);
            primaryCategoryId.push(r.primaryCategoryId);
            primaryCategory.push(r.primaryCategory);
            primarySubCategory.push(r.primarySubCategory);
            businessName.push(r.businessName);
            amazonSellerId.push(r.amazonSellerId);
            estimateSales.push(r.estimateSales);
            avgPrice.push(r.avgPrice);
            percentFba.push(r.percentFba);
            numberReviewsLifetime.push(r.numberReviewsLifetime);
            numberReviews30Days.push(r.numberReviews30Days);
            numberWinningBrands.push(r.numberWinningBrands);
            numberAsins.push(r.numberAsins);
            numberTopAsins.push(r.numberTopAsins);
            street.push(r.street);
            city.push(r.city);
            state.push(r.state);
            country.push(r.country);
            zipCode.push(r.zipCode);
            numBrands1000.push(r.numBrands1000);
            moMGrowth.push(r.moMGrowth);
            moMGrowthCount.push(r.moMGrowthCount);
            startedSellingDate.push(r.startedSellingDate);
        }

        if let Err(e) = sqlx::query!(
        r"
        insert into smart_scout
            (public_id, name, primaryCategoryId, primaryCategory, primarySubCategory, businessName, amazonSellerId, estimateSales, avgPrice, percentFba, numberReviewsLifetime, numberReviews30Days, numberWinningBrands, numberAsins, numberTopAsins, street, city, state, country, zipCode, numBrands1000, moMGrowth, moMGrowthCount, startedSellingDate)
        select * from unnest (
            $1::bigint[],
            $2::text[],
            $3::int[],
            $4::text[],
            $5::text[],
            $6::text[],
            $7::text[],
            $8::real[],
            $9::real[],
            $10::real[],
            $11::int[],
            $12::int[],
            $13::int[],
            $14::int[],
            $15::int[],
            $16::text[],
            $17::text[],
            $18::text[],
            $19::text[],
            $20::text[],
            $21::int[],
            $22::real[],
            $23::int[],
            $24::text[]
        )
        ",
        &id,
        &name,
        &primaryCategoryId,
        &primaryCategory,
        &primarySubCategory,
        &businessName,
        &amazonSellerId,
        &estimateSales,
        &avgPrice,
        &percentFba,
        &numberReviewsLifetime,
        &numberReviews30Days,
        &numberWinningBrands,
        &numberAsins,
        &numberTopAsins,
        &street,
        &city,
        &state,
        &country,
        &zipCode,
        &numBrands1000,
        &moMGrowth,
        &moMGrowthCount,
        &startedSellingDate
    )
    .execute(pool.as_ref())
    .await
    {
        log::error!("Error inserting into smart scout table: {:?}", e);
    }

        start_index += 10000;
        end_index += 10000;
    }

    HttpResponse::Ok().body("Done")
}

#[get("/verify-emails")]
async fn verify_emails(
    pool: web::Data<PgPool>,
    email_verifier_sender: web::Data<EmailVerifierSender>,
) -> HttpResponse {
    let emails = sqlx::query!(
        r"
        select
            email_address,
            founder_name,
            domain
        from
            email
        where
            verification_status = 'PENDING'
        order by created_at desc
        limit 14000
        "
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    for em in emails {
        email_verifier_sender
            .sender
            .send(FounderDomainEmail {
                founder_name: em.founder_name,
                domain: em.domain,
                email: em.email_address,
            })
            .unwrap();
    }

    HttpResponse::Ok().body("Done!")
}

#[get("/verify-emails-custom")]
async fn verify_emails_custom(
    pool: web::Data<PgPool>,
    email_verifier_sender: web::Data<EmailVerifierSender>,
) -> HttpResponse {
    let emails = sqlx::query!(
        r"
        select
            email_address,
            founder_name,
            domain
        from
            email
        where
            verification_status = 'PENDING' and
            created_at BETWEEN '2025-01-11 12:08:00' AND '2025-01-11 12:38:00'
        "
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    for em in emails {
        email_verifier_sender
            .sender
            .send(FounderDomainEmail {
                founder_name: em.founder_name,
                domain: em.domain,
                email: em.email_address,
            })
            .unwrap();
    }

    HttpResponse::Ok().body("Done!")
}

#[get("/verify-emails-hardcoded")]
async fn verify_emails_hardcoded(sentinel: web::Data<Sentinel>) -> HttpResponse {
    let emails = [
        "bmurphy@summitparkllc.com".to_string(),
        "rhannon@summitparkllc.com".to_string(),
        "jjohnson@summitparkllc.com".to_string(),
        "summitpark@summitparkllc.com".to_string(),
        "ruth.barclay@macmillan.com".to_string(),
        "brian.mcsharry@macmillan.com".to_string(),
        "liamc@zentrallc.com".to_string(),
        "staceyk@zentrallc.com".to_string(),
        "ailun.fu@ailun.com".to_string(),
        "about@pinterest.com".to_string(),
        "brad.gordon@charmast.com".to_string(),
        "john@charmast.com".to_string(),
        "michelangelo@amazon.es".to_string(),
        "xiny@amazon.es".to_string(),
        "kevin.audibert@amazon.fr".to_string(),
        "adrien@amazon.fr".to_string(),
        "xavier@amazon.fr".to_string(),
        "sportsman's.guide@sportsmansguide.com".to_string(),
        "ron@twowaydirect.com".to_string(),
        "christina@twowaydirect.com".to_string(),
    ];

    for em in emails {
        sentinel.verify_email_manual(&em).await;
    }

    HttpResponse::Ok().body("Done!")
}

#[get("/verify-emails-smart-scout")]
async fn verify_emails_smart_scout(
    pool: web::Data<PgPool>,
    email_verifier_sender: web::Data<EmailVerifierSender>,
) -> HttpResponse {
    let emails = sqlx::query!(
        r"
        select
            email_address,
            founder_name,
            domain
        from
            email
        "
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    for em in emails {
        email_verifier_sender
            .sender
            .send(FounderDomainEmail {
                founder_name: em.founder_name,
                domain: em.domain,
                email: em.email_address,
            })
            .unwrap();
    }

    HttpResponse::Ok().body("Done!")
}

#[get("/check-proxy-works")]
async fn check_proxy_works() -> HttpResponse {
    let query = build_company_name_search_query("AnkerDirect");

    let google_search_result =
        extract_data_from_google_search_with_reqwest(query.clone(), GoogleSearchType::CompanyName)
            .await;

    match google_search_result {
        GoogleSearchResult::CompanyNames {
            name_candidates,
            page_source,
        } => {
            log::info!("Company name candidates: {:?}", name_candidates);
            HttpResponse::Ok().body(page_source)
        }
        _ => HttpResponse::Ok().body("Not suitable search result"),
    }
}
