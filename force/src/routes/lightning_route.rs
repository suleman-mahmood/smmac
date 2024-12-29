use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dal::google_webpage_db;
use crate::routes::lead_route;
use crate::services::{save_product_search_queries, ProductQuerySender};
use crate::services::{OpenaiClient, VerifiedEmailReceiver};

#[derive(Deserialize)]
struct GetLightningLeadsQuery {
    niche: String,
    count: i64,
}

#[get("")]
async fn get_lightning_leads(
    openai_client: web::Data<OpenaiClient>,
    query: web::Query<GetLightningLeadsQuery>,
    pool: web::Data<PgPool>,
    product_query_sender: web::Data<ProductQuerySender>,
    verified_email_receiver: web::Data<VerifiedEmailReceiver>,
) -> HttpResponse {
    let niche = query.niche.trim().to_lowercase();
    if query.count < 1 {
        return HttpResponse::Ok().body("Count should be > 0");
    }

    // INFO: This channel will now start receiving emails
    let mut verified_email_receiver = verified_email_receiver.sender.subscribe();

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

    let mut emails = Vec::new();

    // TODO: Receive only values for your niche input
    while let Ok(em) = verified_email_receiver.recv().await {
        emails.push(em);

        if emails.len() == query.count as usize {
            break;
        }
    }

    HttpResponse::Ok().json(emails)
}
