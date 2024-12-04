use thirtyfour::{CapabilitiesHelper, DesiredCapabilities, Proxy, WebDriver};

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
            http_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
            ssl_proxy: Some("http://zqsggygg-rotate:ty7ut0nxi4yp@p.webshare.io:80/".to_string()),
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
