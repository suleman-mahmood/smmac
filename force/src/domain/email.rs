use check_if_email_exists::Reachable;

pub struct Email {
    pub email_address: String,
    pub founder_name: String,
    pub domain: String,
    pub verification_status: VerificationStatus,
    pub reachability: Reachability,
}

pub enum VerificationStatus {
    Pending,
    Verified,
    Invalid,
    CatchAll,
}

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
