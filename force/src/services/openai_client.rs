use std::error::Error;

use async_openai::{config::OpenAIConfig, types::CreateCompletionRequestArgs, Client};

pub struct OpenaiClient {
    client: Client<OpenAIConfig>,
}

impl Default for OpenaiClient {
    fn default() -> Self {
        OpenaiClient {
            client: Client::new(),
        }
    }
}

impl OpenaiClient {
    pub async fn get_boolean_searches_from_niche(
        &self,
        niche: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let request = CreateCompletionRequestArgs::default()
            .model("gpt-3.5-turbo-instruct")
            .prompt(
                format!(
                    r#"
                    Give names of different product examples in the following niche: {}
                    Only return 200 product names. In the following format:
                    "Product 1" AND "buy now". Product name and buy now should always be in quotation marks i.e. "".
                    Lastly do not give numbers to products. follow the format I gave you strictly.
                    "#,
                    niche
                )
            )
            .max_tokens(300_u32) // 64000_u32 for prod
            .build()?;

        let response = self.client.completions().create(request).await?;
        log::info!("Response: {:?}", response);

        let first_choice = response
            .choices
            .first()
            .ok_or("No choices in Openai response")?
            .text
            .clone();

        let searches: Vec<String> = first_choice.split("\n").map(|s| s.to_string()).collect();

        Ok(searches)
    }
}
