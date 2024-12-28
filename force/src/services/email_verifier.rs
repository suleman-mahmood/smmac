use std::collections::HashSet;

use actix_web::web::Data;
use check_if_email_exists::Reachable;
use crossbeam::channel::{Receiver, Sender};

use crate::domain::email::FounderDomainEmail;

use super::{PersistantData, Sentinel};

const SET_RESET_LEN: usize = 10_000;

pub async fn email_verified_handler(
    sentinel: Data<Sentinel>,
    email_receiver: Receiver<FounderDomainEmail>,
    persistant_data_sender: Sender<PersistantData>,
) {
    log::info!("Started email verifier handler");
    let mut seen_emails = HashSet::new();

    for email in email_receiver.iter() {
        log::info!(
            "Email verifier handler has {} elements",
            email_receiver.len()
        );

        match seen_emails.contains(&email.email) {
            true => {}
            false => {
                // TODO: Implement time based reset like 10 mins after channel was empty
                if seen_emails.len() > SET_RESET_LEN {
                    seen_emails.clear();
                }
                seen_emails.insert(email.email.clone());
                tokio::spawn(verify_email(
                    sentinel.clone(),
                    persistant_data_sender.clone(),
                    email,
                ));
            }
        }
    }
}

async fn verify_email(
    sentinel: Data<Sentinel>,
    persistant_data_sender: Sender<PersistantData>,
    email: FounderDomainEmail,
) {
    log::info!("Verifying email: {}", email.email);

    let reachable = sentinel.get_email_verification_status(&email.email).await;
    match reachable {
        Reachable::Safe => {
            persistant_data_sender
                .send(PersistantData::UpdateEmailVerified(email.email))
                .unwrap();
        }
        _ => {}
    };
}
