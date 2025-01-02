use sqlx::PgConnection;

use crate::domain::smart_scout::SmartScout;

#[derive(sqlx::Type)]
#[sqlx(type_name = "SmartScoutJobStatus", rename_all = "SCREAMING_SNAKE_CASE")]
enum SmartScoutJobStatus {
    Started,
    Completed,
    Failed,
}

pub async fn start_job(con: &mut PgConnection, smart_scout_id: i64) -> Result<bool, sqlx::Error> {
    todo!()

    // sqlx::query_scalar!(
    //     r"
    //     select smart_scout_id from smart_scout_job where smart_scout_id = $1
    //     ",
    //     smart_scout_id,
    // )
    // .fetch_optional(&mut *con)
    // .await?;
    //
    // Ok(true)
}

pub async fn get_n_unscraped_company_ids(
    con: &mut PgConnection,
    n: i64,
) -> Result<Vec<SmartScout>, sqlx::Error> {
    todo!()
}
