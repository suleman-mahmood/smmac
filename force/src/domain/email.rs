struct Email {
    email_address: String,
    founder_name: String,
    domain: String,
    verification_status: VerificationStatus,
    reachability: Reachability,
}

enum VerificationStatus {
    Pending,
    Verified,
    Invalid,
    CatchAll,
}

enum Reachability {
    Safe,
    Unknown,
    Risky,
    Invalid,
}
