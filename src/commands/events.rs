use std::borrow::Cow;
use std::ops::Add;

use anyhow::Context as _;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use chrono_tz::Tz;
use ics::properties::{Description, DtEnd, DtStart, Location, Summary};
use ics::{Event, ICalendar};
use image::EncodableLayout;
use poise::serenity_prelude::{AttachmentType, ReactionType, ScheduledEventType};
use tracing::error;

use crate::constants::{TIMEZONE, TIME_INPUT_FORMAT};
use crate::{done, Context, Error};

const EVENT_URL: &str = "https://discord.com/events/";

/// Export all events on this server as ICS calendar file
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn export_events(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;
    let events = ctx
        .guild_id()
        .expect("guild_only")
        .scheduled_events(ctx.http(), false)
        .await?;
    let mut calendar = ICalendar::new("2.0", "ics-rs");
    for event in events {
        let mut ics_event = Event::new(
            event.id.0.to_string(),
            Utc::now().format(TIME_INPUT_FORMAT).to_string(),
        );

        ics_event.push(Summary::new(event.name));
        if let Some(description) = event.description {
            ics_event.push(Description::new(description));
        }
        if let Some(metadata) = event.metadata {
            ics_event.push(Location::new(metadata.location));
        }
        ics_event.push(DtStart::new(
            event.start_time.format(TIME_INPUT_FORMAT).to_string(),
        ));
        ics_event.push(DtEnd::new(
            event
                .end_time
                .unwrap_or(event.start_time.add(Duration::hours(1)).into())
                .format(TIME_INPUT_FORMAT)
                .to_string(),
        ));

        calendar.add_event(ics_event);
    }
    let mut bytes = Vec::new();
    calendar.write(&mut bytes)?;
    ctx.send(|r| {
        r.attachment(AttachmentType::Bytes {
            filename: "calendar.ics".to_string(),
            data: Cow::from(bytes.as_bytes()),
        })
    })
    .await?;
    done!(ctx);
}

/// Create a new meetup
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    ephemeral,
    required_permissions = "MANAGE_EVENTS"
)]
pub(crate) async fn event(
    ctx: Context<'_>,
    name: String,
    location: String,
    #[description = "yyyy-mm-dd hh:mm, example: 2012-12-21 12:34"] start: String,
    #[description = "yyyy-mm-dd hh:mm, default start_time + 1h"] end: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let start_date = parse_date(&start, "start")?;
    let end_date = match end {
        None => start_date.add(Duration::hours(1)),
        Some(input) => parse_date(&input, "end")?,
    };
    let guild_id = ctx.guild_id().context("guild_only")?;
    let event = guild_id
        .create_scheduled_event(ctx.http(), |e| {
            e.kind(ScheduledEventType::External)
                .name(&name)
                .location(location)
                .start_time(start_date)
                .end_time(end_date)
        })
        .await?;
    let announcement = format!(
        "[{}]({}{}/{}) mit {}",
        name,
        EVENT_URL,
        event.guild_id,
        event.id,
        ctx.author()
    );
    let announcement_channel = ctx.data().announcement_channel.get(&guild_id);
    if announcement_channel.is_none() {
        error!("No announcement channel configured for guild {guild_id}");
        return Err(Error::from(format!(
            "No announcement channel configured for guild {guild_id}"
        )));
    };
    let msg = announcement_channel
        .unwrap()
        .say(ctx.http(), announcement)
        .await?;
    msg.react(ctx.http(), ReactionType::from('👍')).await?;
    msg.react(ctx.http(), ReactionType::from('❔')).await?;

    let thread = announcement_channel
        .unwrap()
        .create_public_thread(ctx.http(), msg.id, |t| t.name(name))
        .await?;
    thread.id.add_thread_member(ctx, ctx.author().id).await?;
    done!(ctx);
}

fn parse_date(input: &str, name: &str) -> Result<DateTime<Tz>, Error> {
    match NaiveDateTime::parse_from_str(&input, "%Y-%m-%d %H:%M") {
        Ok(date) => Ok(date.and_local_timezone(TIMEZONE).unwrap()),
        Err(e) => {
            error!("Couldn't parse start time {input}: {e:?}");
            Err(Error::from(format!("Couldn't parse {name} time")))
        }
    }
}
