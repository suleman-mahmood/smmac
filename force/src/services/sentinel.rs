use std::time::Duration;

use async_smtp::{
    commands::{MailCommand, RcptCommand},
    SmtpClient, SmtpTransport,
};
use check_if_email_exists::{check_email, CheckEmailInput, CheckEmailOutput, Reachable};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncWrite, BufStream},
    net::TcpStream,
};

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

    async fn get_email_info(&self, email: &str) -> CheckEmailOutput {
        let mut input = CheckEmailInput::new(email.to_string());
        input.set_smtp_timeout(Some(Duration::from_millis(100)));

        check_email(&input).await
    }

    pub async fn verify_email_manual(&self, email: &str) -> bool {
        let dns_info = self.get_email_info(email).await;

        // CheckEmailOutput input: "sulemanmahmood99@gmail.com", is_reachable: Safe, misc: Ok(MiscDetails { is_disposable: false, is_role_account: false, gravatar_url: None, haveibeenpwned: None }),
        //   mx: Ok(MxDetails { lookup: Ok(MxLookup(Lookup { query: Query { name: Name("gmail.com"), query_type: MX, query_class: IN },
        //     records: [
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 30, exchange: Name("alt3.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 10, exchange: Name("alt1.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 20, exchange: Name("alt2.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 40, exchange: Name("alt4.gmail-smtp-in.l.google.com.") })) },
        //       Record { name_labels: Name("gmail.com."), rr_type: MX, dns_class: IN, ttl: 1411, rdata: Some(MX(MX { preference: 5, exchange: Name("gmail-smtp-in.l.google.com.") })) }
        //     ],
        //     valid_until: Instant { tv_sec: 6077130, tv_nsec: 595604176 } })) })
        let exchanges: Vec<String> = dns_info
            .mx
            .unwrap()
            .lookup
            .unwrap()
            .iter()
            .map(|rdata| rdata.exchange().to_string())
            .collect();

        let smtp_server = exchanges.first().unwrap();
        let smtp_server = smtp_server.trim_end_matches(".");
        let smtp_server_port = format!("{}:25", smtp_server);

        // Define a new trait that combines AsyncRead, AsyncWrite, and Unpin
        trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send {}
        impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncReadWrite for T {}

        let stream = TcpStream::connect(smtp_server_port).await.unwrap();
        let stream = BufStream::new(Box::new(stream) as Box<dyn AsyncReadWrite>);
        let client = SmtpClient::new();
        let mut transport = SmtpTransport::new(client, stream).await.unwrap();

        transport
            .get_mut()
            .command(MailCommand::new(
                Some("random.guy@fit.com".parse().unwrap()),
                vec![],
            ))
            .await
            .unwrap();

        let response = transport
            .get_mut()
            .command(RcptCommand::new(email.parse().unwrap(), vec![]))
            .await
            .unwrap();

        response.is_positive()
    }
}
