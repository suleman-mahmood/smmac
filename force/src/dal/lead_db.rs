use sqlx::PgPool;
use uuid::Uuid;

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

    for domain_url in domain_urls_list {
        sqlx::query!(
            r#"
            insert into domain
                (id, product_id, domain_candidate_url)
            values
                ($1, $2, $3)
            "#,
            Uuid::new_v4(),
            product_id,
            domain_url,
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}
