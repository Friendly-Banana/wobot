use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fs::read_to_string;
use std::sync::RwLock;

use anyhow::Context as _;
use image::DynamicImage;
use poise::serenity_prelude::{ChannelId, Colour, GatewayIntents, GuildId, ReactionType, UserId};
use poise::{Framework, PrefixFrameworkOptions};
use regex::Regex;
use serde::Deserialize;
use shuttle_poise::ShuttlePoise;
use shuttle_runtime::CustomError;
use shuttle_secrets::SecretStore;
use sqlx::{query, PgPool};
use tracing::{debug, info};

use crate::commands::*;

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
    nsfw: bool,
    #[serde(default)]
    colour: Colour,
}

#[derive(Deserialize)]
struct Config {
    event_channel_per_guild: HashMap<GuildId, ChannelId>,
    excluded_channels: HashSet<ChannelId>,
    auto_reactions: HashMap<String, ReactionType>,
    auto_replies: Vec<AutoReply>,
}

/// User data, which is stored and accessible in all command invocations
#[derive(Debug)]
pub(crate) struct Data {
    database: PgPool,
    bot_id: RwLock<UserId>,
    announcement_channel: HashMap<GuildId, ChannelId>,
    excluded_channels: RwLock<HashSet<ChannelId>>,
    auto_reactions: HashMap<ReactionType, Regex>,
    auto_replies: Vec<AutoReply>,
    reaction_msgs: RwLock<HashSet<u64>>,
    mensa_state: RwLock<HashMap<UserId, MensaPosition>>,
    avatar_cache: RwLock<HashMap<UserId, DynamicImage>>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[shuttle_runtime::main]
async fn poise(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
    #[shuttle_shared_db::Postgres(local_uri = "postgres://test:pass@localhost:5432/postgres")]
    pool: PgPool,
) -> ShuttlePoise<Data, Error> {
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let config_data = read_to_string("assets/config.hjson").context("Couldn't load config file")?;
    let config: Config = deser_hjson::from_str(&config_data).context("Bad config")?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Migrations failed")?;

    let commands = vec![
        meme(),
        obama(),
        cutie_pie(),
        keyword_statistics(),
        boop(),
        uwu(),
        uwu_text(),
        ping(),
        latency(),
        servers(),
        say(),
        react(),
        activity(),
        clear(),
        emoji(),
        features(),
        mensa(),
        canteen(),
        cruisine(),
        event(),
        export_events(),
        reaction_role(),
        register_commands(),
        exclude(),
    ];
    let framework = Framework::builder()
        .options(poise::FrameworkOptions {
            commands,
            event_handler: |ctx, event, _framework, data| {
                Box::pin(handler::event_handler(ctx, event, _framework, data))
            },
            prefix_options: PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .token(discord_token)
        .intents(
            GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::MESSAGE_CONTENT
                | GatewayIntents::GUILD_EMOJIS_AND_STICKERS
                | GatewayIntents::GUILD_MESSAGE_REACTIONS
                | GatewayIntents::GUILD_SCHEDULED_EVENTS,
        )
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                info!("{} is connected!", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                let reaction_msgs: Vec<_> = query!("SELECT message_id FROM reaction_roles")
                    .fetch_all(&pool)
                    .await?;
                debug!("Loaded reaction messages");
                Ok(Data {
                    database: pool,
                    bot_id: RwLock::new(ready.user.id),
                    excluded_channels: RwLock::new(config.excluded_channels),
                    announcement_channel: config.event_channel_per_guild,
                    auto_reactions: config
                        .auto_reactions
                        .into_iter()
                        .map(|(k, v)| (v, Regex::new(&format!("\\b{}\\b", k)).unwrap()))
                        .collect(),
                    auto_replies: config.auto_replies,
                    reaction_msgs: RwLock::new(
                        reaction_msgs.iter().map(|f| f.message_id as u64).collect(),
                    ),
                    mensa_state: RwLock::new(HashMap::new()),
                    avatar_cache: RwLock::new(HashMap::new()),
                })
            })
        })
        .build()
        .await
        .map_err(CustomError::new)?;

    Ok(framework.into())
}
