use sqlx::{postgres::PgQueryResult, PgConnection};

use crate::domain::smart_scout::SmartScout;

#[derive(sqlx::Type)]
#[sqlx(type_name = "SmartScoutJobStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SmartScoutJobStatus {
    Started,
    Completed,
    Failed,
}

pub async fn start_job(
    con: &mut PgConnection,
    smart_scout_id: i64,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r"
        insert into smart_scout_job
            (smart_scout_id, status)
        values
            ($1, 'STARTED')
        ",
        smart_scout_id
    )
    .execute(con)
    .await
}

pub async fn finish_job(
    con: &mut PgConnection,
    smart_scout_id: i64,
    status: SmartScoutJobStatus,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r"
        update smart_scout_job set
            status = $2
        where
            smart_scout_id = $1
        ",
        smart_scout_id,
        status as SmartScoutJobStatus,
    )
    .execute(con)
    .await
}

pub async fn get_n_unscraped_company_ids(
    con: &mut PgConnection,
    n: i64,
) -> Result<Vec<SmartScout>, sqlx::Error> {
    let rows = sqlx::query!(
        r"
        with unscraped_ids as (
            select
                distinct public_id
            from
                smart_scout

            except

            select
                distinct ss.public_id
            from
                smart_scout ss
                join smart_scout_job ssj on ssj.smart_scout_id = ss.id
        )
        select
            ss.id,
            ss.name
        from
            smart_scout ss
            join unscraped_ids ui on ui.public_id = ss.public_id
        where
            ss.name is not null
        order by random()
        limit $1
        ",
        n,
    )
    .fetch_all(con)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SmartScout {
            id: r.id,
            name: r.name.unwrap(),
        })
        .collect())
}
