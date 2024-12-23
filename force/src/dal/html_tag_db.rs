use sqlx::{PgConnection, PgPool};

use crate::domain::html_tag::HtmlTag;

pub async fn insert_html_tag(con: &mut PgConnection, html_tag: HtmlTag, web_page_id: i64) {
    todo!()
}
