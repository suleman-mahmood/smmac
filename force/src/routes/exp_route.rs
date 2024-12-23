use actix_web::{get, web, HttpResponse};

use crate::services::DomainScraperSender;

#[get("/check-channel-works")]
async fn check_channel_works(
    domain_scraper_sender: web::Data<DomainScraperSender>,
) -> HttpResponse {
    let domain_scraper_sender = domain_scraper_sender.sender.clone();
    ["pro 1", "pro 2", "pro 999"].iter().for_each(|q| {
        match domain_scraper_sender.send(q.to_string()) {
            Ok(_) => {}
            Err(e) => log::error!("Found error while sending: {:?}", e),
        }
    });

    HttpResponse::Ok().body("Done")
}
