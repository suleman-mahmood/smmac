use std::{error::Error, time::Duration};

use sqlx::{Acquire, PgPool};
use tokio::{sync::mpsc::UnboundedSender, time};

use crate::{
    dal::smart_scout_db,
    domain::{
        html_tag::{extract_company_domain, extract_domain},
        smart_scout::SmartScout,
    },
    routes::lead_route::{
        build_company_name_search_query, build_founder_seach_queries, BLACK_LIST_DOMAINS,
    },
    services::{
        extract_data_from_google_search_with_reqwest, CompanyNameData, GoogleSearchResult,
        GoogleSearchType,
    },
};

use super::{FounderQueryChannelData, PersistantData};

const N: i64 = 10;

pub async fn smart_scout_scraper_handler(
    pool: PgPool,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
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
            tokio::spawn(scrape_company_domain_query(
                ss,
                founder_query_sender.clone(),
                persistant_data_sender.clone(),
            ));
        }
    }
}

async fn scrape_company_domain_query(
    ss: SmartScout,
    founder_query_sender: UnboundedSender<FounderQueryChannelData>,
    persistant_data_sender: UnboundedSender<PersistantData>,
) {
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
            if let Err(e) = persistant_data_sender.send(PersistantData::CompanyName(
                CompanyNameData::NoResult { query },
            )) {
                log::error!(
                    "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                    e,
                    e.source(),
                );
            }
        }
        GoogleSearchResult::CompanyNames {
            name_candidates,
            page_source,
        } => {
            let domains: Vec<String> = name_candidates
                .clone()
                .into_iter()
                .filter_map(|nc| extract_domain(nc))
                .collect();

            let company_name = extract_company_domain(&ss.name, domains.clone());

            if !BLACK_LIST_DOMAINS
                .iter()
                .any(|&blacklist| company_name.contains(blacklist))
            {
                for query in build_founder_seach_queries(&company_name) {
                    founder_query_sender
                        .send(FounderQueryChannelData {
                            query,
                            domain: company_name.clone(),
                        })
                        .unwrap();
                }
            }

            if let Err(e) =
                persistant_data_sender.send(PersistantData::CompanyName(CompanyNameData::Result {
                    query,
                    page_source,
                    page_number: 1,
                    html_tags: name_candidates,
                    company_name,
                }))
            {
                log::error!(
                    "Persistant data sender channel got an Error: {:?} | Source: {:?}",
                    e,
                    e.source(),
                );
            }
        }
    }
}
