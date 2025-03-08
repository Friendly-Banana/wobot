use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fs::read_to_string;
use std::sync::{Arc, RwLock};
use std::time::Duration;

#[cfg(feature = "activity")]
use crate::check_access::check_access;
use crate::check_birthday::check_birthdays;
use crate::check_reminder::check_reminders;
use crate::commands::*;
#[cfg(feature = "activity")]
use crate::constants::ONE_DAY;
use anyhow::Context as _;
use itertools::Itertools;
#[cfg(feature = "activity")]
use mini_moka::sync::{Cache, CacheBuilder};
use poise::builtins::{register_globally, register_in_guild};
use poise::serenity_prelude::{
    ChannelId, ClientBuilder, Colour, GatewayIntents, GuildId, ReactionType, RoleId, UserId,
};
use poise::{EditTracker, Framework, PrefixFrameworkOptions};
use serde::Deserialize;
use shuttle_runtime::{CustomError, SecretStore};
use shuttle_serenity::ShuttleSerenity;
use songbird::serenity::SerenityInit;
use sqlx::{query, PgPool};
use tracing::{error, info};

#[cfg(feature = "activity")]
mod check_access;
mod check_birthday;
mod check_reminder;
mod commands;
mod constants;
mod easy_embed;
mod handler;

#[derive(Debug, Deserialize)]
struct AutoReply {
    keywords: Vec<String>,
    user: UserId,
    title: String,
    description: String,
    #[serde(default)]
    ping: bool,
    #[serde(default)]
    /// colour as an integer
    colour: Colour,
    chance: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct LinkFix {
    host: Option<String>,
    tracking: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Access {
    log_channel: ChannelId,
    active_days: u32,
    descending_roles: Vec<RoleId>,
}
#[derive(Deserialize)]
struct Config {
    #[cfg(feature = "activity")]
    #[serde(default)]
    access_per_guild: HashMap<GuildId, Access>,
    #[serde(default)]
    event_channel_per_guild: HashMap<GuildId, ChannelId>,
    #[serde(default)]
    link_fixes: HashMap<String, LinkFix>,
    #[serde(default)]
    auto_reactions: HashMap<String, ReactionType>,
    #[serde(default)]
    auto_replies: Vec<AutoReply>,
    #[serde(default)]
    entry_sounds: HashMap<UserId, String>,
}

#[cfg(feature = "activity")]
#[derive(Debug, Clone)]
struct CacheEntry {}

/// User data, which is stored and accessible in all command invocations
#[derive(Debug)]
pub(crate) struct Data {
    cat_api_token: String,
    dog_api_token: String,
    mp_api_token: String,
    database: PgPool,
    #[cfg(feature = "activity")]
    activity_per_guild: HashMap<GuildId, Cache<UserId, CacheEntry>>,
    event_channel_per_guild: HashMap<GuildId, ChannelId>,
    link_fixes: HashMap<String, LinkFix>,
    auto_reactions: Vec<(String, ReactionType)>,
    auto_replies: Vec<AutoReply>,
    entry_sounds: HashMap<UserId, String>,
    reaction_msgs: RwLock<HashSet<u64>>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[shuttle_runtime::main]
async fn poise(
    #[shuttle_runtime::Secrets] secret_store: SecretStore,
    #[shuttle_shared_db::Postgres(local_uri = "postgres://test:pass@localhost:5432/postgres")]
    pool: PgPool,
) -> ShuttleSerenity {
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let config_data = read_to_string("assets/config.hjson").unwrap_or_default();
    let config: Config = deser_hjson::from_str(&config_data)
        .context("Bad config")
        .map_err(|e| {
            // TODO we need the debug output, fix/remove shuttle
            error!("{:#}", e);
            e
        })?;
    #[cfg(feature = "activity")]
    let activity = config
        .access_per_guild
        .keys()
        .copied()
        .map(|guild| (guild, CacheBuilder::new(500).time_to_live(ONE_DAY).build()))
        .collect();

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Migrations failed")?;

    let framework = Framework::builder()
        .options(poise::FrameworkOptions {
            commands: get_all_commands(),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(handler::event_handler(ctx, event, _framework, data))
            },
            prefix_options: PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                edit_tracker: Some(Arc::from(EditTracker::for_timespan(Duration::from_secs(
                    60,
                )))),
                execute_untracked_edits: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(move |ctx, ready, _framework| {
            Box::pin(async move {
                info!("{} is connected!", ready.user.name);
                register_globally(ctx, &[modules(), register_commands()]).await?;
                for guild in &ready.guilds {
                    let modules = get_active_modules(&pool, guild.id).await?;
                    register_in_guild(ctx, &get_active_commands(modules), guild.id).await?;
                    info!("Loaded modules for guild {}", guild.id);
                }
                load_bot_emojis(ctx, ready.guilds.iter().map(|g| g.id).collect_vec()).await?;
                let reaction_msgs: Vec<_> = query!("SELECT message_id FROM reaction_roles")
                    .fetch_all(&pool)
                    .await?;
                info!("Loaded reaction messages");
                check_reminders(ctx.clone(), pool.clone());
                check_birthdays(
                    ctx.clone(),
                    pool.clone(),
                    config.event_channel_per_guild.clone(),
                );
                #[cfg(feature = "activity")]
                check_access(ctx.clone(), pool.clone(), config.access_per_guild);
                Ok(Data {
                    cat_api_token: secret_store.get("CAT_API_TOKEN").unwrap_or("".to_string()),
                    dog_api_token: secret_store.get("DOG_API_TOKEN").unwrap_or("".to_string()),
                    mp_api_token: secret_store
                        .get("MENSAPLAN_API_TOKEN")
                        .unwrap_or("".to_string()),
                    database: pool,
                    #[cfg(feature = "activity")]
                    activity_per_guild: activity,
                    event_channel_per_guild: config.event_channel_per_guild,
                    link_fixes: config.link_fixes,
                    auto_reactions: config.auto_reactions.into_iter().collect_vec(),
                    auto_replies: config.auto_replies,
                    entry_sounds: config.entry_sounds,
                    reaction_msgs: RwLock::new(
                        reaction_msgs.iter().map(|f| f.message_id as u64).collect(),
                    ),
                })
            })
        })
        .build();

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let client = ClientBuilder::new(discord_token, intents)
        .framework(framework)
        .register_songbird()
        .await
        .map_err(CustomError::new)?;

    Ok(client.into())
}
