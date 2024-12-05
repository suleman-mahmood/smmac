use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};

const NUM_PARALLEL_DRIVERS: u8 = 10_u8;

pub struct Droid {
    pub drivers: Vec<WebDriver>,
}

impl Droid {
    pub async fn new() -> Self {
        let mut caps = DesiredCapabilities::chrome();

        let proxy = Proxy::Manual {
            ftp_proxy: None,
            http_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
            ssl_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
            socks_proxy: None,
            socks_version: None,
            socks_username: None,
            socks_password: None,
            no_proxy: None,
        };
        caps.set_proxy(proxy).unwrap();

        let mut drivers: Vec<WebDriver> = vec![];
        for _ in 0..NUM_PARALLEL_DRIVERS {
            // http://chrome:4444/wd/hub
            // http://localhost:58656
            let new_driver = WebDriver::new("http://localhost:62510", caps.clone())
                .await
                .unwrap();
            new_driver.maximize_window().await.unwrap();
            drivers.push(new_driver);
        }

        Droid { drivers }
    }
}
