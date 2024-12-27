use sqlx::{postgres::PgQueryResult, PgConnection};

use crate::domain::email::Email;

pub async fn insert_email(con: &mut PgConnection, email: Email) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r"
        insert into email
            (email_address, verification_status, reachability, founder_name, domain)
        values
            ($1, 'PENDING', 'UNKNOWN', $2, $3)
        returning id
        ",
        email.email_address,
        email.founder_name,
        email.domain,
    )
    .fetch_one(&mut *con)
    .await
}

pub async fn update_email_verified(
    con: &mut PgConnection,
    email: String,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r"
        update email set
            reachability = 'SAFE',
            verification_status = 'VERIFIED' 
        where
            email_address = $1
        ",
        email,
    )
    .execute(con)
    .await
}
