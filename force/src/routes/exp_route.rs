use actix_web::{get, web, HttpResponse};
use reqwest::header::ValueIter;
use sqlx::PgPool;

use crate::{
    dal::lead_db::{EmailReachability, EmailVerifiedStatus},
    domain::email::{Reachability, VerificationStatus},
    services::ProductQuerySender,
};

#[get("/check-channel-works")]
async fn check_channel_works(domain_scraper_sender: web::Data<ProductQuerySender>) -> HttpResponse {
    let domain_scraper_sender = domain_scraper_sender.sender.clone();
    ["pro 1", "pro 2", "pro 999"].iter().for_each(|q| {
        match domain_scraper_sender.send(q.to_string()) {
            Ok(_) => {}
            Err(e) => log::error!("Found error while sending: {:?}", e),
        }
    });

    HttpResponse::Ok().body("Done")
}

struct EmailRow {
    email_address: String,
    verified_status: EmailVerifiedStatus,
    reachability: EmailReachability,
}

#[get("/migrate")]
async fn migrate(pool: web::Data<PgPool>) -> HttpResponse {
    let rows = sqlx::query!(r"select * from product")
        .fetch_all(pool.as_ref())
        .await
        .unwrap();

    for r in rows {
        if let Err(e) = sqlx::query!(
            r"
            insert into niche
                (user_niche, gippity_prompt, generated_product)
            values
                ($1, $2, $3)
            ",
            r.niche,
            "before migration prompt",
            r.product
        )
        .execute(pool.as_ref())
        .await
        {
            log::error!(
                "Error inserting into niche table from product table: {:?}",
                e
            );
        }
    }

    let rows = sqlx::query_as!(
        EmailRow,
        r#"select
            email_address,
            verified_status as "verified_status: EmailVerifiedStatus",
            reachability as "reachability: EmailReachability"
        from
            email_old
        "#
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap();

    for r in rows {
        let status: VerificationStatus = r.verified_status.into();
        let reach: Reachability = r.reachability.into();

        if let Err(e) = sqlx::query!(
            r"
            insert into email
                (email_address, verification_status, reachability, founder_name, domain)
            values
                ($1, $2, $3, 'before-migration-founder-name', 'before-migration-domain')
            ",
            r.email_address,
            status as VerificationStatus,
            reach as Reachability,
        )
        .execute(pool.as_ref())
        .await
        {
            log::error!("Error inserting into new email table from old: {:?}", e);
        }
    }

    HttpResponse::Ok().body("Done")
}
