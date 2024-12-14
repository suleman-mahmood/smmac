use sqlx::PgPool;

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
