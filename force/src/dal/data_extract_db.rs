use sqlx::PgConnection;

use crate::domain::data_extract::DataExtract;

#[derive(sqlx::Type)]
#[sqlx(type_name = "DataType", rename_all = "SCREAMING_SNAKE_CASE")]
enum DataType {
    Domain,
    FounderName,
}

pub async fn insert_data(
    con: &mut PgConnection,
    data: DataExtract,
    tag_id: i64,
) -> Result<i64, sqlx::Error> {
    let (content, data_type) = match data {
        DataExtract::Domain(content) => (content, DataType::Domain),
        DataExtract::FounderName(content) => (content, DataType::FounderName),
    };

    sqlx::query_scalar!(
        r"
        insert into data_extract
            (data, data_type, html_tag_id)
        values
            ($1, $2, $3)
        returning id
        ",
        content,
        data_type as DataType,
        tag_id,
    )
    .fetch_one(&mut *con)
    .await
}
