use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, FounderRow};

#[derive(Template)]
#[template(path = "founder.html")]
struct FounderTemplate {
    founders: Vec<FounderRow>,
}

#[get("/founder")]
async fn founder(pool: web::Data<PgPool>) -> HttpResponse {
    let founders = app_db::get_founder_table(&pool).await.unwrap_or(vec![]);
    HttpResponse::Ok().body(FounderTemplate { founders }.render().unwrap())
}
