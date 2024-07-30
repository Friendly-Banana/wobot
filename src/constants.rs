use std::fs::File;
use std::io::{BufReader, Read};
use std::time::Duration;

use chrono_tz::Europe::Berlin;
use chrono_tz::Tz;
use image::Rgba;
use once_cell::sync::Lazy;
use reqwest::Client as ReqwestClient;
use rusttype::Font;
use tracing::info;

pub(crate) const TIMEZONE: Tz = Berlin;
pub(crate) const ONE_HOUR: Duration = Duration::from_secs(60 * 60);
pub(crate) const ONE_DAY: Duration = Duration::from_secs(24 * 60 * 60);
pub(crate) const ONE_YEAR: Duration = Duration::from_secs(365 * 24 * 60 * 60);

pub(crate) const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
pub(crate) const FONT_PATH: &str = "assets/Rockwill.ttf";
pub(crate) static FONT: Lazy<Font> = Lazy::new(|| {
    info!("Loading font");
    let mut font_data = Vec::new();
    BufReader::new(
        File::open(FONT_PATH).unwrap_or_else(|_| panic!("Path to font is wrong {:?}", FONT_PATH)),
    )
    .read_to_end(&mut font_data)
    .expect("Failed to read font");
    Font::try_from_vec(font_data).expect("Failed to parse font")
});

pub(crate) static HTTP_CLIENT: Lazy<ReqwestClient> = Lazy::new(ReqwestClient::new);
