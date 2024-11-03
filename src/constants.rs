use ab_glyph::FontRef;
use chrono_tz::Europe::Berlin;
use chrono_tz::Tz;
use image::Rgba;
use reqwest::Client as ReqwestClient;
use std::fs::File;
use std::io::{BufReader, Read};
use std::sync::LazyLock;
use std::time::Duration;
use tracing::info;

pub(crate) const TIMEZONE: Tz = Berlin;
pub(crate) const ONE_HOUR: Duration = Duration::from_secs(60 * 60);
pub(crate) const ONE_DAY: Duration = Duration::from_secs(24 * 60 * 60);
pub(crate) const ONE_YEAR: Duration = Duration::from_secs(365 * 24 * 60 * 60);

pub(crate) const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
pub(crate) const FONT_PATH: &str = "assets/rockwill.ttf";
static FONT_DATA: LazyLock<Vec<u8>> = LazyLock::new(|| {
    info!("Loading font data");
    let mut font_data = Vec::new();
    BufReader::new(
        File::open(FONT_PATH).unwrap_or_else(|_| panic!("Path to font is wrong {:?}", FONT_PATH)),
    )
    .read_to_end(&mut font_data)
    .expect("Failed to read font");
    font_data
});
pub(crate) static FONT: LazyLock<FontRef> =
    LazyLock::new(|| FontRef::try_from_slice(FONT_DATA.as_slice()).expect("Failed to parse font"));

static USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

pub(crate) static HTTP_CLIENT: LazyLock<ReqwestClient> = LazyLock::new(|| {
    ReqwestClient::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(USER_AGENT)
        .build()
        .expect("HTTP client")
});
