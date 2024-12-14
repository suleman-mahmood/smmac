use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, EmailRow};

#[derive(Template)]
#[template(path = "email.html")]
struct EmailTemplate {
    emails: Vec<EmailRow>,
}

#[get("/email")]
async fn email(pool: web::Data<PgPool>) -> HttpResponse {
    let emails = app_db::get_email_table(&pool).await.unwrap_or(vec![]);
    HttpResponse::Ok().body(EmailTemplate { emails }.render().unwrap())
}
