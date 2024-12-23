use std::time::Duration;

use crossbeam::channel::{Receiver, Sender};

pub struct DomainScraperSender {
    pub sender: Sender<String>,
}

pub async fn domain_scraper(receiver: Receiver<String>) {
    loop {
        match receiver.recv() {
            Ok(query) => {
                todo!();
            }
            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}
