use std::fmt::Display;

use check_if_email_exists::Reachable;
use serde::Deserialize;
use sqlx::{postgres::PgQueryResult, PgConnection, PgPool};
use uuid::Uuid;

use crate::routes::lead_route::{
    FounderDomain, FounderDomainEmail, FounderElement, FounderTagCandidate, FRESH_RESULTS,
};

pub async fn get_product_search_queries(
    niche: &str,
    pool: &PgPool,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let rows = sqlx::query_scalar!(
        r#"
        select
            domain_search_url
        from
            product
        where
            niche = $1 and
            no_results = false
        "#,
        niche,
    )
    .fetch_all(pool)
    .await?;

    match rows.is_empty() {
        true => Ok(None),
        false => Ok(Some(rows)),
    }
}

pub async fn insert_niche_products(
    products: Vec<String>,
    search_queries: Vec<String>,
    niche: &str,
    pool: &PgPool,
) -> Result<PgQueryResult, sqlx::Error> {
    let existing_products =
        sqlx::query_scalar!("select product from product where niche = $1", niche)
            .fetch_all(pool)
            .await
            .unwrap_or(vec![]);

    let p_and_q: Vec<(String, String)> = products
        .into_iter()
        .zip(search_queries.into_iter())
        .filter(|(pro, _)| !existing_products.contains(pro))
        .collect();
    let products: Vec<String> = p_and_q.iter().map(|(p, _)| p.to_string()).collect();
    let search_queries: Vec<String> = p_and_q.iter().map(|(_, q)| q.to_string()).collect();

    let total_rows = p_and_q.len();
    let ids: Vec<Uuid> = (0..total_rows).map(|_| Uuid::new_v4()).collect();
    let niche: Vec<String> = (0..total_rows).map(|_| niche.to_string()).collect();

    sqlx::query!(
        r#"
        insert into product
            (id, niche, product, domain_search_url)
        select * from unnest (
            $1::uuid[],
            $2::text[],
            $3::text[],
            $4::text[]
        )
        "#,
        &ids,
        &niche,
        &products,
        &search_queries
    )
    .execute(pool)
    .await
}

pub async fn get_domains_for_niche(niche: &str, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let domains = sqlx::query_scalar!(
        r#"
        select
            distinct d.domain
        from
            domain d
            join product p on p.id = d.product_id
        where
            p.niche = $1 and
            d.domain is not null
        "#,
        niche,
    )
    .fetch_all(pool)
    .await?;

    let domains: Vec<String> = domains.into_iter().flatten().collect();
    Ok(domains)
}

pub async fn get_unscraped_products(
    niche: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    let products = sqlx::query_scalar!(
        r#"
        select
            domain_search_url
        from
            product
        where
            niche = $1 and
            no_results = False
        "#,
        niche
    )
    .fetch_all(pool)
    .await?;

    if FRESH_RESULTS {
        return Ok(products);
    }

    let scraped_products = sqlx::query_scalar!(
        r#"
        select
            distinct p.domain_search_url
        from
            domain d
            join product p on p.id = d.product_id
        where
            p.niche = $1
        "#,
        niche
    )
    .fetch_all(pool)
    .await?;

    let products = products
        .into_iter()
        .filter(|p| !scraped_products.contains(p))
        .collect();

    Ok(products)
}

pub async fn insert_domain_candidate_urls(
    domain_urls_list: Vec<String>,
    domains: Vec<Option<String>>,
    founders: Vec<Option<String>>,
    search_url: &str,
    not_found: bool,
    con: &mut PgConnection,
) -> Result<(), sqlx::Error> {
    let product_id = sqlx::query_scalar!(
        r#"
        select id from product where domain_search_url = $1
        "#,
        search_url
    )
    .fetch_optional(&mut *con)
    .await?;

    if product_id.is_none() {
        log::error!("No row found in product for url: {}", search_url);
        return Ok(());
    }
    let product_id = product_id.unwrap();

    if not_found {
        sqlx::query!(
            "update product set no_results = true where domain_search_url = $1",
            search_url,
        )
        .execute(con)
        .await?;
        return Ok(());
    }

    for ((domain_url, dom), foun) in domain_urls_list
        .iter()
        .zip(domains.iter())
        .zip(founders.iter())
    {
        _ = sqlx::query!(
            r#"
            insert into domain
                (id, product_id, domain_candidate_url, domain, founder_search_url)
            values
                ($1, $2, $3, $4, $5)
            "#,
            Uuid::new_v4(),
            product_id,
            domain_url,
            dom.clone(),
            foun.clone()
        )
        .execute(&mut *con)
        .await;
    }
    Ok(())
}

pub async fn get_unscraped_domains(
    domains: Vec<String>,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    let scraped_domains = sqlx::query_scalar!(
        r#"
        select domain from founder where domain = any($1)
        "#,
        &domains
    )
    .fetch_all(pool)
    .await?;

    let domains = domains
        .into_iter()
        .filter(|d| !scraped_domains.contains(d))
        .collect();

    Ok(domains)
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ElementType", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ElementType {
    Span,
    HThree,
}

pub async fn insert_domain_no_results(
    domain: &str,
    con: &mut PgConnection,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r#"
        insert into founder
            (id, domain, element_content, element_type, founder_name, no_results)
        values
            ($1, $2, $3, $4, $5, $6)
        "#,
        Uuid::new_v4(),
        domain,
        "no-content",
        ElementType::Span as ElementType,
        "no-content",
        true,
    )
    .execute(con)
    .await
}

pub async fn insert_founders(
    founder: FounderTagCandidate,
    names: Vec<Option<String>>,
    domain: &str,
    con: &mut PgConnection,
) {
    for (ele, name) in founder.elements.into_iter().zip(names.into_iter()) {
        let content;
        let element_type = match ele {
            FounderElement::Span(c) => {
                content = c;
                ElementType::Span
            }
            FounderElement::H3(c) => {
                content = c;
                ElementType::HThree
            }
        };

        _ = sqlx::query!(
            r#"
            insert into founder
                (id, domain, element_content, element_type, founder_name)
            values
                ($1, $2, $3, $4, $5)
            "#,
            Uuid::new_v4(),
            domain,
            content,
            element_type as ElementType,
            name,
        )
        .execute(&mut *con)
        .await;
    }
}

pub async fn get_founder_domains(
    domains: Vec<String>,
    pool: &PgPool,
) -> Result<Option<Vec<FounderDomain>>, sqlx::Error> {
    let records = sqlx::query!(
        r#"
        select
            founder_name,
            domain
        from
            founder
        where
            domain = any($1) and
            founder_name is not null
        "#,
        &domains,
    )
    .fetch_all(pool)
    .await?;

    match records.is_empty() {
        true => Ok(None),
        false => {
            let mut fds = Vec::new();
            records.into_iter().for_each(|row| {
                if let Some(name) = row.founder_name {
                    fds.push(FounderDomain {
                        founder_name: name,
                        domain: row.domain,
                    });
                }
            });
            Ok(Some(fds))
        }
    }
}

pub async fn get_raw_emails(
    founder_domain: FounderDomain,
    pool: &PgPool,
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let emails = sqlx::query_scalar!(
        r#"
        select
            e.email_address
        from
            email e
            join founder f on f.id = e.founder_id
        where
            f.domain = $1 and
            f.founder_name = $2
        "#,
        founder_domain.domain,
        founder_domain.founder_name,
    )
    .fetch_all(pool)
    .await?;

    match emails.is_empty() {
        true => Ok(None),
        false => Ok(Some(emails)),
    }
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "EmailVerifiedStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmailVerifiedStatus {
    Pending,
    Verified,
    Invalid,
}

impl Display for EmailVerifiedStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailVerifiedStatus::Pending => write!(f, "Pending"),
            EmailVerifiedStatus::Verified => write!(f, "Verified"),
            EmailVerifiedStatus::Invalid => write!(f, "Invalid"),
        }
    }
}

pub async fn insert_emails(founder_domain_emails: Vec<FounderDomainEmail>, pool: &PgPool) {
    for fde in founder_domain_emails {
        let founder_id = sqlx::query_scalar!(
            r#"
            select id from founder where domain = $1 and founder_name = $2
            "#,
            fde.domain,
            fde.founder_name,
        )
        .fetch_optional(pool)
        .await;

        if let Ok(Some(founder_id)) = founder_id {
            _ = sqlx::query!(
                r#"
                insert into email
                    (id, founder_id, email_address, verified_status, reachability)
                values
                    ($1, $2, $3, $4, $5)
                "#,
                Uuid::new_v4(),
                founder_id,
                fde.email,
                EmailVerifiedStatus::Pending as EmailVerifiedStatus,
                EmailReachability::Unknown as EmailReachability,
            )
            .execute(pool)
            .await;
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "Reachability", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmailReachability {
    Safe,
    Unknown,
    Risky,
    Invalid,
}

impl Display for EmailReachability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmailReachability::Safe => write!(f, "Safe"),
            EmailReachability::Unknown => write!(f, "Unknown"),
            EmailReachability::Risky => write!(f, "Risky"),
            EmailReachability::Invalid => write!(f, "Invalid"),
        }
    }
}

impl From<Reachable> for EmailReachability {
    fn from(value: Reachable) -> Self {
        match value {
            Reachable::Safe => EmailReachability::Safe,
            Reachable::Unknown => EmailReachability::Unknown,
            Reachable::Risky => EmailReachability::Risky,
            Reachable::Invalid => EmailReachability::Invalid,
        }
    }
}

pub async fn set_email_verification_reachability(
    email: &str,
    status: EmailVerifiedStatus,
    reachability: EmailReachability,
    con: &mut PgConnection,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r#"
        update email set
            reachability = $2,
            verified_status = $3
        where
            email_address = $1
        "#,
        email,
        reachability as EmailReachability,
        status as EmailVerifiedStatus,
    )
    .execute(con)
    .await
}

pub async fn get_verified_emails_for_niche(
    niche: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            distinct e.email_address
        from
            email e
            join founder f on f.id = e.founder_id
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        where
            p.niche = $1 and
            e.verified_status = 'VERIFIED'
        "#,
        niche
    )
    .fetch_all(pool)
    .await
}

pub async fn get_raw_pending_emails_for_niche(
    niche: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            e.email_address
        from
            email e
            join founder f on f.id = e.founder_id
            join domain d on d.domain = f.domain
            join product p on p.id = d.product_id
        where
            p.niche = $1 and
            e.verified_status = 'PENDING'
        "#,
        niche,
    )
    .fetch_all(pool)
    .await
}
