use check_if_email_exists::Reachable;
use serde::Deserialize;

use crate::dal::lead_db::{EmailReachability, EmailVerifiedStatus};

pub struct Email {
    pub email_address: String,
    pub founder_name: String,
    pub domain: String,
    pub verification_status: VerificationStatus,
    pub reachability: Reachability,
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VerificationStatus", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationStatus {
    Pending,
    Verified,
    Invalid,
    CatchAll,
}

impl From<EmailVerifiedStatus> for VerificationStatus {
    fn from(value: EmailVerifiedStatus) -> Self {
        match value {
            EmailVerifiedStatus::Pending => Self::Pending,
            EmailVerifiedStatus::Verified => Self::Verified,
            EmailVerifiedStatus::Invalid => Self::Invalid,
        }
    }
}

#[derive(Debug, PartialEq, Deserialize, sqlx::Type)]
#[sqlx(type_name = "Reachability", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Reachability {
    Safe,
    Unknown,
    Risky,
    Invalid,
}

impl From<Reachable> for Reachability {
    fn from(value: Reachable) -> Self {
        match value {
            Reachable::Safe => Reachability::Safe,
            Reachable::Unknown => Reachability::Unknown,
            Reachable::Risky => Reachability::Risky,
            Reachable::Invalid => Reachability::Invalid,
        }
    }
}

impl From<EmailReachability> for Reachability {
    fn from(value: EmailReachability) -> Self {
        match value {
            EmailReachability::Safe => Self::Safe,
            EmailReachability::Unknown => Self::Unknown,
            EmailReachability::Risky => Self::Risky,
            EmailReachability::Invalid => Self::Invalid,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FounderDomainEmail {
    pub founder_name: String,
    pub domain: String,
    pub email: String,
}

pub fn construct_email_permutations(name: &str, domain: &str) -> Vec<FounderDomainEmail> {
    let mut emails_db: Vec<FounderDomainEmail> = vec![];

    let name_pieces: Vec<&str> = name.split(" ").collect();
    if name_pieces.len() == 2 {
        let name_vec: Vec<&str> = name.split(" ").collect();
        let first_name = name_vec.first().unwrap().to_lowercase();
        let last_name = name_vec.get(1).unwrap().to_lowercase();

        emails_db.push(FounderDomainEmail {
            email: format!("{}@{}", first_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}@{}", last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}{}@{}", first_name, last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!("{}.{}@{}", first_name, last_name, domain),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!(
                "{}{}@{}",
                first_name,
                last_name.chars().next().unwrap(),
                domain
            ),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
        emails_db.push(FounderDomainEmail {
            email: format!(
                "{}{}@{}",
                first_name.chars().next().unwrap(),
                last_name,
                domain
            ),
            founder_name: name.to_string(),
            domain: domain.to_string(),
        });
    }

    emails_db
}
