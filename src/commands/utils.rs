use crate::constants::{HTTP_CLIENT, ONE_DAY};
use crate::{Context, Data, UserError};
use anyhow::Context as _;
use chrono::{DateTime, Utc};
use image::DynamicImage;
use image::codecs::png::PngEncoder;
use mini_moka::sync::Cache;
use poise::serenity_prelude::{
    Colour, CreateAttachment, CreateEmbed, EmojiId, GuildId, MESSAGE_CODE_LIMIT, ReactionType, User,
};
use poise::{CreateReply, ReplyHandle};
use rand::prelude::IndexedRandom;
use rand::rng;
use sqlx::query;
use std::collections::VecDeque;
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

pub(crate) async fn load_avatar(avatar_url: String) -> anyhow::Result<DynamicImage> {
    if let Some(avatar) = AVATAR_CACHE.get(&avatar_url) {
        return Ok(avatar);
    }

    let result = HTTP_CLIENT.get(&avatar_url).send().await;
    let bytes = result.context("Downloading avatar failed")?.bytes().await?;
    let avatar = image::load_from_memory(&bytes)?;
    AVATAR_CACHE.insert(avatar_url, avatar.clone());

    Ok(avatar)
}

pub(crate) async fn get_avatar_url(ctx: &Context<'_>, user: &User) -> anyhow::Result<String> {
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
) -> anyhow::Result<()> {
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
) -> anyhow::Result<()> {
    let mut output_bytes: Vec<u8> = Vec::new();
    img.write_with_encoder(PngEncoder::new(&mut output_bytes))?;

    ctx.send(CreateReply::default().attachment(CreateAttachment::bytes(output_bytes, filename)))
        .await?;
    Ok(())
}

pub async fn get_emoji_id(reaction: &ReactionType, data: &Data) -> anyhow::Result<i64> {
    match reaction {
        ReactionType::Custom { id, .. } => Ok(id.get() as i64),
        ReactionType::Unicode(unicode) => {
            query!(
                "INSERT INTO unicode_to_emoji (unicode) VALUES ($1) ON CONFLICT DO NOTHING",
                unicode
            )
            .execute(&data.database)
            .await?;
            let id = query!(
                "SELECT id FROM unicode_to_emoji WHERE unicode = $1",
                unicode
            )
            .fetch_one(&data.database)
            .await
            .context(format!("Emoji {unicode} should have an id now"))?;
            Ok(id.id)
        }
        _ => unimplemented!(),
    }
}

pub async fn get_emoji_from_id(
    ctx: Context<'_>,
    guild_id: i64,
    emoji_id: i64,
) -> anyhow::Result<String> {
    if emoji_id >> 32 == 0 {
        let emoji = query!(
            "SELECT unicode FROM unicode_to_emoji WHERE id = $1",
            emoji_id
        )
        .fetch_one(&ctx.data().database)
        .await
        .with_context(|| format!("Emoji {emoji_id} should be in the database"))?;
        return Ok(emoji.unicode);
    }
    Ok(GuildId::new(guild_id as u64)
        .emoji(ctx.http(), EmojiId::new(emoji_id as u64))
        .await
        .map(|e| e.to_string())
        .unwrap_or(emoji_id.to_string()))
}

/// Split text into multiple messages to stay under Discord's limit
pub async fn paginate_text(ctx: Context<'_>, lines: &mut VecDeque<String>) -> anyhow::Result<()> {
    let mut s = lines.pop_front().unwrap();
    loop {
        match lines.pop_front() {
            Some(line) => {
                // we'll add a newline
                if line.len() + s.len() + 1 > MESSAGE_CODE_LIMIT {
                    ctx.reply(&s).await?;
                    s = String::new();
                }
                s.push('\n');
                s.push_str(&line);
            }
            None => {
                ctx.send(CreateReply::default().content(s)).await?;
                break;
            }
        }
    }
    Ok(())
}

/// Parse a date/datetime string using the `date` command with Europe/Berlin timezone
pub async fn parse_date(date: &str) -> anyhow::Result<DateTime<Utc>> {
    let output = Command::new("date")
        .arg("--rfc-3339=seconds")
        .arg("--date")
        .arg(format!("TZ=\"Europe/Berlin\" {}", date))
        .output()
        .await
        .context("Failed to execute date command")?;

    if !output.status.success() {
        return Err(UserError::err(format!(
            "Date command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let output_str = String::from_utf8(output.stdout).context("date output is invalid UTF-8")?;
    let date = DateTime::parse_from_rfc3339(output_str.trim())
        .context("Failed to parse date output")?
        .with_timezone(&Utc);
    Ok(date)
}

/// If the input is a duration, it's added to the current date/time
pub async fn parse_duration_or_date(
    current: DateTime<Utc>,
    add: &str,
) -> anyhow::Result<DateTime<Utc>> {
    let dur_result = parse_duration::parse(add);
    if let Ok(dur) = dur_result {
        return Ok(current + dur);
    }
    let date_result = parse_date(add).await;
    if let Ok(date) = date_result {
        return Ok(date);
    }
    Err(UserError::err(format!(
        "Not a date or duration: {}, {}",
        date_result.unwrap_err(),
        dur_result.unwrap_err()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::TIMEZONE;
    use chrono::{Duration, NaiveDate, TimeZone};

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

    const YEAR: Duration = Duration::seconds(31_556_952);
    const MONTH: Duration = Duration::seconds(2_629_746);

    #[tokio::test]
    async fn test_valid_duration() {
        let then = parse_duration_or_date(NOW, "2h").await.unwrap();
        assert_eq!(then, NOW + Duration::hours(2));

        let duration = parse_duration_or_date(NOW, "1y 2M 3w 4d 5h 6m")
            .await
            .unwrap();
        assert_eq!(
            duration,
            NOW + YEAR
                + MONTH * 2
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
