use actix_web::{get, web, HttpResponse};
use async_smtp::{
    commands::{MailCommand, RcptCommand},
    Envelope, SendableEmail, SmtpClient, SmtpTransport,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;
use tokio::{
    io::{AsyncRead, AsyncWrite, BufStream},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{
    dal::lead_db::{EmailReachability, EmailVerifiedStatus},
    domain::email::{Reachability, VerificationStatus},
    services::{ProductQuerySender, Sentinel},
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
async fn verify_emails(sentinel: web::Data<Sentinel>) -> HttpResponse {
    let emails: Vec<String> = vec![
        "wbush@nimble.com".to_string(),
        // "wesb@nimble.com".to_string(),
        // "bush@nimble.com".to_string(),
        // "wes@nimble.com".to_string(),
        // "andresp@nimble.com".to_string(),
        // "johnk@nimble.com".to_string(),
        // "johnkostoulas@nimble.com".to_string(),
        // "john@nimble.com".to_string(),
        // "awallace@nimble.com".to_string(),
        // "alanw@nimble.com".to_string(),
        // "sulemanmahmood9988347@gmail.com".to_string(),
        // "suleman@mazlo.com".to_string(),
        "sulemanmahmood99@gmail.com".to_string(),
    ];

    for em in emails {
        // CheckEmailOutput input: "sulemanmahmood99@gmail.com", is_reachable: Safe, misc: Ok(MiscDetails { is_disposable: false, is_role_account: false, gravatar_url: None, haveibeenpwned: None }),
        //   mx: Ok(MxDetails { lookup: Ok(MxLookup(Lookup { query: Query { name: Name("gmail.com"), query_type: MX, query_class: IN },
        //     records: [
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 30, exchange: Name("alt3.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 10, exchange: Name("alt1.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 20, exchange: Name("alt2.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 40, exchange: Name("alt4.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 5, exchange: Name("gmail-smtp-in.l.google.com.") })) }
        //     ],
        //     valid_until: Instant { tv_sec: 6077130, tv_nsec: 595604176 } })) })

        let email_output = sentinel.get_email_info(&em).await;
        let exchanges: Vec<String> = email_output
            .mx
            .unwrap()
            .lookup
            .unwrap()
            .iter()
            .map(|rdata| rdata.exchange().to_string())
            .collect();

        log::info!("Got exchangese: {:?}", exchanges);

        let smtp_server = exchanges.first().unwrap();
        let smtp_server = smtp_server.trim_end_matches(".");
        let smtp_server_port = format!("{}:25", smtp_server);

        log::info!("Connecting to smtp server: {:?}", smtp_server_port);

        // Define a new trait that combines AsyncRead, AsyncWrite, and Unpin
        trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}
        impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncReadWrite for T {}

        let stream = TcpStream::connect(smtp_server_port).await.unwrap();
        let stream = BufStream::new(Box::new(stream) as Box<dyn AsyncReadWrite>);
        let client = SmtpClient::new();
        let mut transport = SmtpTransport::new(client, stream).await.unwrap();

        let response = transport
            .get_mut()
            .command(MailCommand::new(
                Some("random.guy@fit.com".parse().unwrap()),
                vec![],
            ))
            .await
            .unwrap();

        log::info!("How is the response? {:?}", response.is_positive());
        log::info!("Code: {:?}", response.code);
        log::info!("Response: {:?}", response);

        let response = transport
            .get_mut()
            .command(RcptCommand::new(em.parse().unwrap(), vec![]))
            .await
            .unwrap();

        log::info!("How is the response? {:?}", response.is_positive());
        log::info!("Code: {:?}", response.code);
        log::info!("Response: {:?}", response);

        // Perform an SMTP handshake
        // let mut smtp_connection = SmtpConnection::connect(
        //     format!("{}:25", smtp_server),
        //     None,
        //     &ClientId::Domain("verwellfit.com".to_string()),
        //     None,
        //     None,
        // )
        // .unwrap();
        //
        // log::info!("Got ehlo response: {:?}", smtp_connection.read_response());
        //
        // let response = smtp_connection
        //     .command(format!("MAIL FROM:<noreply@yourdomain.com>"))
        //     .unwrap();
        // log::info!("How is the response? {:?}", response.code().is_positive());
        // log::info!("Code: {:?}", response.code());
        // log::info!("Response: {:?}", response);
        //
        // let response = smtp_connection
        //     .command(format!("RCPT TO:<{}>", em))
        //     .unwrap();
        // smtp_connection.quit().unwrap();
        //
        // log::info!("How is the response? {:?}", response.code().is_positive());
        // log::info!("Code: {:?}", response.code());
        // log::info!("Response: {:?}", response);
    }

    HttpResponse::Ok().body("Done!")
}
