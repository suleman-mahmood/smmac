use std::collections::HashSet;

use actix_web::web::Data;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::routes::lead_route::build_founder_seach_queries;

use super::{FounderQueryChannelData, PersistantData, Sentinel};

const SET_RESET_LEN: usize = 10_000;

pub async fn domain_qualifier_handler(
    sentinel: Data<Sentinel>,
    mut product_query_receiver: UnboundedReceiver<String>,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Started domain qualifier");
    let mut seen_queries = HashSet::new();

    // TODO: Use tokio::select! to check for a signal that asks to move certain tasks from priority queue to backgound
    while let Some(domain) = product_query_receiver.recv().await {
        log::info!(
            "Domain qualifier handler has {} elements",
            product_query_receiver.len()
        );

        match seen_queries.contains(&domain) {
            true => {}
            false => {
                // TODO: Implement time based reset like 10 mins after channel was empty
                if seen_queries.len() > SET_RESET_LEN {
                    seen_queries.clear();
                }
                seen_queries.insert(domain.clone());
                tokio::spawn(qualify_domain(
                    sentinel.clone(),
                    domain,
                    founder_query_sender.clone(),
                    persistant_data_sender.clone(),
                ));
            }
        }
    }
}

async fn qualify_domain(
    sentinel: Data<Sentinel>,
    domain: String,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
    log::info!("Qualifying domain: {}", domain);

    let email = format!("kdsjfkljrkvj87@{}", domain);
    let is_catch_all = sentinel.verify_email_manual(email.as_str()).await;

    match is_catch_all {
        false => {
            for query in build_founder_seach_queries(&domain) {
                founder_query_sender
                    .send(FounderQueryChannelData {
                        query,
                        domain: domain.clone(),
                    })
                    .unwrap();
            }
        }
        true => {}
    }
}
