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
