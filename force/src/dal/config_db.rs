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
            insert into configuration
                (key, value)
            values
                ('chatgpt-products-for-niche-start', $1)
            on conflict(key) do update set
                value = $1
            "#,
            start
        )
        .execute(pool)
        .await?;
    }

    if let Some(end) = end {
        sqlx::query!(
            r#"
            insert into configuration
                (key, value)
            values
                ('chatgpt-products-for-niche-end', $1)
            on conflict(key) do update set
                value = $1
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
        insert into configuration
            (key, value)
        values
            ('google-search-domain-page-depth', $1)
        on conflict(key) do update set
            value = $1
        "#,
        depth.to_string()
    )
    .execute(pool)
    .await
}
