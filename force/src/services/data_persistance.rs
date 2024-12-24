use std::time::Duration;

use crossbeam::channel::Receiver;

pub enum PersistantData {}

pub async fn data_persistance_handler(data_receiver: Receiver<PersistantData>) {
    log::info!("Started data persistance handler");

    loop {
        match data_receiver.recv() {
            Ok(data) => {
                todo!();
            }

            Err(_) => tokio::time::sleep(Duration::from_secs(5)).await,
        }
    }
}
