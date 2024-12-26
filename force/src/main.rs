use std::{net::TcpListener, time::Duration};

use actix_web::web;
use crossbeam::channel::unbounded;
use env_logger::Env;
use force::{
    configuration::get_configuration,
    services::{
        data_persistance_handler, domain_scraper_handler, email_verified_handler,
        founder_scraper_handler, FounderQueryChannelData, OpenaiClient, PersistantData,
        ProductQuerySender, Sentinel,
    },
    startup::run,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let configuration = get_configuration().expect("Failed to read configuration.");

    let pool_options = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(15 * 60)) // 15 minutes
        .max_lifetime(None);

    let connection_pool = pool_options.connect_lazy_with(configuration.database.with_db());
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;
    let openai_client = OpenaiClient::new(configuration.api_keys.openai);
    let sentinel = Sentinel::new(configuration.api_keys.bulk_email_checker);
    let sentinel = web::Data::new(sentinel);

    let (product_query_sender, product_query_receiver) = unbounded::<String>();
    let (founder_query_sender, founder_query_receiver) = unbounded::<FounderQueryChannelData>();
    let (email_sender, email_receiver) = unbounded::<String>();
    let (persistant_data_sender, persistant_data_receiver) = unbounded::<PersistantData>();

    let product_query_sender = ProductQuerySender {
        sender: product_query_sender,
    };

    // Spawn tasks
    tokio::spawn(domain_scraper_handler(
        product_query_receiver,
        founder_query_sender,
        persistant_data_sender.clone(),
    ));
    tokio::spawn(founder_scraper_handler(
        founder_query_receiver,
        email_sender,
        persistant_data_sender,
    ));
    tokio::spawn(email_verified_handler(sentinel.clone(), email_receiver));
    tokio::spawn(data_persistance_handler(
        persistant_data_receiver,
        connection_pool.clone(),
    ));

    run(
        listener,
        connection_pool,
        openai_client,
        sentinel,
        product_query_sender,
    )?
    .await
}
