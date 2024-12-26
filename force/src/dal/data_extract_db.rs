use sqlx::PgConnection;

use crate::domain::data_extract::DataExtract;

pub async fn insert_data(
    con: &mut PgConnection,
    data: DataExtract,
    tag_id: i64,
) -> Result<i64, sqlx::Error> {
    todo!()
}
