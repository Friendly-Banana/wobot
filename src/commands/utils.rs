use crate::constants::{HTTP_CLIENT, ONE_DAY};
use crate::{Context, Error};
use anyhow::Context as _;
use chrono::{DateTime, Duration, Utc};
use fundu::DurationParser;
use fundu::TimeUnit::{Day, Hour, Minute, Month, Week, Year};
use image::DynamicImage;
use image::codecs::png::PngEncoder;
use mini_moka::sync::Cache;
use poise::serenity_prelude::{Colour, CreateAttachment, CreateEmbed, User};
use poise::{CreateReply, ReplyHandle};
use rand::prelude::IndexedRandom;
use rand::rng;
use std::sync::LazyLock;
use tokio::process::Command;

const COLORS: [Colour; 19] = [
    Colour::BLURPLE,
    Colour::DARK_GOLD,
    Colour::DARK_GREEN,
    Colour::BLITZ_BLUE,
    Colour::DARK_PURPLE,
    Colour::DARK_RED,
    Colour::DARK_TEAL,
    Colour::GOLD,
    Colour::MAGENTA,
    Colour::BLUE,
    Colour::ORANGE,
    Colour::PURPLE,
    Colour::RED,
    Colour::ROSEWATER,
    Colour::TEAL,
    Colour::BLITZ_BLUE,
    Colour::MEIBE_PINK,
    Colour::MAGENTA,
    Colour::FOOYOO,
];

pub(crate) fn random_color() -> Colour {
    *COLORS.choose(&mut rng()).unwrap()
}

static AVATAR_CACHE: LazyLock<Cache<String, DynamicImage>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(50 * 1024 * 1024) // 50 MB
        .time_to_idle(10 * ONE_DAY)
        .weigher(|_, v: &DynamicImage| v.as_bytes().len() as u32)
        .build()
});

pub(crate) async fn load_avatar(avatar_url: String) -> Result<DynamicImage, Error> {
    if let Some(avatar) = AVATAR_CACHE.get(&avatar_url) {
        return Ok(avatar);
    }

    let result = HTTP_CLIENT.get(&avatar_url).send().await;
    let bytes = result.context("Downloading avatar failed")?.bytes().await?;
    let avatar = image::load_from_memory(&bytes)?;
    AVATAR_CACHE.insert(avatar_url, avatar.clone());

    Ok(avatar)
}

pub(crate) async fn get_avatar_url(ctx: &Context<'_>, user: &User) -> Result<String, Error> {
    let partial_guild = ctx.partial_guild().await;
    if let Some(guild) = partial_guild {
        let member = guild.member(&ctx, user.id).await?;
        if let Some(hash) = member.avatar {
            return Ok(format!(
                "https://cdn.discordapp.com/guilds/{}/users/{}/avatars/{}.png?size=256",
                guild.id, user.id, hash
            ));
        }
    }

    if let Some(hash) = &user.avatar {
        return Ok(format!(
            "https://cdn.discordapp.com/avatars/{}/{}.png?size=256",
            user.id, hash
        ));
    }

    let mut url = user.default_avatar_url();
    url.push_str("?size=256");
    Ok(url)
}

pub(crate) async fn remove_components_but_keep_embeds(
    ctx: Context<'_>,
    mut m: CreateReply,
    reply: ReplyHandle<'_>,
) -> Result<(), Error> {
    let original = reply.message().await?;
    m = m.components(Vec::new());
    m.embeds = original
        .embeds
        .iter()
        .cloned()
        .map(CreateEmbed::from)
        .collect();
    reply.edit(ctx, m).await?;
    Ok(())
}

pub async fn send_image(
    ctx: Context<'_>,
    img: DynamicImage,
    filename: String,
) -> Result<(), Error> {
    let mut output_bytes: Vec<u8> = Vec::new();
    img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

    ctx.send(CreateReply::default().attachment(CreateAttachment::bytes(output_bytes, filename)))
        .await?;
    Ok(())
}

pub const DURATION_PARSER: DurationParser = DurationParser::builder()
    .disable_infinity()
    .default_unit(Hour)
    .allow_time_unit_delimiter()
    .parse_multiple(Some(&[]))
    .time_units(&[Minute, Hour, Day, Week, Month, Year])
    .build();

/// Parse a date/datetime string using the `date` command with Europe/Berlin timezone
pub async fn parse_date(date: &str) -> Result<DateTime<Utc>, Error> {
    let output = Command::new("date")
        .arg("--rfc-3339=seconds")
        .arg("--date")
        .arg(format!("TZ=\"Europe/Berlin\" {}", date))
        .output()
        .await
        .context("Failed to execute date command")?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Date command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let output_str = String::from_utf8(output.stdout).context("date output is invalid UTF-8")?;
    let date = DateTime::parse_from_rfc3339(output_str.trim())
        .context("Failed to parse date output")?
        .with_timezone(&Utc);
    Ok(date)
}

/// Parse a date or duration string (e.g., "2h", "1d")
/// If the input is a duration, it's added to the current date/time
pub async fn parse_duration_or_date(
    current: DateTime<Utc>,
    add: &str,
) -> Result<DateTime<Utc>, Error> {
    let result = DURATION_PARSER.parse(add);
    if let Ok(dur) = result {
        let chrono_dur: Duration = dur.try_into()?;
        return Ok(current + chrono_dur);
    }

    parse_date(add).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::TIMEZONE;
    use chrono::{NaiveDate, TimeZone};

    const NOW: DateTime<Utc> = DateTime::from_timestamp(1, 0).unwrap();

    #[tokio::test]
    async fn test_parse_date_valid_date() {
        let date_str = "2012-03-04 05:06";
        let parsed_date = parse_date(date_str).await.unwrap();

        let naive = NaiveDate::from_ymd_opt(2012, 3, 4)
            .unwrap()
            .and_hms_opt(5, 6, 0)
            .unwrap();
        let expected = TIMEZONE
            .from_local_datetime(&naive)
            .single()
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed_date, expected);
    }

    #[tokio::test]
    async fn test_parse_date_invalid_date() {
        assert!(parse_date("not a date").await.is_err());
    }

    const YEAR: i64 = 60 * 60 * 24 * 365 + 60 * 60 * 24 / 4;
    const MONTH: i64 = YEAR / 12;

    #[tokio::test]
    async fn test_valid_duration() {
        let then = parse_duration_or_date(NOW, "2h").await.unwrap();
        assert_eq!(then, NOW + Duration::hours(2));

        let duration = parse_duration_or_date(NOW, "1y 2M 3w 4d 5h 6m")
            .await
            .unwrap();
        assert_eq!(
            duration,
            NOW + Duration::seconds(YEAR)
                + Duration::seconds(MONTH) * 2
                + Duration::days(7) * 3
                + Duration::days(4)
                + Duration::hours(5)
                + Duration::minutes(6)
        );
    }

    #[tokio::test]
    async fn test_invalid() {
        assert!(parse_duration_or_date(NOW, "invalid").await.is_err());
    }
}
