use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, VerifiedEmailRow};

#[derive(Template)]
#[template(path = "verified_email.html")]
struct VerifiedEmailTemplate {
    emails: Vec<VerifiedEmailRow>,
}

#[get("/verified-email")]
async fn verified_email(pool: web::Data<PgPool>) -> HttpResponse {
    let emails = app_db::get_verified_emails(&pool).await.unwrap_or(vec![]);

    HttpResponse::Ok().body(VerifiedEmailTemplate { emails }.render().unwrap())
}
