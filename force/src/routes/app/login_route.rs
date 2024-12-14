use actix_web::{get, HttpResponse};
use askama::Template;

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {}

#[get("/login")]
async fn login() -> HttpResponse {
    HttpResponse::Ok().body(LoginTemplate {}.render().unwrap())
}
