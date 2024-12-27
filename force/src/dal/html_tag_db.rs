use sqlx::PgConnection;

use crate::domain::html_tag::HtmlTag;

#[derive(sqlx::Type)]
#[sqlx(type_name = "HtmlTagType", rename_all = "SCREAMING_SNAKE_CASE")]
enum HtmlTagType {
    ATag,
    H3Tag,
    SpanTag,
    NextPageATag,
}

pub async fn insert_html_tag(
    con: &mut PgConnection,
    html_tag: HtmlTag,
    web_page_id: i64,
) -> Result<i64, sqlx::Error> {
    let (content, tag_type) = match html_tag {
        HtmlTag::ATag(content) => (content, HtmlTagType::ATag),
        HtmlTag::H3Tag(content) => (content, HtmlTagType::H3Tag),
        HtmlTag::SpanTag(content) => (content, HtmlTagType::SpanTag),
        HtmlTag::NextPageATag(content) => (content, HtmlTagType::NextPageATag),
    };

    sqlx::query_scalar!(
        r"
        insert into html_tag
            (text_content, html_tag_type, google_webpage_id)
        values
            ($1, $2, $3)
        returning id
        ",
        content,
        tag_type as HtmlTagType,
        web_page_id,
    )
    .fetch_one(&mut *con)
    .await
}
