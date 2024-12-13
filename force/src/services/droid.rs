use fake_user_agent::get_rua;
use rand::seq::SliceRandom;
use serde_json::json;
use thirtyfour::{
    CapabilitiesHelper, ChromiumLikeCapabilities, DesiredCapabilities, Proxy, WebDriver,
};
use tokio::sync::Mutex;

const NUM_PARALLEL_DRIVERS: u8 = 1_u8;

const PROXIES: [&str; 100] = [
    "69.12.93.165:6185",
    "162.218.208.190:6309",
    "45.115.195.175:6153",
    "166.88.235.135:5763",
    "173.239.237.162:5808",
    "193.178.227.141:6172",
    "147.185.217.252:6686",
    "193.178.227.118:6149",
    "147.185.217.211:6645",
    "89.249.198.217:6303",
    "154.22.56.46:5087",
    "38.170.176.69:5464",
    "145.223.54.14:5979",
    "145.223.54.126:6091",
    "192.241.125.224:8268",
    "147.185.217.120:6554",
    "192.241.125.89:8133",
    "204.44.91.87:6606",
    "148.135.148.232:6225",
    "103.251.223.222:6201",
    "69.58.9.57:7127",
    "185.226.205.5:5537",
    "142.111.255.22:5311",
    "104.232.209.163:6121",
    "204.44.91.97:6616",
    "69.12.93.127:6147",
    "204.44.108.210:6231",
    "185.171.252.222:6753",
    "156.243.181.108:5596",
    "207.244.218.46:5654",
    "192.241.118.52:8619",
    "142.147.131.151:6051",
    "192.210.191.151:6137",
    "145.223.53.225:6759",
    "184.174.46.192:5821",
    "204.44.121.182:6435",
    "192.3.48.228:6221",
    "45.249.59.28:6004",
    "166.88.235.55:5683",
    "45.127.250.248:5857",
    "185.171.254.247:6279",
    "45.249.59.138:6114",
    "23.27.78.213:5793",
    "204.44.91.73:6592",
    "66.78.34.73:5692",
    "67.227.42.130:6107",
    "156.243.183.243:5732",
    "166.88.238.16:5996",
    "45.141.80.242:5968",
    "64.64.115.238:5873",
    "67.227.42.175:6152",
    "173.245.88.206:5509",
    "64.64.127.43:5996",
    "45.127.250.148:5757",
    "185.216.106.104:6181",
    "103.251.223.124:6103",
    "107.173.105.84:5771",
    "173.214.176.172:6143",
    "38.153.133.135:9539",
    "168.199.244.227:6759",
    "192.241.118.128:8695",
    "142.147.129.10:5619",
    "145.223.51.206:6739",
    "216.173.84.184:6099",
    "192.186.172.172:9172",
    "66.78.32.211:5261",
    "103.251.223.46:6025",
    "198.55.106.80:5598",
    "45.81.149.111:6543",
    "67.227.14.80:6672",
    "38.170.176.163:5558",
    "179.61.166.60:6483",
    "216.74.118.127:6282",
    "156.243.181.10:5498",
    "92.112.174.241:5825",
    "45.249.59.223:6199",
    "145.223.54.25:5990",
    "204.44.92.204:8234",
    "89.249.192.121:6520",
    "104.238.10.225:6171",
    "89.249.192.125:6524",
    "148.135.151.83:5576",
    "166.88.224.183:6081",
    "64.64.115.119:5754",
    "156.243.178.88:7076",
    "69.58.9.110:7180",
    "185.226.205.97:5629",
    "136.0.88.18:5076",
    "136.0.88.157:5215",
    "174.140.254.43:6634",
    "92.113.246.247:5832",
    "192.241.112.125:7627",
    "166.88.195.69:5701",
    "212.42.203.68:6116",
    "45.81.149.14:6446",
    "168.199.244.243:6775",
    "204.44.109.116:5637",
    "198.46.241.202:6737",
    "184.174.46.243:5872",
    "92.113.242.8:6592",
];
pub struct Droid {
    pub drivers: Mutex<Vec<WebDriver>>,
}

impl Droid {
    pub async fn new() -> Self {
        let mut drivers: Vec<WebDriver> = vec![];
        for _ in 0..NUM_PARALLEL_DRIVERS {
            let new_driver = make_new_driver().await;
            drivers.push(new_driver);
        }

        Droid {
            drivers: Mutex::new(drivers),
        }
    }
}

pub fn get_random_proxy() -> String {
    PROXIES.choose(&mut rand::thread_rng()).unwrap().to_string()
}

pub async fn make_new_driver() -> WebDriver {
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
    caps.set_headless().unwrap();

    caps.add_experimental_option(
        "prefs",
        json!({
            "profile.managed_default_content_settings.images": 2,
            "profile.managed_default_content_settings.cookies": 2,
            "profile.managed_default_content_settings.javascript": 2,
            "profile.managed_default_content_settings.media_stream": 2,
            "profile.managed_default_content_settings.media_stream_mic": 2,
            "profile.managed_default_content_settings.media_stream_camera": 2,
            "profile.managed_default_content_settings.plugins": 2,
            "profile.managed_default_content_settings.popups": 2,
            "profile.managed_default_content_settings.geolocation": 2,
            "profile.managed_default_content_settings.notifications": 2,
            "profile.managed_default_content_settings.auto_select_certificate": 2,
            "profile.managed_default_content_settings.fullscreen": 2,
            "profile.managed_default_content_settings.mouselock": 2,
            "profile.managed_default_content_settings.mixed_script": 2,
            "profile.managed_default_content_settings.protocol_handlers": 2,
            "profile.managed_default_content_settings.ppapi_broker": 2,
            "profile.managed_default_content_settings.automatic_downloads": 2,
            "profile.managed_default_content_settings.midi_sysex": 2,
            "profile.managed_default_content_settings.push_messaging": 2,
            "profile.managed_default_content_settings.ssl_cert_decisions": 2,
            "profile.managed_default_content_settings.metro_switch_to_desktop": 2,
            "profile.managed_default_content_settings.protected_media_identifier": 2,
            "profile.managed_default_content_settings.app_banner": 2,
            "profile.managed_default_content_settings.site_engagement": 2,
            "profile.managed_default_content_settings.durable_storage": 2,
        }),
    )
    .unwrap();

    caps.add_arg("start-maximized").unwrap();
    caps.add_arg("disable-infobars").unwrap();
    caps.add_arg("--disable-extensions").unwrap();
    caps.add_arg(&format!("--user-agent={}", get_rua()))
        .unwrap();

    // http://chrome:4444/wd/hub
    // http://localhost:63364
    let new_driver = WebDriver::new("http://localhost:63364", caps)
        .await
        .unwrap();
    new_driver.maximize_window().await.unwrap();

    new_driver
}
