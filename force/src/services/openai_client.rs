use std::error::Error;

use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};

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
    pub fn new(api_key: String) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        OpenaiClient {
            client: Client::with_config(config),
        }
    }

    pub async fn get_boolean_searches_from_niche(
        &self,
        niche: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4o-mini")
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(format!(
                    r#"
                    Give different names for the following product: {}
                    For example for product "yoga mat" similar products will be like: yoga block, silk yoga mat, yellow yoga mat, yoga mat bag, workout mat
                    Only return 3 product names in a list but don't start with a bullet point.
                    Do not give numbers to products.
                    "#,
                    niche
                ))
                .build()?
                .into()])
            .max_tokens(300_u32)
            .build()?;

        let response = self.client.chat().create(request).await?;
        log::info!("Response: {:?}", response);

        let first_choice = response
            .choices
            .first()
            .ok_or("No choices in Openai response")?
            .message
            .content
            .clone()
            .ok_or("No content")?;

        let searches: Vec<String> = first_choice
            .split("\n")
            .map(|s| s.trim().to_string())
            .collect();

        Ok(searches)
    }
}
