use std::net::TcpListener;

use actix_web::{
    dev::Server,
    middleware::Logger,
    web::{self},
    App, HttpServer,
};
use sqlx::PgPool;

use crate::{
    routes::{default_route, lead_route},
    services::OpenaiClient,
};

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    openai_client: OpenaiClient,
) -> Result<Server, std::io::Error> {
    let db_pool = web::Data::new(db_pool);
    let openai_client = web::Data::new(openai_client);

    log::info!("{:?}", std::env::var("OPENAI_API_KEY"));

    let server = HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .service(default_route::default)
            .service(web::scope("/lead").service(lead_route::get_leads_from_niche))
            .app_data(db_pool.clone())
            .app_data(openai_client.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
