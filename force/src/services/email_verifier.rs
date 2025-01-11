use std::{collections::HashSet, error::Error};

use actix_web::web::Data;
use check_if_email_exists::Reachable;
use tokio::sync::{
    broadcast,
    mpsc::{UnboundedReceiver, UnboundedSender},
};

use crate::domain::email::FounderDomainEmail;

use super::{PersistantData, Sentinel};

const SET_RESET_LEN: usize = 10_000;

pub struct VerifiedEmailReceiver {
    pub sender: broadcast::Sender<String>,
}
pub struct EmailVerifierSender {
    pub sender: UnboundedSender<FounderDomainEmail>,
}

pub async fn email_verified_handler(
    sentinel: Data<Sentinel>,
    mut email_receiver: UnboundedReceiver<FounderDomainEmail>,
    persistant_data_sender: UnboundedSender<PersistantData>,
    verified_email_sender: broadcast::Sender<String>,
) {
    log::info!("Started email verifier handler");
    let mut seen_emails = HashSet::new();

    while let Some(email) = email_receiver.recv().await {
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
                    verified_email_sender.clone(),
                    email,
                ));
            }
        }
    }
}

async fn verify_email(
    sentinel: Data<Sentinel>,
    persistant_data_sender: UnboundedSender<PersistantData>,
    verified_email_sender: broadcast::Sender<String>,
    email: FounderDomainEmail, // TODO: Use only email
) {
    log::info!("Verifying email: {}", email.email);

    let valid = sentinel.verify_email_manual(&email.email).await;
    if valid {
        if let Err(e) = verified_email_sender.send(email.email.clone()) {
            log::error!(
                "Verified email sender broadcast channel got an Error: {:?} | Source: {:?}",
                e,
                e.source(),
            );
        }
        if let Err(e) =
            persistant_data_sender.send(PersistantData::UpdateEmailVerified(email.email))
        {
            log::error!(
                "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                e,
                e.source(),
            );
        }
    }

    // let reachable = sentinel.get_email_verification_status(&email.email).await;
    // match reachable {
    //     Reachable::Safe => {
    //         // Errors if there is no route thread listening for verified emails
    //         _ = verified_email_sender.send(email.email.clone());
    //
    //         if let Err(e) =
    //             persistant_data_sender.send(PersistantData::UpdateEmailVerified(email.email))
    //         {
    //             log::error!(
    //                 "Persistant data sender channel got an Error: {:?} | Source: {:?}",
    //                 e,
    //                 e.source(),
    //             );
    //         }
    //     }
    //     _ => {}
    // };
}
