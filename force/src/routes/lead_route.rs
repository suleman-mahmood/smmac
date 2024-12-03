use std::time::Duration;

use actix_web::{get, web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use thirtyfour::{prelude::ElementWaitable, By, DesiredCapabilities, WebDriver};

use crate::services::OpenaiClient;

#[derive(Deserialize)]
struct GetLeadsFromNicheQuery {
    niche: String,
    requester_email: String,
}

#[get("")]
async fn get_leads_from_niche(
    openai_client: web::Data<OpenaiClient>,
    body: web::Query<GetLeadsFromNicheQuery>,
    pool: web::Data<PgPool>,
) -> HttpResponse {
    /*
    1. (v2) User verification and free tier count
    2. Get boolean search list from openai using the niche prompt
    3. Perform web scraping on each boolean search page, store results in db
        3.1 (v2) Rotate ips if getting blocked from google
    4. Construct emails from results in previous step
    5. Verify emails from API
    6. Return verified leads (emails)
    */

    // let products = openai_client
    //     .get_boolean_searches_from_niche(&body.niche)
    //     .await
    //     .unwrap();

    let urls = get_urls_from_google_searches(vec![
        r#""Herbal Green Tea Face Gel" AND "buy now""#.to_string(),
        r#""Organic Tea Tree Hand Cream" AND "buy now""#.to_string(),
        r#""Organic Rosehip Face Oil" AND "buy now""#.to_string(),
        r#""Natural Aloe Vera Gel" AND "buy now""#.to_string(),
        r#""Herbal Neem Face Wash" AND "buy now""#.to_string(),
        r#""Organic Lavender Body Lotion" AND "buy now""#.to_string(),
        r#""Pure Argan Hair Serum" AND "buy now""#.to_string(),
        r#""Shea Butter Lip Balm" AND "buy now""#.to_string(),
        r#""Natural Coffee Scrub" AND "buy now""#.to_string(),
        r#""Chamomile Eye Cream" AND "buy now""#.to_string(),
        r#""Organic Tea Tree Toner" AND "buy now""#.to_string(),
        r#""Turmeric Glow Face Mask" AND "buy now""#.to_string(),
        r#""Organic Coconut Shampoo Bar" AND "buy now""#.to_string(),
        r#""Rose Hydrosol Mist" AND "buy now""#.to_string(),
        r#""Organic Beeswax "and "ream”"AND “buy now”"#.to_string(),
        r#""Natural Clay Detox Mask" AND "buy now""#.to_string(),
        r#""Calendula Baby Lotion" AND "buy now""#.to_string(),
        r#""Green Tea Face Serum" AND "buy now""#.to_string(),
        r#""Herbal Hibiscus Shampoo" AND "buy now""#.to_string(),
        r#""Activated Charcoal Face Soap" AND "buy now""#.to_string(),
        r#""Organic Almond Body Scrub" AND "buy now""#.to_string(),
        r#""Natural Lavender Bath Salts" AND "buy now""#.to_string(),
        r#""Organic Cocoa Butter Moisturizer" AND "buy now""#.to_string(),
        r#"""ose "nd Geranium"Hair Oil” AND “buy now”"#.to_string(),
        r#""Peppermint Foot Cream" AND "buy now""#.to_string(),
        r#""Natural Mineral Sunscreen" AND "buy now""#.to_string(),
        r#""Organic Bamboo Charcoal Mask" AND "buy now""#.to_string(),
        r#""Lemon Verbena "and "ash”"AND “buy now”"#.to_string(),
        r#""Organic Eucalyptus Beard Balm" AND "buy now""#.to_string(),
        r#""Herbal Ayurvedic Night Cream" AND "buy now""#.to_string(),
        r#""Pure Vanilla Lip Scrub" AND "buy now""#.to_string(),
        r#""Organic Jojoba Hair Serum" AND "buy now""#.to_string(),
        r#"""osemary "nd Thyme"Conditioner” AND “buy now”"#.to_string(),
        r#""Natural Oatmeal Exfoliant" AND "buy now""#.to_string(),
        r#""Organic Matcha Clay Mask" AND "buy now""#.to_string(),
        r#""Wild Honey Face Cleanser" AND "buy now""#.to_string(),
        r#""Organic Camellia Oil" AND "buy now""#.to_string(),
        r#""Herbal Basil Toner" AND "buy now""#.to_string(),
        r#""Pure Marula Facial Oil" AND "buy now""#.to_string(),
        r#""Natural Mango Butter Lotion" AND "buy now""#.to_string(),
        r#""Organic Frankincense Serum" AND "buy now""#.to_string(),
        r#"""hamomile "nd Oats"Soap Bar” AND “buy now”"#.to_string(),
        r#""Calendula Healing Balm" AND "buy now""#.to_string(),
        r#""Herbal Licorice Skin Brightener" AND "buy now""#.to_string(),
        r#""Natural Hemp Seed Face Oil" AND "buy now""#.to_string(),
        r#""Rose Quartz Face Roller" AND "buy now""#.to_string(),
        r#""Organic Carrot Seed Sunscreen" AND "buy now""#.to_string(),
        r#""Pure Cocoa Lip Conditioner" AND "buy now""#.to_string(),
        r#""Natural Ylang Ylang Perfume" AND "buy now""#.to_string(),
        r#""Herbal "eem "nd Basil"Soap” AND “buy now”"#.to_string(),
        r#""Organic Echinacea Night Serum" AND "buy now""#.to_string(),
        r#""Natural Rice Bran Moisturizer" AND "buy now""#.to_string(),
        r#""Organic Seabuckthorn Lotion" AND "buy now""#.to_string(),
        r#""Herbal Fenugreek Hair Mask" AND "buy now""#.to_string(),
        r#""Pure "andalwood "ody Butter" AND “buy now”"#.to_string(),
        r#""Natural Rose Petal Mist" AND "buy now""#.to_string(),
        r#""Organic Lavender Face Mask" AND "buy now""#.to_string(),
        r#""Chamomile Herbal Toner" AND "buy now""#.to_string(),
        r#""Natural Grapefruit Scrub" AND "buy now""#.to_string(),
        r#""Organic Patchouli Body Wash" AND "buy now""#.to_string(),
        r#""Herbal Turmeric Oil" AND "buy now""#.to_string(),
        r#""Pure Olive Body Soap" AND "buy now""#.to_string(),
        r#""Natural Pomegranate Serum" AND "buy now""#.to_string(),
        r#""Organic Ginger Hair Tonic" AND "buy now""#.to_string(),
        r#""Herbal Papaya Brightening Cream" AND "buy now""#.to_string(),
        r#""Pure Avocado Butter Cream" AND "buy now""#.to_string(),
        r#""Natural Cucumber Cooling Gel" AND "buy now""#.to_string(),
        r#""Organic Cedarwood Beard Oil" AND "buy now""#.to_string(),
        r#""Herbal Mint Lip Balm" AND "buy now""#.to_string(),
        r#""Organic Camphor Muscle Rub" AND "buy now""#.to_string(),
        r#""Pure Almond Skin Softener" AND "buy now""#.to_string(),
        r#""Natural Aloe Vera Sunscreen" AND "buy now""#.to_string(),
        r#""Organic Jasmine Body Mist" AND "buy now""#.to_string(),
        r#""Herbal Nutmeg Face Glow Serum" AND "buy now""#.to_string(),
        r#""Natural Bamboo Scrubbing Gel" AND "buy now""#.to_string(),
        r#""Organic Witch Hazel Toner" AND "buy now""#.to_string(),
        r#""Pure Lavender Sleep Balm" AND "buy now""#.to_string(),
        r#""Herbal Coconut Hair Mask" AND "buy now""#.to_string(),
        r#""Organic Lemon Zest Face Wash" AND "buy now""#.to_string(),
        r#""Natural Basil Antioxidant Serum" AND "buy now""#.to_string(),
        r#""Organic Wildflower Shampoo" AND "buy now""#.to_string(),
        r#""Chamomile Hydrating Lotion" AND "buy now""#.to_string(),
        r#""Pure Geranium Oil Blend" AND "buy now""#.to_string(),
        r#""Herbal Raspberry Lip Tint" AND "buy now""#.to_string(),
        r#""Organic Vanilla Body Lotion" AND "buy now""#.to_string(),
        r#""Natural Coffee Bean Eye Cream" AND "buy now""#.to_string(),
        r#""Organic Green Apple Conditioner" AND "buy now""#.to_string(),
        r#""Herbal Sage Scalp Treatment" AND "buy now""#.to_string(),
        r#""Pure Orange Blossom Cream" AND "buy now""#.to_string(),
        r#""Natural Rosemary Face Cleanser" AND "buy now""#.to_string(),
        r#""Organic Honey Glow Mask" AND "buy now""#.to_string(),
        r#""Herbal Walnut Scrub" AND "buy now""#.to_string(),
        r#""Pure Lavender Essential Oil" AND "buy now""#.to_string(),
        r#""Natural Papaya Hair Serum" AND "buy now""#.to_string(),
        r#""Organic Poppy Seed Soap" AND "buy now""#.to_string(),
        r#""Herbal Fenugreek Oil" AND "buy now""#.to_string(),
        r#""Pure Castor Hair Elixir" AND "buy now""#.to_string(),
        r#""Natural Hibiscus Shampoo Bar" AND "buy now""#.to_string(),
        r#""Organic Fig Lip Mask" AND "buy now""#.to_string(),
        r#""Herbal Cardamom Body Cream" AND "buy now""#.to_string(),
        r#""Pure Chamomile Skin Soother" AND "buy now""#.to_string(),
        r#""Natural Coconut Butter Scrub" AND "buy now""#.to_string(),
        r#""Organic "avender "nd Lime"Mist” AND “buy now”"#.to_string(),
        r#""Herbal Lemon Peel Scrub" AND "buy now""#.to_string(),
        r#""Pure Rose Petal Face Pack" AND "buy now""#.to_string(),
        r#""Natural Grapeseed Serum" AND "buy now""#.to_string(),
        r#""Organic Tea Tree Face Gel" AND "buy now""#.to_string(),
        r#""Herbal Moringa Night Oil" AND "buy now""#.to_string(),
        r#""Pure "eem "nd Aloe"Gel” AND “buy now”"#.to_string(),
        r#""Natural Orange Spice Body Wash" AND "buy now""#.to_string(),
        r#""Organic Jasmine "and "otion”"AND “buy now”"#.to_string(),
        r#""Herbal Vanilla Bean Lip Butter" AND "buy now""#.to_string(),
        r#""Pure "hea "nd Mango"Lotion” AND “buy now”"#.to_string(),
        r#""Natural Lavender Exfoliating Scrub" AND "buy now""#.to_string(),
        r#""Organic Peppermint Hair Tonic" AND "buy now""#.to_string(),
        r#""Herbal Rosehip Night Cream" AND "buy now""#.to_string(),
        r#""Pure Beeswax Lip Tint" AND "buy now""#.to_string(),
        r#""Natural "osemary "nd Mint"Spray” AND “buy now”"#.to_string(),
        r#""Organic Avocado Hair Conditioner" AND "buy now""#.to_string(),
        r#""Herbal Coconut Milk Soap" AND "buy now""#.to_string(),
        r#""Pure "lmond "nd Apricot"Scrub” AND “buy now”"#.to_string(),
        r#""Natural Pomegranate Face Pack" AND "buy now""#.to_string(),
        r#""Organic Tea Tree "and "ream”"AND “buy now”"#.to_string(),
        r#""Herbal Green Tea Face Gel" AND "buy now""#.to_string(),
        r#""Pure "loe "nd Turmeric"Cleanser” AND “buy now”"#.to_string(),
        r#""Natural Chamomile Sleep Spray" AND "buy now""#.to_string(),
        r#""Organic "asil "nd Lemon"Soap” AND “buy now”"#.to_string(),
        r#""Herbal "eem "nd Turmeric"Cream” AND “buy now”"#.to_string(),
        r#""Pure "andalwood "ody Lotion" AND “buy now”"#.to_string(),
        r#""Natural Tea Tree Face Cream" AND "buy now""#.to_string(),
        r#""Organic Hibiscus Hair Conditioner" AND "buy now""#.to_string(),
        r#""Herbal Rose Water Toner" AND "buy now""#.to_string(),
        r#""Pure Camellia Face Mist" AND "buy now""#.to_string(),
        r#""Natural "loe "nd Rose"Scrub” AND “buy now”"#.to_string(),
        r#""Organic Peppermint Beard Oil" AND "buy now""#.to_string(),
        r#""Herbal Chamomile Eye Gel" AND "buy now""#.to_string(),
        r#""Pure Basil Leaf Moisturizer" AND "buy now""#.to_string(),
        r#""Natural Hibiscus Lip Butter" AND "buy now""#.to_string(),
        r#""Organic Lemon Basil Hair Serum" AND "buy now""#.to_string(),
        r#""Herbal "eem "nd Coconut"Oil” AND “buy now”"#.to_string(),
        r#""Pure Vanilla Bean Body Butter" AND "buy now""#.to_string(),
        r#""Natural Coffee Bean Face Mask" AND "buy now""#.to_string(),
        r#""Organic Aloe Mint Shampoo" AND "buy now""#.to_string(),
        r#""Herbal Eucalyptus Balm" AND "buy now""#.to_string(),
        r#""Pure "vocado "nd Olive"Cream” AND “buy now”"#.to_string(),
        r#""Natural Coconut Water Toner" AND "buy now""#.to_string(),
        r#""Organic Orange Spice Lip Balm" AND "buy now""#.to_string(),
        r#""Herbal "ibiscus "nd Rose"Lotion” AND “buy now”"#.to_string(),
        r#""Pure "hea "nd Argan"Oil Cream” AND “buy now”"#.to_string(),
        r#""Natural "asil "nd Mint"Face Wash” AND “buy now”"#.to_string(),
        r#""Organic Chamomile "and "ream”"AND “buy now”"#.to_string(),
        r#""Herbal "osemary "nd Olive"Oil” AND “buy now”"#.to_string(),
        r#""Pure Lavender Night Serum" AND "buy now""#.to_string(),
        r#""Natural Aloe Mint Soap" AND "buy now""#.to_string(),
        r#""Organic "emon "nd Honey"Mask” AND “buy now”"#.to_string(),
        r#""Herbal Tea Tree Face Lotion" AND "buy now""#.to_string(),
        r#""Pure "hamomile "nd Lavender"#.to_string(),
    ])
    .await;

    match urls {
        Ok(urls) => log::info!("Got urls: {:?}", urls),
        Err(e) => log::error!("Error: {}", e),
    }

    HttpResponse::Ok().body("Works!")
}

async fn get_urls_from_google_searches(search_terms: Vec<String>) -> Result<Vec<String>, String> {
    let search_urls: Vec<String> = search_terms
        .iter()
        .flat_map(|st| {
            // (0..50).map(move |index| {
            (0..1).map(move |index| {
                format!(
                    "https://www.google.com/search?q={}&start={}",
                    st,
                    index * 10
                )
            })
        })
        .collect();

    let caps = DesiredCapabilities::chrome();
    let driver = WebDriver::new("http://localhost:54610", caps)
        .await
        .map_err(|e| e.to_string())?;
    driver.maximize_window().await.map_err(|e| e.to_string())?;

    let mut urls: Vec<String> = vec![];

    for url in search_urls.iter() {
        driver.goto(url).await.map_err(|e| e.to_string())?;

        let a_tag = driver
            .find(By::XPath(
                "/a", // "/html/body/div[3]/div/div[13]/div/div[2]/div[2]/div/div/div/div/div/div/div[1]/div/div/span/a",
            ))
            .await;
        if let Err(e) = a_tag {
            log::error!("Couldn't find a_tag: {}", e);
            continue;
        }

        let a_tags = driver
            .find_all(By::XPath(
                "/a", // "/html/body/div[3]/div/div[13]/div/div[2]/div[2]/div/div/div/div/div/div/div[1]/div/div/span/a",
            ))
            .await
            .unwrap();

        for a_tag in a_tags {
            let href_attribute = a_tag.attr("href").await.unwrap();
            if let Some(href) = href_attribute {
                urls.push(href.clone());
                log::info!("Added url: {}", href);
            }
        }
    }

    Ok(urls)
}
