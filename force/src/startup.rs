use std::net::TcpListener;

use actix_files::Files;
use actix_web::{
    dev::Server,
    middleware::Logger,
    web::{self, Data},
    App, HttpServer,
};
use sqlx::PgPool;

use crate::{
    routes::{
        dashboard_route, default_route, domain_route, email_route, exp_route, founder_route,
        lead_route, login_route, product_route, verified_email_route,
    },
    services::{OpenaiClient, ProductQuerySender, Sentinel},
};

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    openai_client: OpenaiClient,
    sentinel: Data<Sentinel>,
    product_query_sender: ProductQuerySender,
) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);
    let openai_client = web::Data::new(openai_client);
    let product_query_sender = web::Data::new(product_query_sender);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(Files::new("/static", "./templates/static").prefer_utf8(true))
            .service(default_route::default)
            .service(web::scope("/lead").service(lead_route::get_leads_from_niche))
            .service(web::scope("/lightning").service(lead_route::get_leads_from_niche))
            .service(web::scope("/exp").service(exp_route::check_channel_works))
            // .service(
            //     web::scope("/exp")
            // .service(experiment_route::get_gpt_results)
            // .service(experiment_route::open_multiple_browsers)
            // .service(experiment_route::next_search)
            // .service(experiment_route::verify_emails)
            // .service(experiment_route::check_user_agent)
            // .service(experiment_route::check_ip_address)
            // .service(experiment_route::get_fake_emails)
            // .service(experiment_route::extract_domain_from_candidate_url)
            // .service(experiment_route::recalculate_founder_names)
            // .service(experiment_route::get_valid_founder_names)
            // .service(experiment_route::verify_email)
            // .service(experiment_route::emails_step)
            // .service(experiment_route::no_driver_scrape)
            // .service(experiment_route::insert_bulk_products)
            // .service(experiment_route::check_ip_address_request),
            // )
            .service(
                web::scope("/app")
                    .service(login_route::login)
                    .service(domain_route::domain)
                    .service(founder_route::founder)
                    .service(email_route::email)
                    .service(product_route::product)
                    .service(verified_email_route::verified_email)
                    .service(dashboard_route::dashboard)
                    .service(dashboard_route::set_config),
            )
            .app_data(db_pool.clone())
            .app_data(openai_client.clone())
            .app_data(sentinel.clone())
            .app_data(product_query_sender.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
