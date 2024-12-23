use std::{net::TcpListener, time::Duration};

use crossbeam::channel::unbounded;
use env_logger::Env;
use force::{
    configuration::get_configuration,
    services::{domain_scraper_handler, OpenaiClient, ProductQuerySender, Sentinel},
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

    let (product_query_sender, product_query_receiver) = unbounded::<String>();
    let product_query_sender = ProductQuerySender {
        sender: product_query_sender,
    };

    // Spawn tasks
    tokio::spawn(domain_scraper_handler(product_query_receiver));

    run(
        listener,
        connection_pool,
        openai_client,
        sentinel,
        product_query_sender,
    )?
    .await
}
