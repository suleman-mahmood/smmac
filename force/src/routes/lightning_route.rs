use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dal::google_webpage_db;
use crate::routes::lead_route;
use crate::services::OpenaiClient;
use crate::services::{save_product_search_queries, ProductQuerySender};

#[derive(Deserialize)]
struct GetLightningLeadsQuery {
    niche: String,
}

#[get("")]
async fn get_lightning_leads(
    openai_client: web::Data<OpenaiClient>,
    query: web::Query<GetLightningLeadsQuery>,
    pool: web::Data<PgPool>,
    product_query_sender: web::Data<ProductQuerySender>,
) -> HttpResponse {
    let niche = query.niche.trim().to_lowercase();

    let products = save_product_search_queries(&pool, &openai_client, &niche).await;

    let queries: Vec<String> = products
        .into_iter()
        .map(|p| lead_route::build_seach_query(&p))
        .collect();
    let product_queries = google_webpage_db::filter_unscraped_product_queries(&pool, queries)
        .await
        .unwrap();

    let product_query_sender = product_query_sender.sender.clone();
    product_queries
        .iter()
        .for_each(|q| product_query_sender.send(q.to_string()).unwrap());

    HttpResponse::Ok().body("Registered domain!")
}
