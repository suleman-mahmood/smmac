use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, ProductRow};

#[derive(Template)]
#[template(path = "product.html")]
struct ProductTemplate {
    products: Vec<ProductRow>,
}

#[get("/product")]
async fn product(pool: web::Data<PgPool>) -> HttpResponse {
    let products = app_db::get_product_table(&pool).await.unwrap_or(vec![]);

    HttpResponse::Ok().body(ProductTemplate { products }.render().unwrap())
}
