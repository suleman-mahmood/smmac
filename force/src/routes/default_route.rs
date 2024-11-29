use actix_web::{get, HttpResponse, Responder};

#[get("/")]
async fn default() -> impl Responder {
    HttpResponse::Ok().body("Hi there, Darth Vader!")
}
