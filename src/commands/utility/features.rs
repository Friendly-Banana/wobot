use std::time::Duration;

use poise::CreateReply;
use poise::serenity_prelude::{
    ComponentInteractionCollector, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu,
    CreateSelectMenuKind, Timestamp,
};
use sqlx::PgPool;
use sqlx::{query, query_as};
use tracing::{info, warn};

use crate::Context;
use crate::commands::utility::feature_state::FeatureState;
use crate::commands::utility::feature_state::FeatureState::{
    All, Implemented, Postponed, Rejected, ToDo,
};
use crate::commands::utils::remove_components_but_keep_embeds;
use crate::easy_embed::EasyEmbed;
use anyhow::bail;

#[allow(dead_code)]
pub(crate) struct Feature {
    id: i64,
    name: String,
    state: FeatureState,
    timestamp: Timestamp,
}

const FEATURES_VIEW_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const PER_PAGE: u64 = 5;

#[poise::command(
    slash_command,
    prefix_command,
    subcommands("list", "add", "update", "delete")
)]
pub(crate) async fn features(_ctx: Context<'_>) -> anyhow::Result<()> {
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn add(ctx: Context<'_>, name: String) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    info!("{} added feature {}", ctx.author().name, name);
    query!(
        "INSERT INTO features (name, state) VALUES ($1, $2)",
        name,
        ToDo as i64,
    )
    .execute(&ctx.data().database)
    .await?;
    ctx.say(format!("Added feature {}", name)).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn delete(ctx: Context<'_>, name: String) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    query!("DELETE FROM features WHERE features.name = $1", name)
        .execute(&ctx.data().database)
        .await?;
    ctx.say(format!("Deleted feature {}", name)).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only, ephemeral)]
pub(crate) async fn update(
    ctx: Context<'_>,
    name: String,
    state: FeatureState,
) -> anyhow::Result<()> {
    ctx.defer_ephemeral().await?;
    let affected = query!(
        "UPDATE features SET state = $1 WHERE name = $2",
        state as i64,
        name,
    )
    .execute(&ctx.data().database)
    .await?;

    if affected.rows_affected() == 0 {
        ctx.say(format!("No feature named {}", name)).await?;
    } else {
        ctx.say(format!("Updated feature {}", name)).await?;
    }
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn list(ctx: Context<'_>) -> anyhow::Result<()> {
    ctx.defer().await?;
    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx.id());
    let next_button_id = format!("{}next", ctx.id());
    let refresh_button_id = format!("{}refresh", ctx.id());
    let filter_id = format!("{}filter", ctx.id());

    let mut state = All;
    let mut offset: u64 = 0;
    let mut feature_count = get_feature_count(&ctx.data().database, state).await?;
    let mut features: Vec<Feature> = get_features(&ctx.data().database, state, offset).await?;

    let reply = {
        let options = [All, ToDo, Implemented, Rejected, Postponed]
            .map(FeatureState::menu)
            .to_vec();
        let components = vec![
            CreateActionRow::Buttons(vec![
                CreateButton::new(&prev_button_id).emoji('◀'),
                CreateButton::new(&refresh_button_id).emoji('🔄'),
                CreateButton::new(&next_button_id).emoji('▶'),
            ]),
            CreateActionRow::SelectMenu(
                CreateSelectMenu::new(&filter_id, CreateSelectMenuKind::String { options })
                    .placeholder("Select a state")
                    .min_values(1)
                    .max_values(1),
            ),
        ];
        let m = CreateReply::default().components(components);
        make_feature_embeds(features, state, offset, feature_count, m)
    };

    let reply_handle = ctx.send(reply).await?;

    while let Some(press) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(FEATURES_VIEW_TIMEOUT)
        .await
    {
        let db = &ctx.data().database;
        if press.data.custom_id == next_button_id {
            offset += PER_PAGE;
            if offset >= feature_count {
                offset = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            offset = offset
                .checked_sub(PER_PAGE)
                .unwrap_or((feature_count / PER_PAGE) * PER_PAGE);
        } else if press.data.custom_id == refresh_button_id {
            feature_count = get_feature_count(db, state).await?;
        } else if press.data.custom_id == filter_id {
            let values = match &press.data.kind {
                ComponentInteractionDataKind::StringSelect { values } => values,
                value => bail!("invalid select menu interaction {:?}", value),
            };
            let new_state = values[0].parse::<i64>().map_or(All, FeatureState::from);
            if new_state != state {
                state = new_state;
                offset = 0;
                feature_count = get_feature_count(db, state).await?;
            } else {
                press
                    .create_response(
                        ctx.http(),
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("Already showing that state")
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                continue;
            }
        } else {
            // This is an unrelated button interaction
            warn!(
                "unrelated button interaction with same ctx id: {:?}",
                press.data
            );
            continue;
        }
        features = get_features(db, state, offset).await?;

        // Update the message with the new page contents
        let message = make_feature_embeds(
            features,
            state,
            offset,
            feature_count,
            CreateInteractionResponseMessage::default(),
        );
        press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(message),
            )
            .await?;
    }
    remove_components_but_keep_embeds(ctx, CreateReply::default(), reply_handle).await
}

fn make_feature_embeds<T: EasyEmbed>(
    features: Vec<Feature>,
    state: FeatureState,
    offset: u64,
    pages: u64,
    mut reply: T,
) -> T {
    let all = state == All;

    if features.is_empty() {
        reply = reply.easy_embed(
            CreateEmbed::new()
                .title("No features found")
                .description("No features found"),
        );
    }
    for feature in features {
        let mut e = CreateEmbed::new();
        if all {
            e = e.description(feature.state.to_string());
        }
        reply = reply.easy_embed(
            e.title(feature.name)
                .timestamp(feature.timestamp)
                .colour(feature.state),
        );
    }
    reply.content(if all {
        format!("**All** Features {}/{}", offset, pages)
    } else {
        format!("**{}** {}/{}", state, offset, pages)
    })
}

async fn get_feature_count(db: &PgPool, state: FeatureState) -> anyhow::Result<u64> {
    Ok(if state == All {
        query!("SELECT COUNT(*) as count FROM features")
            .fetch_one(db)
            .await?
            .count
    } else {
        query!(
            "SELECT COUNT(*) as count FROM features WHERE state = $1",
            state as i64
        )
        .fetch_one(db)
        .await?
        .count
    }
    .unwrap_or_default() as u64)
}

async fn get_features(
    db: &PgPool,
    state: FeatureState,
    offset: u64,
) -> anyhow::Result<Vec<Feature>> {
    Ok(if state == All {
        query_as!(
            Feature,
            "SELECT * FROM features ORDER BY id LIMIT $1 OFFSET $2",
            PER_PAGE as i64,
            offset as i64
        )
        .fetch_all(db)
        .await?
    } else {
        query_as!(
            Feature,
            "SELECT * FROM features WHERE state = $1 ORDER BY id LIMIT $2 OFFSET $3",
            state as i64,
            PER_PAGE as i64,
            offset as i64
        )
        .fetch_all(db)
        .await?
    })
}
