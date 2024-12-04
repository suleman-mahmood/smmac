use thirtyfour::{DesiredCapabilities, WebDriver};

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
}
