use std::collections::HashSet;

use sqlx::PgPool;

use crate::domain::niche::Niche;

pub async fn get_niche(pool: &PgPool, niche: &str) -> Result<Niche, sqlx::Error> {
    let rows = sqlx::query!(
        r"
        select
            gippity_prompt,
            generated_product
        from
            niche
        where
            user_niche = $1
        ",
        niche
    )
    .fetch_all(pool)
    .await?;

    match rows.is_empty() {
        true => Err(sqlx::Error::RowNotFound),
        false => Ok(Niche {
            user_niche: niche.to_string(),
            gippity_prompt: rows.first().unwrap().gippity_prompt.clone(),
            generated_products: rows.into_iter().map(|r| r.generated_product).collect(),
        }),
    }
}

pub async fn insert_niche(
    pool: &PgPool,
    niche: &str,
    gippity_prompt: &str,
    generated_products: Vec<String>,
) -> Result<(), sqlx::Error> {
    let existing_products = sqlx::query_scalar!(
        "select generated_product from niche where user_niche = $1",
        niche
    )
    .fetch_all(pool)
    .await
    .unwrap_or(vec![]);

    let existing_products: HashSet<String> = HashSet::from_iter(existing_products);
    let new_products = HashSet::from_iter(generated_products);
    let new_products: Vec<String> = new_products
        .difference(&existing_products)
        .map(|p| p.to_string())
        .collect();

    let total_rows = new_products.len();
    let gippity_prompts: Vec<String> = (0..total_rows)
        .map(|_| gippity_prompt.to_string())
        .collect();
    let niches: Vec<String> = (0..total_rows).map(|_| niche.to_string()).collect();

    sqlx::query!(
        r#"
        insert into niche
            (user_niche, gippity_prompt, generated_product)
        select * from unnest (
            $1::text[],
            $2::text[],
            $3::text[]
        )
        "#,
        &niches,
        &gippity_prompts,
        &new_products
    )
    .execute(pool)
    .await?;

    Ok(())
}
