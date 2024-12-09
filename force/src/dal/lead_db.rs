use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::lead_route::{
    FounderDomain, FounderDomainEmail, FounderElement, FounderTagCandidate,
};

pub async fn get_product_search_queries(
    niche: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            domain_search_url
        from
            product
        where
            niche = $1
        "#,
        niche,
    )
    .fetch_all(pool)
    .await
}

pub async fn insert_niche_products(
    products: Vec<String>,
    search_queries: Vec<String>,
    niche: &str,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    for (pro, search_query) in products.iter().zip(search_queries.iter()) {
        sqlx::query!(
            r#"
            insert into product
                (id, niche, product, domain_search_url)
            values
                ($1, $2, $3, $4)
            "#,
            Uuid::new_v4(),
            niche,
            pro,
            search_query,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn get_domains(product_url: &str, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let domains = sqlx::query_scalar!(
        r#"
        select
            distinct d.domain
        from
            domain d
            join product p on p.id = d.product_id
        where
            p.domain_search_url = $1
        "#,
        product_url,
    )
    .fetch_all(pool)
    .await?;

    Ok(domains.into_iter().flatten().collect())
}

pub async fn insert_domain_candidate_urls(
    domain_urls_list: Vec<String>,
    domains: Vec<Option<String>>,
    founders: Vec<Option<String>>,
    search_url: &str,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    let product_id = sqlx::query_scalar!(
        r#"
        select id from product where domain_search_url = $1
        "#,
        search_url
    )
    .fetch_optional(pool)
    .await?;

    if product_id.is_none() {
        log::error!("No row found in product for url: {}", search_url);
        return Ok(());
    }
    let product_id = product_id.unwrap();

    for ((domain_url, dom), foun) in domain_urls_list
        .iter()
        .zip(domains.iter())
        .zip(founders.iter())
    {
        sqlx::query!(
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
        .execute(pool)
        .await?; // TODO: Ignore error silently
    }
    Ok(())
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "ElementType", rename_all = "SCREAMING_SNAKE_CASE")]
enum ElementType {
    Span,
    HThree,
}
pub async fn get_founder_tags(
    domain: &str,
    pool: &PgPool,
) -> Result<Option<Vec<FounderElement>>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        select
            domain,
            element_content,
            element_type as "element_type: ElementType"
        from
            founder
        where
            domain = $1
        "#,
        domain,
    )
    .fetch_all(pool)
    .await?;

    let elements = rows
        .into_iter()
        .map(|r| match r.element_type {
            ElementType::Span => FounderElement::Span(r.element_content),
            ElementType::HThree => FounderElement::H3(r.element_content),
        })
        .collect();

    Ok(Some(elements))
}

pub async fn insert_founders(
    founder: FounderTagCandidate,
    names: Vec<Option<String>>,
    domain: &str,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
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

        sqlx::query!(
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
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn get_founder_domains(
    domains: Vec<String>,
    pool: &PgPool,
) -> Result<Vec<FounderDomain>, sqlx::Error> {
    let record = sqlx::query!(
        r#"
        select
            founder_name,
            domain
        from
            founder
        where
            domain = any($1) and
            domain is not null and
            founder_name is not null
        "#,
        &domains,
    )
    .fetch_all(pool)
    .await?;

    Ok(record
        .into_iter()
        .map(|row| FounderDomain {
            founder_name: row.founder_name.unwrap(),
            domain: row.domain.unwrap(),
        })
        .collect())
}

pub async fn get_raw_emails(
    founder_domain: FounderDomain,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
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
    .await
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "EmailVerifiedStatus", rename_all = "SCREAMING_SNAKE_CASE")]
enum EmailVerifiedStatus {
    Pending,
    Verified,
    Invalid,
}
pub async fn insert_emails(
    founder_domain_emails: Vec<FounderDomainEmail>,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    for fde in founder_domain_emails {
        let founder_id = sqlx::query_scalar!(
            r#"
            select id from founder where domain = $1 and founder_name = $2
            "#,
            fde.domain,
            fde.founder_name,
        )
        .fetch_optional(pool)
        .await?;

        if founder_id.is_none() {
            continue;
        }
        let founder_id = founder_id.unwrap();

        sqlx::query!(
            r#"
            insert into email
                (id, founder_id, email_address, verified_status)
            values
                ($1, $2, $3, $4)
            "#,
            Uuid::new_v4(),
            founder_id,
            fde.email,
            EmailVerifiedStatus::Pending as EmailVerifiedStatus,
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}
