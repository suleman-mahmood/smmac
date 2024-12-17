use sqlx::{postgres::PgQueryResult, PgPool};

pub async fn get_gippity_prompt(
    pool: &PgPool,
) -> Result<(Option<String>, Option<String>), sqlx::Error> {
    let start = sqlx::query_scalar!(
        r#"
        select
            value
        from
            configuration
        where
            key = 'chatgpt-products-for-niche-start'
        "#,
    )
    .fetch_optional(pool)
    .await?;

    let end = sqlx::query_scalar!(
        r#"
        select
            value
        from
            configuration
        where
            key = 'chatgpt-products-for-niche-end'
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok((start, end))
}

pub async fn set_gippity_prompt(
    start: Option<&str>,
    end: Option<&str>,
    pool: &PgPool,
) -> Result<(), sqlx::Error> {
    if let Some(start) = start {
        sqlx::query!(
            r#"
        update configuration set
            value = $1
        where
            key = 'chatgpt-products-for-niche-start'
        "#,
            start
        )
        .execute(pool)
        .await?;
    }

    if let Some(end) = end {
        sqlx::query!(
            r#"
        update configuration set
            value = $1
        where
            key = 'chatgpt-products-for-niche-end'
        "#,
            end
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn get_google_search_page_depth(pool: &PgPool) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        select
            value
        from
            configuration
        where
            key = 'google-search-domain-page-depth'
        "#,
    )
    .fetch_optional(pool)
    .await
}

pub async fn set_google_search_page_depth(
    depth: u8,
    pool: &PgPool,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r#"
        update configuration set
            value = $1
        where
            key = 'google-search-domain-page-depth'
        "#,
        depth.to_string()
    )
    .execute(pool)
    .await
}
