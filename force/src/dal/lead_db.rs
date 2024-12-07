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
