use sqlx::{types::chrono, PgPool};

use super::lead_db::{EmailReachability, EmailVerifiedStatus};

pub struct ProductRow {
    pub niche: String,
    pub product: String,
    pub no_results: bool,
}

pub async fn get_product_table(pool: &PgPool) -> Result<Vec<ProductRow>, sqlx::Error> {
    sqlx::query_as!(
        ProductRow,
        r#"
        select
            niche,
            product,
            no_results
        from
            product
        order by created_at desc
        "#,
    )
    .fetch_all(pool)
    .await
}

pub struct DomainRow {
    pub niche: String,
    pub product: String,
    pub domain_candidate_url: String,
    pub domain: Option<String>,
}

pub async fn get_domain_table(pool: &PgPool) -> Result<Vec<DomainRow>, sqlx::Error> {
    sqlx::query_as!(
        DomainRow,
        r#"
        select
            p.niche,
            p.product,
            d.domain_candidate_url,
            d.domain
        from
            domain d
            join product p on p.id = d.product_id
        order by d.created_at desc
        "#,
    )
    .fetch_all(pool)
    .await
}

pub struct FounderRow {
    pub niche: String,
    pub product: String,
    pub domain: String,
    pub element_content: String,
    pub founder_name: Option<String>,
    pub no_results: bool,
}

pub async fn get_founder_table(pool: &PgPool) -> Result<Vec<FounderRow>, sqlx::Error> {
    sqlx::query_as!(
        FounderRow,
        r#"
        select
            p.niche,
            p.product,
            f.domain,
            f.element_content,
            f.founder_name,
            f.no_results
        from
            founder f
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        order by f.created_at desc
        "#,
    )
    .fetch_all(pool)
    .await
}

pub struct EmailRow {
    pub niche: String,
    pub product: String,
    pub domain: String,
    pub founder_name: Option<String>,
    pub email_address: String,
    pub verified_status: EmailVerifiedStatus,
    pub reachability: EmailReachability,
}

pub async fn get_email_table(pool: &PgPool) -> Result<Vec<EmailRow>, sqlx::Error> {
    sqlx::query_as!(
        EmailRow,
        r#"
        select
            p.niche,
            p.product,
            f.domain,
            f.founder_name,
            e.email_address,
            e.verified_status as "verified_status: EmailVerifiedStatus",
            e.reachability as "reachability: EmailReachability"
        from
            email e
            join founder f on f.id = e.founder_id
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        order by e.created_at desc
        "#,
    )
    .fetch_all(pool)
    .await
}

pub struct VerifiedEmailRow {
    pub email: String,
    pub founder_name: Option<String>,
    pub domain: Option<String>,
    pub product: Option<String>,
    pub niche: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn get_verified_emails(pool: &PgPool) -> Result<Vec<VerifiedEmailRow>, sqlx::Error> {
    sqlx::query_as!(
        VerifiedEmailRow,
        r#"
        with filtered_emails as (
            select
                email_address
            from
                email
            where
                verified_status = 'VERIFIED'

            except

            select
                distinct unnest(array_agg(e.email_address))
            from
                email e
                join founder f on f.id = e.founder_id
                join domain d on d.domain = f.domain
                join product p on p.id = d.product_id
            where
                e.verified_status = 'VERIFIED'
            group by
                f.domain, f.founder_name
            having
                count(distinct e.email_address) > 2
        )
        select
            e.email_address as email,
            (array_agg(f.founder_name))[1] as founder_name,
            (array_agg(f.domain))[1] as domain,
            (array_agg(p.product))[1] as product,
            (array_agg(p.niche))[1] as niche,
            (array_agg(e.created_at))[1] as created_at
        from
            filtered_emails fe
            join email e on e.email_address = fe.email_address
            join founder f on f.id = e.founder_id
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        group by
            e.email_address
        order by
            6 desc
        "#
    )
    .fetch_all(pool)
    .await
}
