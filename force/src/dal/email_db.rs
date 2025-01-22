use sqlx::{postgres::PgQueryResult, PgConnection, PgPool};

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

pub async fn update_email_unverified(
    con: &mut PgConnection,
    email: String,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query!(
        r"
        update email set
            reachability = 'INVALID',
            verification_status = 'INVALID'
        where
            email_address = $1
        ",
        email,
    )
    .execute(con)
    .await
}

pub async fn get_verified_emails_for_niche(
    pool: &PgPool,
    niche: &str,
    count: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        r"
        with filtered_emails as (
            select
                email_address
            from
                email
            where
                verification_status = 'VERIFIED'

            except

            select
                distinct unnest(array_agg(e.email_address))
            from
                email e
            where
                e.verification_status = 'VERIFIED'
            group by
                e.domain, e.founder_name
            having
                count(distinct e.email_address) > 2
        )
        select
            e.email_address as email
        from
            filtered_emails fe
            join email e on e.email_address = fe.email_address
            join data_extract ded on ded.data = e.domain and ded.data_type = 'DOMAIN'
            join data_extract def on def.data = e.founder_name and def.data_type = 'FOUNDER_NAME'
            join html_tag ht on ht.id in (ded.html_tag_id, def.html_tag_id)
            join google_webpage gw on gw.id = ht.google_webpage_id
            join niche n on n.generated_product = gw.search_query
        where
            n.user_niche = $1 and
            e.verification_status = 'VERIFIED'
        group by
            e.email_address
        limit $2
        ",
        niche,
        count
    )
    .fetch_all(pool)
    .await
}
