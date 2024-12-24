use std::error::Error;

use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use sqlx::PgPool;

use crate::dal::{config_db, niche_db};

pub const FRESH_RESULTS: bool = true; // Default to false

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

pub async fn save_product_search_queries(
    pool: &PgPool,
    openai_client: &OpenaiClient,
    niche: &str,
) -> Vec<String> {
    if !FRESH_RESULTS {
        if let Ok(products) = niche_db::get_niche(pool, niche).await {
            return products.generated_products;
        }
    }

    let (left_prompt, right_prompt) = config_db::get_gippity_prompt(pool).await.unwrap();
    let prompt = format!(
        "{} {} {}",
        left_prompt.unwrap_or("Give different names for the following product:".to_string()),
        niche,
        right_prompt.unwrap_or(r#"
            For example for product "yoga mat" similar products will be like: yoga block, silk yoga mat, yellow yoga mat, yoga mat bag, workout mat.
            Only return 10 product names in a list but don't start with a bullet point.
            Do not give numbers to products.
            Give each product on a new line.
        "#.to_string())
    );

    let products = openai_client
        .get_boolean_searches_from_niche(&prompt)
        .await
        .unwrap();

    let products: Vec<String> = products
        .into_iter()
        .map(|p| p.trim().to_lowercase())
        .collect();

    _ = niche_db::insert_niche(pool, niche, &prompt, products.clone()).await;

    products
}
