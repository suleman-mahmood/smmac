use std::net::TcpListener;

use actix_web::{
    dev::Server,
    middleware::Logger,
    web::{self},
    App, HttpServer,
};
use sqlx::PgPool;

use crate::{
    routes::{default_route, experiment_route, lead_route},
    services::{Droid, OpenaiClient, Sentinel},
};

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    openai_client: OpenaiClient,
    droid: Droid,
    sentinel: Sentinel,
) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);
    let openai_client = web::Data::new(openai_client);
    let droid = web::Data::new(droid);
    let sentinel = web::Data::new(sentinel);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(default_route::default)
            .service(web::scope("/lead").service(lead_route::get_leads_from_niche))
            .service(
                web::scope("/exp")
                    .service(experiment_route::get_gpt_results)
                    .service(experiment_route::open_multiple_browsers)
                    .service(experiment_route::next_search)
                    .service(experiment_route::verify_emails)
                    .service(experiment_route::check_user_agent)
                    .service(experiment_route::check_ip_address)
                    .service(experiment_route::get_fake_emails)
                    .service(experiment_route::extract_domain_from_candidate_url),
            )
            .app_data(db_pool.clone())
            .app_data(openai_client.clone())
            .app_data(droid.clone())
            .app_data(sentinel.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
