use sqlx::PgPool;

use super::lead_db::EmailVerifiedStatus;

pub struct DomainStat {
    pub niche: String,
    pub product: String,
    pub unique_domains: Option<i64>,
}

pub async fn get_domain_stats(pool: &PgPool) -> Result<Vec<DomainStat>, sqlx::Error> {
    let niches = sqlx::query_scalar!("select niche from product order by created_at desc limit 3")
        .fetch_all(pool)
        .await?;

    sqlx::query_as!(
        DomainStat,
        r#"
        select
            p.niche,
            p.product,
            count(distinct d.domain) as unique_domains
        from
            product p
            join domain d on d.product_id = p.id
        where
            p.niche = any($1)
        group by
            p.niche, p.product
        "#,
        &niches,
    )
    .fetch_all(pool)
    .await
}

pub struct FounderStat {
    pub niche: String,
    pub product: String,
    pub domain: String,
    pub unique_founders: Option<i64>,
}

pub async fn get_founder_stats(pool: &PgPool) -> Result<Vec<FounderStat>, sqlx::Error> {
    let niches = sqlx::query_scalar!("select niche from product order by created_at desc limit 3")
        .fetch_all(pool)
        .await?;

    sqlx::query_as!(
        FounderStat,
        r#"
        select
            p.niche,
            p.product,
            f.domain,
            count(distinct f.founder_name) as unique_founders
        from
            founder f
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        where
            p.niche = any($1)
        group by
            p.niche, p.product, f.domain
        "#,
        &niches,
    )
    .fetch_all(pool)
    .await
}

pub struct EmailStat {
    pub niche: String,
    pub product: String,
    pub domain: String,
    pub founder_name: Option<String>,
    pub verified_status: EmailVerifiedStatus,
    pub unique_emails: Option<i64>,
}

pub async fn get_email_stats(pool: &PgPool) -> Result<Vec<EmailStat>, sqlx::Error> {
    let niches = sqlx::query_scalar!("select niche from product order by created_at desc limit 3")
        .fetch_all(pool)
        .await?;

    sqlx::query_as!(
        EmailStat,
        r#"
        select
            p.niche,
            p.product,
            f.domain,
            f.founder_name,
            e.verified_status as "verified_status: EmailVerifiedStatus",
            count(distinct e.email_address) as unique_emails
        from
            email e
            join founder f on f.id = e.founder_id
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        where
            p.niche = any($1)
        group by
            p.niche, p.product, f.domain, f.founder_name, e.verified_status
        "#,
        &niches,
    )
    .fetch_all(pool)
    .await
}
