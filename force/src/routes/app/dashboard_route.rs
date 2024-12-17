use actix_web::{get, post, web, HttpResponse};
use askama::Template;
use serde::Deserialize;
use sqlx::PgPool;

use crate::dal::{
    app_db::{self, ProductRow},
    config_db,
};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    products: Vec<ProductRow>,
}

#[get("/dashboard")]
async fn dashboard(pool: web::Data<PgPool>) -> HttpResponse {
    let products = app_db::get_product_table(&pool).await.unwrap_or(vec![]);
    HttpResponse::Ok().body(DashboardTemplate { products }.render().unwrap())
}

#[derive(Deserialize)]
struct SetConfigBody {
    key: String,
    value: String,
}

#[post("/set-config")]
async fn set_config(pool: web::Data<PgPool>, body: web::Form<SetConfigBody>) -> HttpResponse {
    match body.key.as_str() {
        "chatgpt-products-for-niche-start" => {
            config_db::set_gippity_prompt(Some(&body.value), None, &pool)
                .await
                .unwrap();
        }
        "chatgpt-products-for-niche-end" => {
            config_db::set_gippity_prompt(None, Some(&body.value), &pool)
                .await
                .unwrap();
        }
        "google-search-domain-page-depth" => {
            config_db::set_google_search_page_depth(body.value.parse().unwrap_or(1), &pool)
                .await
                .unwrap();
        }
        _ => return HttpResponse::Ok().body(format!("Setting wrong configuration: {}", body.key)),
    }

    HttpResponse::Ok().body("Done!")
}
