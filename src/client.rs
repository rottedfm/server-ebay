use anyhow::{Context, Result};
use fantoccini::elements::Element;
use fantoccini::{Client as FantocciniClient, ClientBuilder, Locator};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use tokio::time::sleep;

#[derive(Debug)]
pub struct BrowserClient {
    xvfb: Child,
    x11vnc: Child,
    driver: Child,
    client: FantocciniClient,
    _profile_dir: TempDir,
}

// Represents a scraped eBay listing
#[derive(Debug, Serialize, Deserialize)]
pub struct Listing {
    pub title: String,
    pub description: String,
    pub condition: String,
    pub item_id: String,
    pub price: String,
    pub images: Vec<String>,
    pub views: String,
    pub watchers: String,
}

impl BrowserClient {
    pub async fn build() -> Result<Self> {
        let display_num = 99;
        let display_env = format!(":{}", display_num);

        info!("Starting Xvfb on DISPLAY={}", display_env);
        let xvfb = Command::new("Xvfb")
            .arg(&display_env)
            .arg("-screen")
            .arg("0")
            .arg("1280x1024x24")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start Xvfb")?;

        info!("Starting x11vnc on DISPLAY={}", display_env);
        let x11vnc = Command::new("x11vnc")
            .arg("-display")
            .arg(&display_env)
            .arg("-nopw")
            .arg("-forever")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start x11vnc")?;

        let profile_dir =
            Self::create_firefox_profile().context("Failed to create Firefox profile")?;

        let geckodriver_path =
            which::which("geckodriver").context("Could not find 'geckodriver' in PATH")?;

        info!("Starting geckodriver");
        let driver = Command::new(&geckodriver_path)
            .arg("--port")
            .arg("4444")
            .env("DISPLAY", &display_env)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start geckodriver")?;

        sleep(tokio::time::Duration::from_secs(2)).await;

        let client = ClientBuilder::native()
            .connect("http://localhost:4444")
            .await
            .context("Failed to connect to geckodriver")?;

        let pid_info = format!("{}\n{}\n{}\n", driver.id(), xvfb.id(), x11vnc.id());
        fs::write("/tmp/ebay_driver_pids", pid_info).context("Failed to write PID file")?;

        info!(
            "BrowserClient initialized with geckodriver PID {}, xvfb PID {}, x11vnc PID {}",
            driver.id(),
            xvfb.id(),
            x11vnc.id()
        );
        Ok(Self {
            xvfb,
            x11vnc,
            driver,
            client,
            _profile_dir: profile_dir,
        })
    }

    pub async fn ebay_signin(&mut self, email: &str, password: &str) -> Result<()> {
        info!("Navigating to https://signin.ebay.com/signin/...");
        self.client
            .goto("https://signin.ebay.com/signin/")
            .await
            .context("Failed to navigate")?;

        sleep(tokio::time::Duration::from_secs(2)).await;

        self.wait_if_captcha_detected()
            .await
            .context("Failed to detect captcha")?;

        let email_field: Element = self
            .client
            .wait()
            .for_element(fantoccini::Locator::Css("#userid"))
            .await
            .context("Failed to wait for #userid")?;

        email_field
            .send_keys(email)
            .await
            .context("Failed to send_keys to #userid")?;

        self.client
            .find(Locator::Css("#signin-continue-btn"))
            .await
            .context("Failed to find #signin-continue-btn")?
            .click()
            .await
            .context("Failed to click #sigin-continue-btn")?;

        Ok(())
    }

    async fn wait_if_captcha_detected(&mut self) -> Result<()> {
        let current_url = self.client.current_url().await?.to_string();

        if current_url.to_lowercase().contains("captcha") {
            info!("CAPTCHA detected. Please wait for buster or manually solve it...");

            loop {
                sleep(tokio::time::Duration::from_secs(2)).await;
                let new_url = self.client.current_url().await?.to_string();

                if new_url != current_url && !new_url.to_lowercase().contains("captcha") {
                    info!("CAPTCHA cleared. Continuing...");
                    break;
                }
            }
        }

        Ok(())
    }

    fn create_firefox_profile() -> Result<TempDir> {
        let dir = tempfile::tempdir().context("Failed to create temporary profile dir")?;
        let profile_path = dir.path();

        let user_js = profile_path.join("user.js");
        let prefs = r#"
// --- Anti-fingerprinting ---
user_pref("general.useragent.override", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36");
user_pref("general.platform.override", "Win32");
user_pref("intl.accept_languages", "en-US,en");

// --- Disable automation detection ---
user_pref("dom.webdriver.enabled", false);
user_pref("useAutomationExtension", false);
user_pref("media.navigator.enabled", false);
user_pref("media.peerconnection.enabled", false);
user_pref("privacy.resistFingerprinting", false);

// --- Fingerprint surfaces ---
user_pref("canvas.capturestream.enabled", false);
user_pref("canvas.logging.enabled", false);
user_pref("gfx.offscreencanvas.enabled", false);
user_pref("dom.battery.enabled", false);
user_pref("dom.gamepad.enabled", false);

// --- Startup noise ---
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("startup.homepage_welcome_url", "");
user_pref("startup.homepage_welcome_url.additional", "");
user_pref("browser.aboutwelcome.enabled", false);

// --- Geolocation & tracking ---
user_pref("geo.enabled", false);
user_pref("privacy.trackingprotection.enabled", false);

// --- Referrer spoofing ---
user_pref("network.http.referer.XOriginPolicy", 0);
user_pref("network.http.referer.trimmingPolicy", 0);
user_pref("network.http.sendRefererHeader", 2);
"#;
        fs::write(&user_js, prefs).context("Failed to write user.js")?;

        let extensions_dir = profile_path.join("extensions");
        fs::create_dir_all(&extensions_dir).context("Failed to create extensions directory")?;

        let buster_xpi_path = PathBuf::from("./resources/buster.xpi");
        if !buster_xpi_path.exists() {
            error!("Missing Buster extension at {}", buster_xpi_path.display());
            return Err(anyhow::anyhow!(
                "Missing Buster extension at {}",
                buster_xpi_path.display()
            ));
        }

        let buster_target = extensions_dir.join("buster@dessant.xpi");
        fs::copy(&buster_xpi_path, &buster_target).context("Failed to copy Buster extension")?;

        Ok(dir)
    }

    pub async fn teardown() -> Result<()> {
        info!("Shutting down browser processes using PID file");
        let content = fs::read_to_string("/tmp/ebay_driver_pids")
            .context("PID file not found. Is the browser running?")?;
        let mut lines = content.lines();

        let geckodriver_pid: u32 = lines
            .next()
            .unwrap_or("0")
            .parse()
            .context("Invalid geckodriver PID")?;
        let xvfb_pid: u32 = lines
            .next()
            .unwrap_or("0")
            .parse()
            .context("Invalid Xvfb PID")?;
        let x11vnc_pid: u32 = lines
            .next()
            .unwrap_or("0")
            .parse()
            .context("Invalid x11vnc PID")?;

        for (name, pid) in [
            ("geckodriver", geckodriver_pid),
            ("Xvfb", xvfb_pid),
            ("x11vnc", x11vnc_pid),
        ] {
            if pid != 0 {
                match Command::new("kill").arg("-9").arg(pid.to_string()).status() {
                    Ok(status) if status.success() => info!("Killed {} with PID {}", name, pid),
                    Ok(status) => error!(
                        "Failed to kill {} with PID {}. Exit code: {:?}",
                        name,
                        pid,
                        status.code()
                    ),
                    Err(e) => error!("Error running kill command for {}: {}", name, e),
                }
            } else {
                error!("{} PID is 0 or invalid, skipping kill", name);
            }
        }

        fs::remove_file("/tmp/ebay_driver_pids").ok();
        info!("PID file removed and teardown complete");
        Ok(())
    }
}
