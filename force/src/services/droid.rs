use rand::seq::SliceRandom;
use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};

const PROXY_LIST: [&str; 1] = ["http://uzuugbox-rotate:mgbqddrmu9fi@p.webshare.io:80/"];

pub struct Droid {
    pub driver: WebDriver,
}

impl Droid {
    pub async fn new() -> Self {
        let caps = DesiredCapabilities::chrome();

        // http://chrome:4444/wd/hub
        // http://localhost:58656
        let driver = WebDriver::new("http://localhost:62510", caps)
            .await
            .unwrap();
        driver.maximize_window().await.unwrap();

        Droid { driver }
    }

    pub async fn new_driver_from_proxy() -> Self {
        let mut caps = DesiredCapabilities::chrome();

        let proxy = Proxy::Manual {
            ftp_proxy: None,
            http_proxy: Some(
                PROXY_LIST
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .to_string(),
            ),
            ssl_proxy: None,
            socks_proxy: None,
            socks_version: None,
            socks_username: None,
            socks_password: None,
            no_proxy: None,
        };
        caps.set_proxy(proxy).unwrap();

        // http://chrome:4444/wd/hub
        // http://localhost:58656
        let driver = WebDriver::new("http://localhost:62510", caps)
            .await
            .unwrap();
        driver.maximize_window().await.unwrap();

        Droid { driver }
    }
}
