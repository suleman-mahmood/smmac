use check_if_email_exists::{check_email, CheckEmailInput, Reachable};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct Sentinel {
    client: Client,
    api_key: String,
    url: String,
}

#[derive(Serialize)]
struct GetQuery {
    key: String,
    email: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    status: String,
}

impl Sentinel {
    pub fn new(api_key: String) -> Self {
        let client = reqwest::Client::new();

        Sentinel {
            client,
            api_key,
            url: "https://api.bulkemailchecker.com/real-time".to_string(),
        }
    }

    pub async fn verfiy_email_from_bulk_mail_checker(&self, email: String) -> bool {
        match self
            .client
            .get(self.url.clone())
            .query(&GetQuery {
                key: self.api_key.clone(),
                email,
            })
            .send()
            .await
        {
            Ok(res) => match res.json::<ApiResponse>().await {
                Ok(json) => json.status == "passed",
                Err(e) => {
                    log::error!("Error when deserializing to json: {:?}", e);
                    false
                }
            },
            Err(e) => {
                log::error!("Got error from bulk email check api: {:?}", e);
                false
            }
        }
    }

    pub async fn verfiy_email(&self, email: String) -> bool {
        let mut input = CheckEmailInput::new(email);
        input
            .set_from_email("random.guy@fit.com".to_string())
            .set_hello_name("verywellfit.com".to_string());

        // .set_proxy(CheckEmailInputProxy {
        //     host: "p.webshare.io".to_string(),
        //     port: 9999,
        //     username: None,
        //     password: None,
        // }); //works
        // .set_proxy(CheckEmailInputProxy {
        //     host: "69.12.93.165".to_string(),
        //     port: 6185,
        //     username: None,
        //     password: None,
        // }); //works
        // .set_gmail_use_api(true) // SMTP is ok and its API works but invalid email
        // .set_smtp_port(587);

        // Verify this email, using async/await syntax.
        let result = check_email(&input).await;

        log::info!("Result from API: {:?}", result);

        match result.is_reachable {
            Reachable::Safe => true,
            Reachable::Unknown => false,
            Reachable::Risky => false,
            Reachable::Invalid => false,
        }
    }

    pub async fn get_email_verification_status(&self, email: &str) -> Reachable {
        let mut input = CheckEmailInput::new(email.to_string());
        input
            .set_from_email("random.guy@fit.com".to_string())
            .set_hello_name("verywellfit.com".to_string());

        // Verify this email, using async/await syntax.
        let result = check_email(&input).await;

        log::info!("{} verification result {:?}", email, result);

        result.is_reachable
    }
}
