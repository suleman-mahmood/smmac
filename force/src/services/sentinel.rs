use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct Sentinel {
    client: Client,
    api_key: String,
    url: String,
}

#[derive(Serialize)]
struct PostBody {
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
            url: "https://api-v4.bulkemailchecker.com/".to_string(),
        }
    }

    pub async fn verfiy_email(&self, email: String) -> bool {
        match self
            .client
            .post(self.url.clone())
            .json(&PostBody {
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
}
