use fake_user_agent::get_rua;
use rand::seq::SliceRandom;
use thirtyfour::{
    CapabilitiesHelper, ChromiumLikeCapabilities, DesiredCapabilities, Proxy, WebDriver,
};

const NUM_PARALLEL_DRIVERS: u8 = 10_u8;

const PROXIES: [&str; 10] = [
    "198.23.239.134:6540",
    "207.244.217.165:6712",
    "107.172.163.27:6543",
    "64.137.42.112:5157",
    "173.211.0.148:6641",
    "161.123.152.115:6360",
    "167.160.180.203:6754",
    "154.36.110.199:6853",
    "173.0.9.70:5653",
    "173.0.9.209:5792",
];

pub struct Droid {
    pub drivers: Vec<WebDriver>,
}

impl Droid {
    pub async fn new() -> Self {
        let mut drivers: Vec<WebDriver> = vec![];
        for _ in 0..NUM_PARALLEL_DRIVERS {
            let mut caps = DesiredCapabilities::chrome();
            let proxy = Proxy::Manual {
                ftp_proxy: None,
                http_proxy: Some(PROXIES.choose(&mut rand::thread_rng()).unwrap().to_string()),
                ssl_proxy: Some(PROXIES.choose(&mut rand::thread_rng()).unwrap().to_string()),
                socks_proxy: None,
                socks_version: None,
                socks_username: None,
                socks_password: None,
                no_proxy: None,
            };
            caps.set_proxy(proxy).unwrap();

            caps.add_arg(&format!("--user-agent={}", get_rua()))
                .unwrap();

            // http://chrome:4444/wd/hub
            // http://localhost:58656
            let new_driver = WebDriver::new("http://localhost:63364", caps)
                .await
                .unwrap();
            new_driver.maximize_window().await.unwrap();
            drivers.push(new_driver);
        }

        Droid { drivers }
    }
}
