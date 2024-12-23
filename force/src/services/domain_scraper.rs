use std::time::Duration;

use crossbeam::channel::{Receiver, Sender};

pub struct DomainScraperSender {
    pub sender: Sender<String>,
}

pub async fn domain_scraper(receiver: Receiver<String>) {
    log::info!("Started domain scraper");
    loop {
        match receiver.recv() {
            Ok(query) => {
                log::info!("Got query: {:?}", query);
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}
