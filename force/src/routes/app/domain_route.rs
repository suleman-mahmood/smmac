use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, DomainRow};

#[derive(Template)]
#[template(path = "domain.html")]
struct DomainTemplate {
    domains: Vec<DomainRow>,
}

#[get("/domain")]
async fn domain(pool: web::Data<PgPool>) -> HttpResponse {
    let domains = app_db::get_domain_table(&pool).await.unwrap_or(vec![]);
    HttpResponse::Ok().body(DomainTemplate { domains }.render().unwrap())
}
