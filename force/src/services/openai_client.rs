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
        prompt: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4o-mini")
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()?
                .into()])
            .max_tokens(1000_u32)
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
