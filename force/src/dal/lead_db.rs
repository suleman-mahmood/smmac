use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::routes::lead_route::{FounderElement, FounderTagCandidate};

pub async fn get_product_search_queries(
    niche: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            domain_boolean_search
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
                (id, niche, product, domain_boolean_search)
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

pub async fn get_domain_candidate_urls_for_product(
    product_url: &str,
    pool: &PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            d.domain_candidate_url
        from
            domain d
            join product p on p.id = d.product_id
        where
            p.domain_boolean_search = $1
        "#,
        product_url,
    )
    .fetch_all(pool)
    .await
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
        select id from product where domain_boolean_search = $1
        "#,
        search_url
    )
    .fetch_optional(pool)
    .await?;

    if product_id.is_none() {
        log::error!("No row found in product for url: {}", search_url);
        return Ok(());
    }

    for ((domain_url, dom), foun) in domain_urls_list
        .iter()
        .zip(domains.iter())
        .zip(founders.iter())
    {
        sqlx::query!(
            r#"
            insert into domain
                (id, product_id, domain_candidate_url, domain, founder_boolean_search)
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
        .await?;
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
    domain: &str,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    for ele in founder.elements {
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
                (id, domain, element_content, element_type)
            values
                ($1, $2, $3, $4)
            "#,
            Uuid::new_v4(),
            domain,
            content,
            element_type as ElementType,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}
