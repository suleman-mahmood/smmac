use std::net::TcpListener;

use env_logger::Env;
use force::{
    configuration::get_configuration,
    services::{Droid, OpenaiClient, Sentinel},
    startup::run,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let configuration = get_configuration().expect("Failed to read configuration.");
    let connection_pool = PgPoolOptions::new().connect_lazy_with(configuration.database.with_db());
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;
    let openai_client = OpenaiClient::new(configuration.api_keys.openai);
    let droid = Droid::new().await;
    let sentinel = Sentinel::new(configuration.api_keys.bulk_email_checker);

    run(listener, connection_pool, openai_client, droid, sentinel)?.await
}
