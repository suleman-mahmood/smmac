use std::time::Duration;

use sqlx::{Acquire, PgPool};
use tokio::time;

use crate::{
    dal::smart_scout_db,
    domain::{html_tag::extract_company_name, smart_scout::SmartScout},
    routes::lead_route::build_company_name_search_query,
    services::{
        extract_data_from_google_search_with_reqwest, GoogleSearchResult, GoogleSearchType,
    },
};

const N: i64 = 10;

pub async fn smart_scout_scraper_handler(pool: PgPool) {
    log::info!("Started smart scout scraper");

    // Create a 30 min interval
    let mut interval = time::interval(Duration::from_secs(30 * 60));

    loop {
        /*
        1. Tick every 'm' minutes
        2. Check stop condition: Use a notify channel on route invocation
        3. Get 'n' (random) unscraped jobs from the smart scout table
        4. Start scraping them
        */
        interval.tick().await;

        // TODO: Stop condition

        let pool_con_result = pool.acquire().await;
        let Ok(mut pool_con) = pool_con_result else {
            log::error!("Pool timed out: {:?}", pool_con_result.unwrap_err());
            continue;
        };
        let con = pool_con.acquire().await.unwrap();

        let ss_companies = smart_scout_db::get_n_unscraped_company_ids(con, N)
            .await
            .unwrap();

        for ss in ss_companies {
            smart_scout_db::start_job(con, ss.id).await.unwrap();
            tokio::spawn(scrape_company_domain_query(ss));
        }
    }
}

async fn scrape_company_domain_query(ss: SmartScout) {
    log::info!(
        "Scraping google for company domain for company: {}",
        ss.name
    );

    let query = build_company_name_search_query(&ss.name);

    let google_search_result =
        extract_data_from_google_search_with_reqwest(query.clone(), GoogleSearchType::CompanyName)
            .await;

    match google_search_result {
        GoogleSearchResult::Domains { .. } | GoogleSearchResult::Founders(..) => {
            log::error!("Returning domains or founders from company name search");
        }
        GoogleSearchResult::CaptchaBlocked => {
            log::error!("Returning from captcha blocked on url {}", query);
        }
        GoogleSearchResult::NotFound => {
            todo!();
        }
        GoogleSearchResult::CompanyNames {
            name_candidates,
            page_source,
        } => {
            let company_names: Vec<Option<String>> = name_candidates
                .into_iter()
                .map(|tag| extract_company_name(tag))
                .collect();

            // TODO: Add logic to transfer data to further channels
            // TODO: Add data persistance logic
        }
    }
}
