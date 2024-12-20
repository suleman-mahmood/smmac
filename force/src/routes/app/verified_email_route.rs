use actix_web::{get, web, HttpResponse};
use askama::Template;
use sqlx::PgPool;

use crate::dal::app_db::{self, VerifiedEmailRow};

#[derive(Template)]
#[template(path = "verified_email.html")]
struct VerifiedEmailTemplate {
    emails: Vec<VerifiedEmailTemplateRow>,
}

struct VerifiedEmailTemplateRow {
    pub email: String,
    pub founder_name: String,
    pub domain: String,
    pub product: String,
    pub niche: String,
}

impl From<VerifiedEmailRow> for VerifiedEmailTemplateRow {
    fn from(value: VerifiedEmailRow) -> Self {
        Self {
            email: value.email,
            founder_name: value.founder_name.unwrap_or("".to_string()),
            domain: value.domain.unwrap_or("".to_string()),
            product: value.product.unwrap_or("".to_string()),
            niche: value.niche.unwrap_or("".to_string()),
        }
    }
}

#[get("/verified-email")]
async fn verified_email(pool: web::Data<PgPool>) -> HttpResponse {
    let emails = app_db::get_verified_emails(&pool).await.unwrap_or(vec![]);
    let template_emails = emails.into_iter().map(|e| e.into()).collect();

    HttpResponse::Ok().body(
        VerifiedEmailTemplate {
            emails: template_emails,
        }
        .render()
        .unwrap(),
    )
}
