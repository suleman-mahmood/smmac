use std::collections::HashSet;

use sqlx::{postgres::PgQueryResult, PgPool};

use crate::domain::google_webpage::{DataExtractionIntent, GoogleWebPage};

pub async fn insert_web_page(
    pool: &PgPool,
    webpage: GoogleWebPage,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r"
        insert into google_webpage
            (search_query, page_source, page_number, data_extraction_intent, any_result)
        values
            ($1, $2, $3, $4, $5)
        ",
        webpage.search_query,
        webpage.page_source,
        i32::from(webpage.page_number),
        webpage.data_extraction_intent as DataExtractionIntent,
        webpage.any_result,
    )
    .execute(pool)
    .await
}

async fn filter_unscraped_product_queries(
    pool: &PgPool,
    queries: Vec<String>,
) -> Result<Vec<String>, sqlx::Error> {
    let existing_queries = sqlx::query_scalar!(
        r"
        select
            search_query
        from
            google_webpage
        where
            search_query = any($1)
        ",
        &queries,
    )
    .fetch_all(pool)
    .await?;

    let existing_queries: HashSet<String> = HashSet::from_iter(existing_queries);
    let queries = HashSet::from_iter(queries);

    Ok(queries
        .difference(&existing_queries)
        .map(|q| q.to_string())
        .collect())
}
