use std::borrow::Cow;

use crate::commands::utils::{parse_date, parse_duration_or_date};
use crate::{Context, Error, done};
use chrono::Duration;
use chrono::Utc;
use ics::properties::{Description, DtEnd, DtStart, Location, Summary};
use ics::{Event, ICalendar};
use image::EncodableLayout;
use poise::serenity_prelude::{
    CreateAttachment, CreateScheduledEvent, CreateThread, ScheduledEventType,
};
use std::ops::Add;

const EVENT_URL: &str = "https://discord.com/events/";

/// Export all events on this server as an ICS calendar file
#[poise::command(slash_command, prefix_command, guild_only)]
pub(crate) async fn export_events(ctx: Context<'_>) -> Result<(), Error> {
    const ICS_TIME_FORMAT: &str = "%Y%m%dT%H%M%SZ";

    ctx.defer().await?;
    let events = ctx
        .guild_id()
        .expect("guild_only")
        .scheduled_events(ctx.http(), false)
        .await?;
    let mut calendar = ICalendar::new("2.0", "ics-rs");
    for event in events {
        let mut ics_event = Event::new(
            event.id.get().to_string(),
            Utc::now().format(ICS_TIME_FORMAT).to_string(),
        );

        ics_event.push(Summary::new(event.name));
        if let Some(description) = event.description {
            ics_event.push(Description::new(description));
        }
        if let Some(metadata) = event.metadata
            && let Some(loc) = metadata.location
        {
            ics_event.push(Location::new(loc));
        }
        ics_event.push(DtStart::new(
            event.start_time.format(ICS_TIME_FORMAT).to_string(),
        ));
        ics_event.push(DtEnd::new(
            event
                .end_time
                .unwrap_or(event.start_time.add(Duration::hours(1)).into())
                .format(ICS_TIME_FORMAT)
                .to_string(),
        ));

        calendar.add_event(ics_event);
    }
    let mut bytes = Vec::new();
    calendar.write(&mut bytes)?;
    ctx.send(CreateReply::default().attachment(CreateAttachment::bytes(
        Cow::from(bytes.as_bytes()),
        "calendar.ics".to_string(),
    )))
    .await?;
    done!(ctx);
}

/// Create a new event, channel and announcement
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
    #[description = "date(time) like today 5pm or 2024-12-31 18:00"] start: String,
    #[description = "date(time) or duration, default start + 1 hour"] end: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let start = parse_date(&start).await?;
    let end = if let Some(datestr) = &end {
        parse_duration_or_date(start, datestr).await?
    } else {
        start + Duration::hours(1)
    };

    let guild_id = ctx.guild_id().expect("guild_only");
    let new_event = CreateScheduledEvent::new(ScheduledEventType::External, &name, start)
        .location(location)
        .end_time(end);
    let event = guild_id
        .create_scheduled_event(ctx.http(), new_event)
        .await?;
    let announcement = format!(
        "[{}]({}{}/{}) mit {}",
        name,
        EVENT_URL,
        event.guild_id,
        event.id,
        ctx.author()
    );
    let announcement_channel = ctx.data().event_channel_per_guild.get(&guild_id);
    match announcement_channel {
        None => {
            ctx.reply("Event has been created. To also send an announcement and create a thread, configure a channel for this server").await?;
            Ok(())
        }
        Some(channel) => {
            let msg = channel.say(ctx.http(), announcement).await?;
            let thread = channel
                .create_thread_from_message(ctx.http(), msg.id, CreateThread::new(name))
                .await?;
            thread.id.add_thread_member(ctx, ctx.author().id).await?;
            done!(ctx);
        }
    }
}
