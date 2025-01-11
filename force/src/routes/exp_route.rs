use core::f64;
use std::u16;

use actix_web::{get, web, HttpResponse};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    dal::lead_db::{EmailReachability, EmailVerifiedStatus},
    domain::email::{FounderDomainEmail, Reachability, VerificationStatus},
    services::{EmailVerifierSender, ProductQuerySender, Sentinel},
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

#[get("/verify-emails-hard-coded")]
async fn verify_emails_hard_coded(sentinel: web::Data<Sentinel>) -> HttpResponse {
    let emails = vec![
        "pjariwala@bestchoiceproducts.com".to_string(),
        "schaudhry@bestchoiceproducts.com".to_string(),
        "jason@bestchoiceproducts.com".to_string(),
        "sritzler@bestchoiceproducts.com".to_string(),
        "gyoder@bestchoiceproducts.com".to_string(),
        "spreetail@spreetail.com".to_string(),
        "com@spreetail.com".to_string(),
        "spreetailcom@spreetail.com".to_string(),
        "spreetail.com@spreetail.com".to_string(),
        "spreetailc@spreetail.com".to_string(),
        "scom@spreetail.com".to_string(),
        "thomas@spreetail.com".to_string(),
        "heldridge@spreetail.com".to_string(),
        "thomasheldridge@spreetail.com".to_string(),
        "thomas.heldridge@spreetail.com".to_string(),
        "thomash@spreetail.com".to_string(),
        "theldridge@spreetail.com".to_string(),
        "ecommerce@spreetail.com".to_string(),
        "advertising@spreetail.com".to_string(),
        "ecommerceadvertising@spreetail.com".to_string(),
        "ecommerce.advertising@spreetail.com".to_string(),
        "ecommercea@spreetail.com".to_string(),
        "eadvertising@spreetail.com".to_string(),
        "owen@spreetail.com".to_string(),
        "carr@spreetail.com".to_string(),
        "owencarr@spreetail.com".to_string(),
        "owen.carr@spreetail.com".to_string(),
        "owenc@spreetail.com".to_string(),
        "ocarr@spreetail.com".to_string(),
        "spreetail's@spreetail.com".to_string(),
        "post@spreetail.com".to_string(),
        "spreetail'spost@spreetail.com".to_string(),
        "spreetail's.post@spreetail.com".to_string(),
        "spreetail'sp@spreetail.com".to_string(),
        "spost@spreetail.com".to_string(),
        "aselby@zappos.com".to_string(),
        "mingyang@baylandhealth.com".to_string(),
        "mingyangs@baylandhealth.com".to_string(),
        "mehrdad@baylandhealth.com".to_string(),
        "esmaeilian@baylandhealth.com".to_string(),
        "mehrdadesmaeilian@baylandhealth.com".to_string(),
        "mehrdad.esmaeilian@baylandhealth.com".to_string(),
        "mehrdade@baylandhealth.com".to_string(),
        "mesmaeilian@baylandhealth.com".to_string(),
        "kris@baylandhealth.com".to_string(),
        "chen@baylandhealth.com".to_string(),
        "krischen@baylandhealth.com".to_string(),
        "kris.chen@baylandhealth.com".to_string(),
        "krisc@baylandhealth.com".to_string(),
        "kchen@baylandhealth.com".to_string(),
        "bayland@baylandhealth.com".to_string(),
        "health@baylandhealth.com".to_string(),
        "baylandhealth@baylandhealth.com".to_string(),
        "bayland.health@baylandhealth.com".to_string(),
        "baylandh@baylandhealth.com".to_string(),
        "bhealth@baylandhealth.com".to_string(),
        "jason@baylandhealth.com".to_string(),
        "burke@baylandhealth.com".to_string(),
        "jasonburke@baylandhealth.com".to_string(),
        "jason.burke@baylandhealth.com".to_string(),
        "jasonb@baylandhealth.com".to_string(),
        "jburke@baylandhealth.com".to_string(),
        "brittany@baylandhealth.com".to_string(),
        "stalworth@baylandhealth.com".to_string(),
        "brittanystalworth@baylandhealth.com".to_string(),
        "brittany.stalworth@baylandhealth.com".to_string(),
        "brittanys@baylandhealth.com".to_string(),
        "bstalworth@baylandhealth.com".to_string(),
        "briana@baylandhealth.com".to_string(),
        "leslie@baylandhealth.com".to_string(),
        "brianaleslie@baylandhealth.com".to_string(),
        "briana.leslie@baylandhealth.com".to_string(),
        "brianal@baylandhealth.com".to_string(),
        "bleslie@baylandhealth.com".to_string(),
        "michelle@baylandhealth.com".to_string(),
        "lora@baylandhealth.com".to_string(),
        "michellelora@baylandhealth.com".to_string(),
        "michelle.lora@baylandhealth.com".to_string(),
        "michellel@baylandhealth.com".to_string(),
        "mlora@baylandhealth.com".to_string(),
        "angela@baylandhealth.com".to_string(),
        "cheng@baylandhealth.com".to_string(),
        "angelacheng@baylandhealth.com".to_string(),
        "angela.cheng@baylandhealth.com".to_string(),
        "angelac@baylandhealth.com".to_string(),
        "acheng@baylandhealth.com".to_string(),
        "joseph@baylandhealth.com".to_string(),
        "sun@baylandhealth.com".to_string(),
        "josephsun@baylandhealth.com".to_string(),
        "joseph.sun@baylandhealth.com".to_string(),
        "josephs@baylandhealth.com".to_string(),
        "jsun@baylandhealth.com".to_string(),
        "quinn@spreetail.com".to_string(),
        "small@spreetail.com".to_string(),
        "quinnsmall@spreetail.com".to_string(),
        "quinn.small@spreetail.com".to_string(),
        "quinns@spreetail.com".to_string(),
        "qsmall@spreetail.com".to_string(),
        "nelson@spreetail.com".to_string(),
        "micek@spreetail.com".to_string(),
        "nelsonmicek@spreetail.com".to_string(),
        "nelson.micek@spreetail.com".to_string(),
        "nelsonm@spreetail.com".to_string(),
        "nmicek@spreetail.com".to_string(),
        "chad@spreetail.com".to_string(),
        "kilpatrick@spreetail.com".to_string(),
        "chadkilpatrick@spreetail.com".to_string(),
        "chad.kilpatrick@spreetail.com".to_string(),
        "chadk@spreetail.com".to_string(),
        "ckilpatrick@spreetail.com".to_string(),
        "josh@spreetail.com".to_string(),
        "smith@spreetail.com".to_string(),
        "joshsmith@spreetail.com".to_string(),
        "josh.smith@spreetail.com".to_string(),
        "joshs@spreetail.com".to_string(),
        "jsmith@spreetail.com".to_string(),
        "justine@spreetail.com".to_string(),
        "steiner@spreetail.com".to_string(),
        "justinesteiner@spreetail.com".to_string(),
        "justine.steiner@spreetail.com".to_string(),
        "justines@spreetail.com".to_string(),
        "jsteiner@spreetail.com".to_string(),
        "zhen@baylandhealth.com".to_string(),
        "su@baylandhealth.com".to_string(),
        "zhensu@baylandhealth.com".to_string(),
        "zhen.su@baylandhealth.com".to_string(),
        "zhens@baylandhealth.com".to_string(),
        "zsu@baylandhealth.com".to_string(),
        "george@baylandhealth.com".to_string(),
        "pu@baylandhealth.com".to_string(),
        "georgepu@baylandhealth.com".to_string(),
        "george.pu@baylandhealth.com".to_string(),
        "georgep@baylandhealth.com".to_string(),
        "gpu@baylandhealth.com".to_string(),
        "mike@baylandhealth.com".to_string(),
        "zhu@baylandhealth.com".to_string(),
        "mikezhu@baylandhealth.com".to_string(),
        "mike.zhu@baylandhealth.com".to_string(),
        "mikez@baylandhealth.com".to_string(),
        "mzhu@baylandhealth.com".to_string(),
        "zicheng@baylandhealth.com".to_string(),
        "zhao@baylandhealth.com".to_string(),
        "zichengzhao@baylandhealth.com".to_string(),
        "zicheng.zhao@baylandhealth.com".to_string(),
        "zichengz@baylandhealth.com".to_string(),
        "zzhao@baylandhealth.com".to_string(),
        "frank@baylandhealth.com".to_string(),
        "frankf@baylandhealth.com".to_string(),
        "katherine@baylandhealth.com".to_string(),
        "thomas@baylandhealth.com".to_string(),
        "katherinethomas@baylandhealth.com".to_string(),
        "katherine.thomas@baylandhealth.com".to_string(),
        "katherinet@baylandhealth.com".to_string(),
        "kthomas@baylandhealth.com".to_string(),
        "pchen@bestchoiceproducts.com".to_string(),
        "jabran@utopiadeals.com".to_string(),
        "jabran.niaz@utopiadeals.com".to_string(),
        "zappos@zappos.com".to_string(),
        "pbucher@zappos.com".to_string(),
        "nick@baylandhealth.com".to_string(),
        "fecarotta@baylandhealth.com".to_string(),
        "nickfecarotta@baylandhealth.com".to_string(),
        "nick.fecarotta@baylandhealth.com".to_string(),
        "nickf@baylandhealth.com".to_string(),
        "nfecarotta@baylandhealth.com".to_string(),
        "andrew@baylandhealth.com".to_string(),
        "abraham@baylandhealth.com".to_string(),
        "andrewabraham@baylandhealth.com".to_string(),
        "andrew.abraham@baylandhealth.com".to_string(),
        "andrewa@baylandhealth.com".to_string(),
        "aabraham@baylandhealth.com".to_string(),
        "gokhan.boz@verisure.com".to_string(),
        "nina.cronstedt@verisure.com".to_string(),
        "pking@soldejaneiro.com".to_string(),
        "uma@ebay.com".to_string(),
        "michael@focuscamera.com".to_string(),
        "msilberstein@focuscamera.com".to_string(),
        "stevenwang@yaheetech.eu".to_string(),
        "pindarli@yaheetech.eu".to_string(),
        "mike@apexmediaseattle.com".to_string(),
        "jerrod@apexmediaseattle.com".to_string(),
        "jamie@ebay.com".to_string(),
        "samuel.stewart@eurooptic.com".to_string(),
        "vsmolyanskyy@focuscamera.com".to_string(),
        "ssilberstein@focuscamera.com".to_string(),
        "dave@apexmediaseattle.com".to_string(),
        "angusxu@yaheetech.eu".to_string(),
        "sam@focuscamera.com".to_string(),
        "samsilberstein@focuscamera.com".to_string(),
        "isaac@realessentials.com".to_string(),
        "kelsey@realessentials.com".to_string(),
        "kcortina@heydude.com".to_string(),
        "michael@realessentials.com".to_string(),
        "moses@realessentials.com".to_string(),
        "maggy@houstondiamondoutlet.com".to_string(),
        "jay@houstondiamondoutlet.com".to_string(),
        "zuzana@justlovefashion.com".to_string(),
        "melicharova@justlovefashion.com".to_string(),
        "zuzanamelicharova@justlovefashion.com".to_string(),
        "zuzana.melicharova@justlovefashion.com".to_string(),
        "zuzanam@justlovefashion.com".to_string(),
        "zmelicharova@justlovefashion.com".to_string(),
        "brian@justlovefashion.com".to_string(),
        "baskin@justlovefashion.com".to_string(),
        "brianbaskin@justlovefashion.com".to_string(),
        "brian.baskin@justlovefashion.com".to_string(),
        "brianb@justlovefashion.com".to_string(),
        "bbaskin@justlovefashion.com".to_string(),
        "john@justlovefashion.com".to_string(),
        "jones@justlovefashion.com".to_string(),
        "johnjones@justlovefashion.com".to_string(),
        "john.jones@justlovefashion.com".to_string(),
        "johnj@justlovefashion.com".to_string(),
        "jjones@justlovefashion.com".to_string(),
        "sally@justlovefashion.com".to_string(),
        "mansell@justlovefashion.com".to_string(),
        "sallymansell@justlovefashion.com".to_string(),
        "sally.mansell@justlovefashion.com".to_string(),
        "sallym@justlovefashion.com".to_string(),
        "smansell@justlovefashion.com".to_string(),
        "tate@justlovefashion.com".to_string(),
        "heblon@justlovefashion.com".to_string(),
        "tateheblon@justlovefashion.com".to_string(),
        "tate.heblon@justlovefashion.com".to_string(),
        "tateh@justlovefashion.com".to_string(),
        "theblon@justlovefashion.com".to_string(),
        "courtney@justlovefashion.com".to_string(),
        "foster@justlovefashion.com".to_string(),
        "courtneyfoster@justlovefashion.com".to_string(),
        "courtney.foster@justlovefashion.com".to_string(),
        "courtneyf@justlovefashion.com".to_string(),
        "cfoster@justlovefashion.com".to_string(),
        "kate@justlovefashion.com".to_string(),
        "mackz@justlovefashion.com".to_string(),
        "katemackz@justlovefashion.com".to_string(),
        "kate.mackz@justlovefashion.com".to_string(),
        "katem@justlovefashion.com".to_string(),
        "kmackz@justlovefashion.com".to_string(),
        "mihaly@justlovefashion.com".to_string(),
        "szabo@justlovefashion.com".to_string(),
        "mihalyszabo@justlovefashion.com".to_string(),
        "mihaly.szabo@justlovefashion.com".to_string(),
        "mihalys@justlovefashion.com".to_string(),
        "mszabo@justlovefashion.com".to_string(),
        "just@justlovefashion.com".to_string(),
        "love@justlovefashion.com".to_string(),
        "justlove@justlovefashion.com".to_string(),
        "just.love@justlovefashion.com".to_string(),
        "justl@justlovefashion.com".to_string(),
        "jlove@justlovefashion.com".to_string(),
        "fashion@justlovefashion.com".to_string(),
        "lovefashion@justlovefashion.com".to_string(),
        "love.fashion@justlovefashion.com".to_string(),
        "lovef@justlovefashion.com".to_string(),
        "lfashion@justlovefashion.com".to_string(),
        "chiba@justlovefashion.com".to_string(),
        "achiba@justlovefashion.com".to_string(),
        "hamna@justlovefashion.com".to_string(),
        "rahelle@justlovefashion.com".to_string(),
        "hamnarahelle@justlovefashion.com".to_string(),
        "hamna.rahelle@justlovefashion.com".to_string(),
        "hamnar@justlovefashion.com".to_string(),
        "hrahelle@justlovefashion.com".to_string(),
        "anne-christine@justlovefashion.com".to_string(),
        "polet@justlovefashion.com".to_string(),
        "anne-christinepolet@justlovefashion.com".to_string(),
        "anne-christine.polet@justlovefashion.com".to_string(),
        "anne-christinep@justlovefashion.com".to_string(),
        "apolet@justlovefashion.com".to_string(),
        "fatima@justlovefashion.com".to_string(),
        "ibrahim@justlovefashion.com".to_string(),
        "fatimaibrahim@justlovefashion.com".to_string(),
        "fatima.ibrahim@justlovefashion.com".to_string(),
        "fatimai@justlovefashion.com".to_string(),
        "fibrahim@justlovefashion.com".to_string(),
        "katharina@justlovefashion.com".to_string(),
        "herzog@justlovefashion.com".to_string(),
        "katharinaherzog@justlovefashion.com".to_string(),
        "katharina.herzog@justlovefashion.com".to_string(),
        "katharinah@justlovefashion.com".to_string(),
        "kherzog@justlovefashion.com".to_string(),
        "hmiller@itcosmetics.com".to_string(),
        "rpavela@itcosmetics.com".to_string(),
        "hatcher.meeks@therabody.com".to_string(),
        "joshua@manscaped.com".to_string(),
        "ryan@manscaped.com".to_string(),
        "ryan.fiore@manscaped.com".to_string(),
        "lucas@manscaped.com".to_string(),
        "lucas.coyle@manscaped.com".to_string(),
        "meggan@manscaped.com".to_string(),
        "meggan.porter@manscaped.com".to_string(),
        "coleman@grivetoutdoors.com".to_string(),
        "coleman.whitsitt@grivetoutdoors.com".to_string(),
        "stephen@grivetoutdoors.com".to_string(),
        "will@grivetoutdoors.com".to_string(),
        "michaelmorelli@grivetoutdoors.com".to_string(),
        "cfarrell@heydude.com".to_string(),
        "joan@greatstartools.com".to_string(),
        "zhou@greatstartools.com".to_string(),
        "joan.zhou@greatstartools.com".to_string(),
        "adri@greatstartools.com".to_string(),
        "grivet@grivetoutdoors.com".to_string(),
        "kristiekao@hourloop.com".to_string(),
        "jac@eatsurreal.co.uk".to_string(),
        "chetland@eatsurreal.co.uk".to_string(),
        "jacchetland@eatsurreal.co.uk".to_string(),
        "jac.chetland@eatsurreal.co.uk".to_string(),
        "jacc@eatsurreal.co.uk".to_string(),
        "jchetland@eatsurreal.co.uk".to_string(),
        "kit@eatsurreal.co.uk".to_string(),
        "gammell@eatsurreal.co.uk".to_string(),
        "kitgammell@eatsurreal.co.uk".to_string(),
        "kit.gammell@eatsurreal.co.uk".to_string(),
        "kitg@eatsurreal.co.uk".to_string(),
        "kgammell@eatsurreal.co.uk".to_string(),
        "simone@eatsurreal.co.uk".to_string(),
        "thomas@eatsurreal.co.uk".to_string(),
        "simonethomas@eatsurreal.co.uk".to_string(),
        "simone.thomas@eatsurreal.co.uk".to_string(),
        "simonet@eatsurreal.co.uk".to_string(),
        "sthomas@eatsurreal.co.uk".to_string(),
        "ozawa@thrasio.com".to_string(),
        "zach@grivetoutdoors.com".to_string(),
        "lauren.peterson@grivetoutdoors.com".to_string(),
        "paul@manscaped.com".to_string(),
        "paul.tran@manscaped.com".to_string(),
        "ptran@manscaped.com".to_string(),
        "john@hitouchbusinessservices.com".to_string(),
        "frisk@hitouchbusinessservices.com".to_string(),
        "johnfrisk@hitouchbusinessservices.com".to_string(),
        "john.frisk@hitouchbusinessservices.com".to_string(),
        "johnf@hitouchbusinessservices.com".to_string(),
        "jfrisk@hitouchbusinessservices.com".to_string(),
        "james@hitouchbusinessservices.com".to_string(),
        "hodges@hitouchbusinessservices.com".to_string(),
        "jameshodges@hitouchbusinessservices.com".to_string(),
        "james.hodges@hitouchbusinessservices.com".to_string(),
        "jamesh@hitouchbusinessservices.com".to_string(),
        "jhodges@hitouchbusinessservices.com".to_string(),
        "sheena@hitouchbusinessservices.com".to_string(),
        "christensen@hitouchbusinessservices.com".to_string(),
        "sheenachristensen@hitouchbusinessservices.com".to_string(),
        "sheena.christensen@hitouchbusinessservices.com".to_string(),
        "sheenac@hitouchbusinessservices.com".to_string(),
        "schristensen@hitouchbusinessservices.com".to_string(),
        "debra@hitouchbusinessservices.com".to_string(),
        "jones@hitouchbusinessservices.com".to_string(),
        "debrajones@hitouchbusinessservices.com".to_string(),
        "debra.jones@hitouchbusinessservices.com".to_string(),
        "debraj@hitouchbusinessservices.com".to_string(),
        "djones@hitouchbusinessservices.com".to_string(),
        "scott@hitouchbusinessservices.com".to_string(),
        "miller@hitouchbusinessservices.com".to_string(),
        "scottmiller@hitouchbusinessservices.com".to_string(),
        "scott.miller@hitouchbusinessservices.com".to_string(),
        "scottm@hitouchbusinessservices.com".to_string(),
        "smiller@hitouchbusinessservices.com".to_string(),
        "david@hitouchbusinessservices.com".to_string(),
        "bertlshofer@hitouchbusinessservices.com".to_string(),
        "davidbertlshofer@hitouchbusinessservices.com".to_string(),
        "david.bertlshofer@hitouchbusinessservices.com".to_string(),
        "davidb@hitouchbusinessservices.com".to_string(),
        "dbertlshofer@hitouchbusinessservices.com".to_string(),
        "mallory@thrasio.com".to_string(),
        "mallory.ashwander@thrasio.com".to_string(),
        "business@superiorbrand.com".to_string(),
        "owner@superiorbrand.com".to_string(),
        "businessowner@superiorbrand.com".to_string(),
        "business.owner@superiorbrand.com".to_string(),
        "businesso@superiorbrand.com".to_string(),
        "bowner@superiorbrand.com".to_string(),
        "steve@superiorbrand.com".to_string(),
        "hugh@superiorbrand.com".to_string(),
        "stevehugh@superiorbrand.com".to_string(),
        "steve.hugh@superiorbrand.com".to_string(),
        "steveh@superiorbrand.com".to_string(),
        "shugh@superiorbrand.com".to_string(),
        "superio@superiorbrand.com".to_string(),
        "brand@superiorbrand.com".to_string(),
        "superiobrand@superiorbrand.com".to_string(),
        "superio.brand@superiorbrand.com".to_string(),
        "superiob@superiorbrand.com".to_string(),
        "sbrand@superiorbrand.com".to_string(),
        "justin@superiorbrand.com".to_string(),
        "moss@superiorbrand.com".to_string(),
        "justinmoss@superiorbrand.com".to_string(),
        "justin.moss@superiorbrand.com".to_string(),
        "justinm@superiorbrand.com".to_string(),
        "jmoss@superiorbrand.com".to_string(),
        "gonzalo@superiorbrand.com".to_string(),
        "goberna@superiorbrand.com".to_string(),
        "gonzalogoberna@superiorbrand.com".to_string(),
        "gonzalo.goberna@superiorbrand.com".to_string(),
        "gonzalog@superiorbrand.com".to_string(),
        "ggoberna@superiorbrand.com".to_string(),
        "uchenna@superiorbrand.com".to_string(),
        "edeh@superiorbrand.com".to_string(),
        "uchennaedeh@superiorbrand.com".to_string(),
        "uchenna.edeh@superiorbrand.com".to_string(),
        "uchennae@superiorbrand.com".to_string(),
        "uedeh@superiorbrand.com".to_string(),
        "daniel@superiorbrand.com".to_string(),
        "min@superiorbrand.com".to_string(),
        "danielmin@superiorbrand.com".to_string(),
        "daniel.min@superiorbrand.com".to_string(),
        "danielm@superiorbrand.com".to_string(),
        "dmin@superiorbrand.com".to_string(),
        "charnier@superiorbrand.com".to_string(),
        "corey@superiorbrand.com".to_string(),
        "charniercorey@superiorbrand.com".to_string(),
        "charnier.corey@superiorbrand.com".to_string(),
        "charnierc@superiorbrand.com".to_string(),
        "ccorey@superiorbrand.com".to_string(),
        "clarissa@eatsurreal.co.uk".to_string(),
        "boys@eatsurreal.co.uk".to_string(),
        "clarissaboys@eatsurreal.co.uk".to_string(),
        "clarissa.boys@eatsurreal.co.uk".to_string(),
        "clarissab@eatsurreal.co.uk".to_string(),
        "cboys@eatsurreal.co.uk".to_string(),
        "ian@eatsurreal.co.uk".to_string(),
        "lurie@eatsurreal.co.uk".to_string(),
        "ianlurie@eatsurreal.co.uk".to_string(),
        "ian.lurie@eatsurreal.co.uk".to_string(),
        "ianl@eatsurreal.co.uk".to_string(),
        "ilurie@eatsurreal.co.uk".to_string(),
        "sylvain@eatsurreal.co.uk".to_string(),
        "nony@eatsurreal.co.uk".to_string(),
        "sylvainnony@eatsurreal.co.uk".to_string(),
        "sylvain.nony@eatsurreal.co.uk".to_string(),
        "sylvainn@eatsurreal.co.uk".to_string(),
        "snony@eatsurreal.co.uk".to_string(),
        "gracej@shoplet.com".to_string(),
        "john@shoplet.com".to_string(),
        "tony@shoplet.com".to_string(),
        "tonyellison@shoplet.com".to_string(),
        "tony.ellison@shoplet.com".to_string(),
        "markg@shoplet.com".to_string(),
        "isaac@shoplet.com".to_string(),
        "isaacv@shoplet.com".to_string(),
        "bill@shoplet.com".to_string(),
        "bill.leonard@shoplet.com".to_string(),
        "billl@shoplet.com".to_string(),
        "bleonard@shoplet.com".to_string(),
        "henry.guo@aborderproducts.com".to_string(),
        "michelle.wang@aborderproducts.com".to_string(),
        "paul@lightbulbs.com".to_string(),
        "justin@lightbulbs.com".to_string(),
        "christopher@lightbulbs.com".to_string(),
        "jason@metroshoewarehouse.com".to_string(),
        "heath@metroshoewarehouse.com".to_string(),
        "jessica@metroshoewarehouse.com".to_string(),
        "dominique@metroshoewarehouse.com".to_string(),
        "tony@metroshoewarehouse.com".to_string(),
        "scott@eatsurreal.co.uk".to_string(),
        "brundage@eatsurreal.co.uk".to_string(),
        "scottbrundage@eatsurreal.co.uk".to_string(),
        "scott.brundage@eatsurreal.co.uk".to_string(),
        "scottb@eatsurreal.co.uk".to_string(),
        "sbrundage@eatsurreal.co.uk".to_string(),
        "shannon@eatsurreal.co.uk".to_string(),
        "nightingale@eatsurreal.co.uk".to_string(),
        "shannonnightingale@eatsurreal.co.uk".to_string(),
        "shannon.nightingale@eatsurreal.co.uk".to_string(),
        "shannonn@eatsurreal.co.uk".to_string(),
        "snightingale@eatsurreal.co.uk".to_string(),
        "laura@eatsurreal.co.uk".to_string(),
        "bosworth@eatsurreal.co.uk".to_string(),
        "laurabosworth@eatsurreal.co.uk".to_string(),
        "laura.bosworth@eatsurreal.co.uk".to_string(),
        "laurab@eatsurreal.co.uk".to_string(),
        "lbosworth@eatsurreal.co.uk".to_string(),
        "brian@hitouchbusinessservices.com".to_string(),
        "shephard@hitouchbusinessservices.com".to_string(),
        "brianshephard@hitouchbusinessservices.com".to_string(),
        "brian.shephard@hitouchbusinessservices.com".to_string(),
        "brians@hitouchbusinessservices.com".to_string(),
        "bshephard@hitouchbusinessservices.com".to_string(),
        "chris@hitouchbusinessservices.com".to_string(),
        "stoutamire@hitouchbusinessservices.com".to_string(),
        "chrisstoutamire@hitouchbusinessservices.com".to_string(),
        "chris.stoutamire@hitouchbusinessservices.com".to_string(),
        "chriss@hitouchbusinessservices.com".to_string(),
        "cstoutamire@hitouchbusinessservices.com".to_string(),
        "lukhona@hitouchbusinessservices.com".to_string(),
        "lubisi@hitouchbusinessservices.com".to_string(),
        "lukhonalubisi@hitouchbusinessservices.com".to_string(),
        "lukhona.lubisi@hitouchbusinessservices.com".to_string(),
        "lukhonal@hitouchbusinessservices.com".to_string(),
        "llubisi@hitouchbusinessservices.com".to_string(),
        "desmond.reeves@aborderproducts.com".to_string(),
        "lizbeth.duarte@aborderproducts.com".to_string(),
        "charles@lightbulbs.com".to_string(),
        "scott@superiorbrand.com".to_string(),
        "curtis@superiorbrand.com".to_string(),
        "scottcurtis@superiorbrand.com".to_string(),
        "scott.curtis@superiorbrand.com".to_string(),
        "scottc@superiorbrand.com".to_string(),
        "scurtis@superiorbrand.com".to_string(),
        "rick@superiorbrand.com".to_string(),
        "sousa@superiorbrand.com".to_string(),
        "ricksousa@superiorbrand.com".to_string(),
        "rick.sousa@superiorbrand.com".to_string(),
        "ricks@superiorbrand.com".to_string(),
        "rsousa@superiorbrand.com".to_string(),
        "jinja@superiorbrand.com".to_string(),
        "birkenbeuel@superiorbrand.com".to_string(),
        "jinjabirkenbeuel@superiorbrand.com".to_string(),
        "jinja.birkenbeuel@superiorbrand.com".to_string(),
        "jinjab@superiorbrand.com".to_string(),
        "jbirkenbeuel@superiorbrand.com".to_string(),
        "ozlem@superiorbrand.com".to_string(),
        "tuskan@superiorbrand.com".to_string(),
        "ozlemtuskan@superiorbrand.com".to_string(),
        "ozlem.tuskan@superiorbrand.com".to_string(),
        "ozlemt@superiorbrand.com".to_string(),
        "otuskan@superiorbrand.com".to_string(),
        "cheri@superiorbrand.com".to_string(),
        "bailey@superiorbrand.com".to_string(),
        "cheribailey@superiorbrand.com".to_string(),
        "cheri.bailey@superiorbrand.com".to_string(),
        "cherib@superiorbrand.com".to_string(),
        "cbailey@superiorbrand.com".to_string(),
        "danielh@superiorbrand.com".to_string(),
        "jatin@superiorbrand.com".to_string(),
        "aroora@superiorbrand.com".to_string(),
        "jatinaroora@superiorbrand.com".to_string(),
        "jatin.aroora@superiorbrand.com".to_string(),
        "jatina@superiorbrand.com".to_string(),
        "jaroora@superiorbrand.com".to_string(),
        "william@ergoav.com".to_string(),
        "swari@ergoav.com".to_string(),
        "williamswari@ergoav.com".to_string(),
        "william.swari@ergoav.com".to_string(),
        "williams@ergoav.com".to_string(),
        "wswari@ergoav.com".to_string(),
        "alexander@superiorbrand.com".to_string(),
        "millet@superiorbrand.com".to_string(),
        "alexandermillet@superiorbrand.com".to_string(),
        "alexander.millet@superiorbrand.com".to_string(),
        "alexanderm@superiorbrand.com".to_string(),
        "amillet@superiorbrand.com".to_string(),
        "denev@superiorbrand.com".to_string(),
        "danieldenev@superiorbrand.com".to_string(),
        "daniel.denev@superiorbrand.com".to_string(),
        "danield@superiorbrand.com".to_string(),
        "ddenev@superiorbrand.com".to_string(),
        "ann@superiorbrand.com".to_string(),
        "viaene@superiorbrand.com".to_string(),
        "annviaene@superiorbrand.com".to_string(),
        "ann.viaene@superiorbrand.com".to_string(),
        "annv@superiorbrand.com".to_string(),
        "aviaene@superiorbrand.com".to_string(),
        "lauren@superiorbrand.com".to_string(),
        "keeton@superiorbrand.com".to_string(),
        "laurenkeeton@superiorbrand.com".to_string(),
        "lauren.keeton@superiorbrand.com".to_string(),
        "laurenk@superiorbrand.com".to_string(),
        "lkeeton@superiorbrand.com".to_string(),
        "flanigan@superiorbrand.com".to_string(),
        "laurenflanigan@superiorbrand.com".to_string(),
        "lauren.flanigan@superiorbrand.com".to_string(),
        "laurenf@superiorbrand.com".to_string(),
        "lflanigan@superiorbrand.com".to_string(),
        "julianreis@superordinary.co".to_string(),
        "eolesh@heydude.com".to_string(),
        "robert@ergoav.com".to_string(),
        "dalson@ergoav.com".to_string(),
        "robertdalson@ergoav.com".to_string(),
        "robert.dalson@ergoav.com".to_string(),
        "robertd@ergoav.com".to_string(),
        "rdalson@ergoav.com".to_string(),
        "scott@lightbulbs.com".to_string(),
        "bill@ergoav.com".to_string(),
        "pantaleo@ergoav.com".to_string(),
        "billpantaleo@ergoav.com".to_string(),
        "bill.pantaleo@ergoav.com".to_string(),
        "billp@ergoav.com".to_string(),
        "bpantaleo@ergoav.com".to_string(),
        "jeff@ergoav.com".to_string(),
        "lasch@ergoav.com".to_string(),
        "jefflasch@ergoav.com".to_string(),
        "jeff.lasch@ergoav.com".to_string(),
        "jeffl@ergoav.com".to_string(),
        "jlasch@ergoav.com".to_string(),
        "henry@ergoav.com".to_string(),
        "lyu@ergoav.com".to_string(),
        "henrylyu@ergoav.com".to_string(),
        "henry.lyu@ergoav.com".to_string(),
        "henryl@ergoav.com".to_string(),
        "hlyu@ergoav.com".to_string(),
        "carlos@ergoav.com".to_string(),
        "carlosf@ergoav.com".to_string(),
        "sara@ergoav.com".to_string(),
        "grofcsik@ergoav.com".to_string(),
        "saragrofcsik@ergoav.com".to_string(),
        "sara.grofcsik@ergoav.com".to_string(),
        "sarag@ergoav.com".to_string(),
        "sgrofcsik@ergoav.com".to_string(),
        "dana@ergoav.com".to_string(),
        "graham@ergoav.com".to_string(),
        "danagraham@ergoav.com".to_string(),
        "dana.graham@ergoav.com".to_string(),
        "danag@ergoav.com".to_string(),
        "dgraham@ergoav.com".to_string(),
        "gary@hitouchbusinessservices.com".to_string(),
        "naidus@hitouchbusinessservices.com".to_string(),
        "garynaidus@hitouchbusinessservices.com".to_string(),
        "gary.naidus@hitouchbusinessservices.com".to_string(),
        "garyn@hitouchbusinessservices.com".to_string(),
        "gnaidus@hitouchbusinessservices.com".to_string(),
        "sherrywan@superordinary.co".to_string(),
        "timo@mtah.net".to_string(),
        "timmi@mtah.net".to_string(),
        "timotimmi@mtah.net".to_string(),
        "timo.timmi@mtah.net".to_string(),
        "timot@mtah.net".to_string(),
        "ttimmi@mtah.net".to_string(),
        "colin@mtah.net".to_string(),
        "bradford@mtah.net".to_string(),
        "colinbradford@mtah.net".to_string(),
        "colin.bradford@mtah.net".to_string(),
        "colinb@mtah.net".to_string(),
        "cbradford@mtah.net".to_string(),
        "jack@mtah.net".to_string(),
        "brown@mtah.net".to_string(),
        "jackbrown@mtah.net".to_string(),
        "jack.brown@mtah.net".to_string(),
        "jackb@mtah.net".to_string(),
        "jbrown@mtah.net".to_string(),
        "suman@mtah.net".to_string(),
        "saraf@mtah.net".to_string(),
        "sumansaraf@mtah.net".to_string(),
        "suman.saraf@mtah.net".to_string(),
        "sumans@mtah.net".to_string(),
        "ssaraf@mtah.net".to_string(),
        "shariq@mtah.net".to_string(),
        "enterprises@mtah.net".to_string(),
        "shariqenterprises@mtah.net".to_string(),
        "shariq.enterprises@mtah.net".to_string(),
        "shariqe@mtah.net".to_string(),
        "senterprises@mtah.net".to_string(),
        "abhinay@mtah.net".to_string(),
        "sharma@mtah.net".to_string(),
        "abhinaysharma@mtah.net".to_string(),
        "abhinay.sharma@mtah.net".to_string(),
        "abhinays@mtah.net".to_string(),
        "asharma@mtah.net".to_string(),
        "jamie@mtah.net".to_string(),
        "frost@mtah.net".to_string(),
        "jamiefrost@mtah.net".to_string(),
        "jamie.frost@mtah.net".to_string(),
        "jamief@mtah.net".to_string(),
        "jfrost@mtah.net".to_string(),
        "arkadiy@shoplet.com".to_string(),
        "arkadiyp@shoplet.com".to_string(),
        "james@shoplet.com".to_string(),
        "anvar@mtah.net".to_string(),
        "bagautdinov@mtah.net".to_string(),
        "anvarbagautdinov@mtah.net".to_string(),
        "anvar.bagautdinov@mtah.net".to_string(),
        "anvarb@mtah.net".to_string(),
        "abagautdinov@mtah.net".to_string(),
        "ian@mtah.net".to_string(),
        "anderson@mtah.net".to_string(),
        "iananderson@mtah.net".to_string(),
        "ian.anderson@mtah.net".to_string(),
        "iana@mtah.net".to_string(),
        "ianderson@mtah.net".to_string(),
        "kevin@mtah.net".to_string(),
        "kemper@mtah.net".to_string(),
        "kevinkemper@mtah.net".to_string(),
        "kevin.kemper@mtah.net".to_string(),
        "kevink@mtah.net".to_string(),
        "kkemper@mtah.net".to_string(),
        "francisco@mtah.net".to_string(),
        "barriga@mtah.net".to_string(),
        "franciscobarriga@mtah.net".to_string(),
        "francisco.barriga@mtah.net".to_string(),
        "franciscob@mtah.net".to_string(),
        "fbarriga@mtah.net".to_string(),
        "madhavan@mtah.net".to_string(),
        "mathivanan@mtah.net".to_string(),
        "madhavanmathivanan@mtah.net".to_string(),
        "madhavan.mathivanan@mtah.net".to_string(),
        "madhavanm@mtah.net".to_string(),
        "mmathivanan@mtah.net".to_string(),
        "bharathy@mtah.net".to_string(),
        "bharadwaj@mtah.net".to_string(),
        "bharathybharadwaj@mtah.net".to_string(),
        "bharathy.bharadwaj@mtah.net".to_string(),
        "bharathyb@mtah.net".to_string(),
        "bbharadwaj@mtah.net".to_string(),
        "joe@mtah.net".to_string(),
        "zhou@mtah.net".to_string(),
        "joezhou@mtah.net".to_string(),
        "joe.zhou@mtah.net".to_string(),
        "joez@mtah.net".to_string(),
        "jzhou@mtah.net".to_string(),
        "henrik@mtah.net".to_string(),
        "appert@mtah.net".to_string(),
        "henrikappert@mtah.net".to_string(),
        "henrik.appert@mtah.net".to_string(),
        "henrika@mtah.net".to_string(),
        "happert@mtah.net".to_string(),
        "erin@homespunllc.com".to_string(),
        "woods@homespunllc.com".to_string(),
        "erinwoods@homespunllc.com".to_string(),
        "erin.woods@homespunllc.com".to_string(),
        "erinw@homespunllc.com".to_string(),
        "ewoods@homespunllc.com".to_string(),
        "baban.mitra@expedia.co.in".to_string(),
        "joonas@expedia.co.in".to_string(),
        "kevin@certifiedbrands.com".to_string(),
        "nancy@homespunllc.com".to_string(),
        "pierce@homespunllc.com".to_string(),
        "nancypierce@homespunllc.com".to_string(),
        "nancy.pierce@homespunllc.com".to_string(),
        "nancyp@homespunllc.com".to_string(),
        "npierce@homespunllc.com".to_string(),
        "cheryl@homespunllc.com".to_string(),
        "barlow@homespunllc.com".to_string(),
        "cherylbarlow@homespunllc.com".to_string(),
        "cheryl.barlow@homespunllc.com".to_string(),
        "cherylb@homespunllc.com".to_string(),
        "cbarlow@homespunllc.com".to_string(),
        "ttigue@urbandecay.com".to_string(),
        "fbodnar@designsforhealth.com".to_string(),
        "mickeyn@designsforhealth.com".to_string(),
        "mnelson@designsforhealth.com".to_string(),
        "frizzo@designsforhealth.com".to_string(),
        "wgordon@designsforhealth.com".to_string(),
        "andrew@tppretail.com".to_string(),
        "michelle@mixwholesale.com".to_string(),
        "alon@mtah.net".to_string(),
        "amit@mtah.net".to_string(),
        "alonamit@mtah.net".to_string(),
        "alon.amit@mtah.net".to_string(),
        "alona@mtah.net".to_string(),
        "aamit@mtah.net".to_string(),
        "neil@mtah.net".to_string(),
        "trivedi@mtah.net".to_string(),
        "neiltrivedi@mtah.net".to_string(),
        "neil.trivedi@mtah.net".to_string(),
        "neilt@mtah.net".to_string(),
        "ntrivedi@mtah.net".to_string(),
        "neha@mtah.net".to_string(),
        "agrawal@mtah.net".to_string(),
        "nehaagrawal@mtah.net".to_string(),
        "neha.agrawal@mtah.net".to_string(),
        "nehaa@mtah.net".to_string(),
        "nagrawal@mtah.net".to_string(),
        "jonathan@mtah.net".to_string(),
        "keefer@mtah.net".to_string(),
        "jonathankeefer@mtah.net".to_string(),
        "jonathan.keefer@mtah.net".to_string(),
        "jonathank@mtah.net".to_string(),
        "jkeefer@mtah.net".to_string(),
        "patrick@mtah.net".to_string(),
        "dichter@mtah.net".to_string(),
        "patrickdichter@mtah.net".to_string(),
        "patrick.dichter@mtah.net".to_string(),
        "patrickd@mtah.net".to_string(),
        "pdichter@mtah.net".to_string(),
        "jordan@mtah.net".to_string(),
        "casey@mtah.net".to_string(),
        "jordancasey@mtah.net".to_string(),
        "jordan.casey@mtah.net".to_string(),
        "jordanc@mtah.net".to_string(),
        "jcasey@mtah.net".to_string(),
        "conradie@mtah.net".to_string(),
        "kevinconradie@mtah.net".to_string(),
        "kevin.conradie@mtah.net".to_string(),
        "kevinc@mtah.net".to_string(),
        "kconradie@mtah.net".to_string(),
        "julie.stephenson@tppretail.com".to_string(),
        "shane@curiobrands.com".to_string(),
        "davis@curiobrands.com".to_string(),
        "shanedavis@curiobrands.com".to_string(),
        "shane.davis@curiobrands.com".to_string(),
        "shaned@curiobrands.com".to_string(),
        "sdavis@curiobrands.com".to_string(),
        "steve@curiobrands.com".to_string(),
        "lubahn@curiobrands.com".to_string(),
        "stevelubahn@curiobrands.com".to_string(),
        "steve.lubahn@curiobrands.com".to_string(),
        "stevel@curiobrands.com".to_string(),
        "slubahn@curiobrands.com".to_string(),
        "matthew@certifiedbrands.com".to_string(),
        "chris@linkedin.com".to_string(),
        "jeff.weiner@linkedin.com".to_string(),
        "jweiner@linkedin.com".to_string(),
        "snowbell@snowbellmachines.com".to_string(),
        "curio@curiobrands.com".to_string(),
        "brands@curiobrands.com".to_string(),
        "curiobrands@curiobrands.com".to_string(),
        "curio.brands@curiobrands.com".to_string(),
        "curiob@curiobrands.com".to_string(),
        "cbrands@curiobrands.com".to_string(),
        "robert@curiobrands.com".to_string(),
        "bond@curiobrands.com".to_string(),
        "robertbond@curiobrands.com".to_string(),
        "robert.bond@curiobrands.com".to_string(),
        "robertb@curiobrands.com".to_string(),
        "rbond@curiobrands.com".to_string(),
        "brittany@curiobrands.com".to_string(),
        "mccool@curiobrands.com".to_string(),
        "brittanymccool@curiobrands.com".to_string(),
        "brittany.mccool@curiobrands.com".to_string(),
        "brittanym@curiobrands.com".to_string(),
        "bmccool@curiobrands.com".to_string(),
        "chanel@curiobrands.com".to_string(),
        "chanels@curiobrands.com".to_string(),
        "abraham@sunco.com".to_string(),
        "swan@sunco.com".to_string(),
        "ibrahim@sunco.com".to_string(),
        "isaiah@sunco.com".to_string(),
        "juan@sunco.com".to_string(),
        "dalton@sunco.com".to_string(),
        "rick.claus@microsoft.com".to_string(),
        "rickc@microsoft.com".to_string(),
        "rclaus@microsoft.com".to_string(),
        "puneetc@microsoft.com".to_string(),
        "pchandok@microsoft.com".to_string(),
        "dave@microsoft.com".to_string(),
        "davew@microsoft.com".to_string(),
        "daniel.rippey@microsoft.com".to_string(),
        "drippey@microsoft.com".to_string(),
        "ralph.haupter@microsoft.com".to_string(),
        "ralphh@microsoft.com".to_string(),
        "satya@microsoft.com".to_string(),
        "satyan@microsoft.com".to_string(),
        "snadella@microsoft.com".to_string(),
        "lisa.gralnek@ifdesign.com".to_string(),
        "priyanka@snowbellmachines.com".to_string(),
        "toby.keni@ifdesign.com".to_string(),
        "dan@sunco.com".to_string(),
        "silas@tiktok.com".to_string(),
        "allison@tiktok.com".to_string(),
        "allison.moore@tiktok.com".to_string(),
        "soyee@soyee.co.kr".to_string(),
        "heatherm@halloweencostumes.com".to_string(),
        "ash@tiktok.com".to_string(),
        "kris@tiktok.com".to_string(),
        "russ@woot.com".to_string(),
        "delnevo@woot.com".to_string(),
        "russdelnevo@woot.com".to_string(),
        "russ.delnevo@woot.com".to_string(),
        "russd@woot.com".to_string(),
        "rdelnevo@woot.com".to_string(),
        "kirk@woot.com".to_string(),
        "anderson@woot.com".to_string(),
        "kirkanderson@woot.com".to_string(),
        "kirk.anderson@woot.com".to_string(),
        "kanderson@woot.com".to_string(),
        "eugene@woot.com".to_string(),
        "meidinger@woot.com".to_string(),
        "eugene.meidinger@woot.com".to_string(),
        "eugenem@woot.com".to_string(),
        "emeidinger@woot.com".to_string(),
        "joel@woot.com".to_string(),
        "lewis@woot.com".to_string(),
        "joellewis@woot.com".to_string(),
        "joel.lewis@woot.com".to_string(),
        "joell@woot.com".to_string(),
        "curtis@woot.com".to_string(),
        "curtismanlapig@woot.com".to_string(),
        "curtis.manlapig@woot.com".to_string(),
        "curtism@woot.com".to_string(),
        "cmanlapig@woot.com".to_string(),
        "aslanian@woot.com".to_string(),
        "brandyaslanian@woot.com".to_string(),
        "brandy.aslanian@woot.com".to_string(),
        "baslanian@woot.com".to_string(),
        "saeth@woot.com".to_string(),
        "saethgronberg@woot.com".to_string(),
        "sgronberg@woot.com".to_string(),
        "mattgeorge@woot.com".to_string(),
        "mattg@woot.com".to_string(),
        "mgeorge@woot.com".to_string(),
        "chris@woot.com".to_string(),
        "chrisklassen@woot.com".to_string(),
        "chris.klassen@woot.com".to_string(),
    ];

    for em in emails {
        let result = sentinel.get_email_verification_status(&em).await;
        log::info!("{} is reachable? {:?}", em, result);
    }

    HttpResponse::Ok().body("Done!")
}
