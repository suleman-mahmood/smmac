use rand::seq::SliceRandom;
use serde_json::json;
use thirtyfour::{
    CapabilitiesHelper, ChromiumLikeCapabilities, DesiredCapabilities, Proxy, WebDriver,
};
use tokio::sync::Mutex;

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
    pub drivers: Mutex<Vec<WebDriver>>,
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

            // All available options
            // "profile.managed_default_content_settings.cookies": 2,
            // "profile.managed_default_content_settings.javascript": 2,
            // "profile.managed_default_content_settings.plugins": 2,
            // "profile.managed_default_content_settings.popups": 2,
            // "profile.managed_default_content_settings.geolocation": 2,
            // "profile.managed_default_content_settings.notifications": 2,
            // "profile.managed_default_content_settings.auto_select_certificate": 2,
            // "profile.managed_default_content_settings.fullscreen": 2,
            // "profile.managed_default_content_settings.mouselock": 2,
            // "profile.managed_default_content_settings.mixed_script": 2,
            // "profile.managed_default_content_settings.media_stream": 2,
            // "profile.managed_default_content_settings.media_stream_mic": 2,
            // "profile.managed_default_content_settings.media_stream_camera": 2,
            // "profile.managed_default_content_settings.protocol_handlers": 2,
            // "profile.managed_default_content_settings.ppapi_broker": 2,
            // "profile.managed_default_content_settings.automatic_downloads": 2,
            // "profile.managed_default_content_settings.midi_sysex": 2,
            // "profile.managed_default_content_settings.push_messaging": 2,
            // "profile.managed_default_content_settings.ssl_cert_decisions": 2,
            // "profile.managed_default_content_settings.metro_switch_to_desktop": 2,
            // "profile.managed_default_content_settings.protected_media_identifier": 2,
            // "profile.managed_default_content_settings.app_banner": 2,
            // "profile.managed_default_content_settings.site_engagement": 2,
            // "profile.managed_default_content_settings.durable_storage": 2,

            caps.add_experimental_option(
                "prefs",
                json!({"profile.managed_default_content_settings.images": 2}),
            )
            .unwrap();

            // caps.add_arg("start-maximized").unwrap();
            // caps.add_arg("disable-infobars").unwrap();
            // caps.add_arg("--disable-extensions").unwrap();
            // caps.add_arg(&format!("--user-agent={}", get_rua()))
            //     .unwrap();

            // http://chrome:4444/wd/hub
            // http://localhost:58656
            let new_driver = WebDriver::new("http://localhost:63364", caps)
                .await
                .unwrap();
            new_driver.maximize_window().await.unwrap();
            drivers.push(new_driver);
        }

        Droid {
            drivers: Mutex::new(drivers),
        }
    }
}
