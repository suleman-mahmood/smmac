use std::{net::TcpListener, time::Duration};

use actix_web::web;
use env_logger::Env;
use force::{
    configuration::get_configuration,
    domain::email::FounderDomainEmail,
    services::{
        data_persistance_handler, domain_scraper_handler, email_verified_handler,
        founder_scraper_handler, smart_scout_scraper_handler, EmailVerifierSender,
        FounderQueryChannelData, OpenaiClient, PersistantData, PersistantDataSender,
        ProductQuerySender, Sentinel, VerifiedEmailReceiver,
    },
    startup::run,
};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::{self, mpsc};

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

    let (product_query_sender, product_query_receiver) = mpsc::unbounded_channel::<String>();
    let (founder_query_sender, founder_query_receiver) =
        mpsc::unbounded_channel::<FounderQueryChannelData>();
    let (email_sender, email_receiver) = mpsc::unbounded_channel::<FounderDomainEmail>();
    let (persistant_data_sender, persistant_data_receiver) =
        mpsc::unbounded_channel::<PersistantData>();
    let (verified_email_sender, verified_email_receiver) =
        sync::broadcast::channel::<String>(10_000);
    drop(verified_email_receiver); // TODO: Remove this?

    let product_query_sender = ProductQuerySender {
        sender: product_query_sender,
    };
    let verified_email_receiver = VerifiedEmailReceiver {
        sender: verified_email_sender.clone(),
    };
    let email_verifier_sender = EmailVerifierSender {
        sender: email_sender.clone(),
    };

    // Spawn backgound tasks
    let pers_data_clone = persistant_data_sender.clone();
    let fou_q_clone = founder_query_sender.clone();
    tokio::spawn(async move {
        domain_scraper_handler(product_query_receiver, fou_q_clone, pers_data_clone).await
    });

    let pers_data_clone = persistant_data_sender.clone();
    tokio::spawn(async move {
        founder_scraper_handler(founder_query_receiver, email_sender, pers_data_clone).await
    });

    let sent_clone = sentinel.clone();
    let pers_data_clone = persistant_data_sender.clone();
    tokio::spawn(async move {
        email_verified_handler(
            sent_clone,
            email_receiver,
            pers_data_clone,
            verified_email_sender,
        )
        .await
    });

    let pool_clone = connection_pool.clone();
    let pers_data_clone = persistant_data_sender.clone();
    tokio::spawn(async move {
        data_persistance_handler(persistant_data_receiver, pers_data_clone, pool_clone).await
    });

    let pool_clone = connection_pool.clone();
    tokio::spawn(async move {
        smart_scout_scraper_handler(pool_clone, founder_query_sender, persistant_data_sender).await
    });

    run(
        listener,
        connection_pool,
        openai_client,
        sentinel,
        product_query_sender,
        verified_email_receiver,
        email_verifier_sender,
    )?
    .await
}
