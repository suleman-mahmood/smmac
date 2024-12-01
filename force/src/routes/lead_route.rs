use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct GetLeadsFromNicheQuery {
    niche: String,
    requester_email: String,
}

#[get("/")]
async fn get_leads_from_niche(
    body: web::Query<GetLeadsFromNicheQuery>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    /*
    1. User verification and free tier count
    2. Get boolean search list from open api using the niche prompt
    3. Perform web scraping on each boolean search page, store results in db
        3.1 Rotate ips if getting blocked from google
    4. Construct emails from results in previous step
    5. Verify emails from API
    6. Return verified leads (emails)
    */
    todo!()
}
