use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::lead_db::{self, ProductRow};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    products: Vec<ProductRow>,
}

#[get("/dashboard")]
async fn dashboard(pool: web::Data<PgPool>) -> HttpResponse {
    let products = lead_db::get_product_table(&pool).await.unwrap_or(vec![]);
    HttpResponse::Ok().body(DashboardTemplate { products }.render().unwrap())
}
