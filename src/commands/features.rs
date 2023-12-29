use std::time::Duration;

use poise::serenity_prelude::{CollectComponentInteraction, InteractionResponseType, Timestamp};
use sqlx::PgPool;
use sqlx::{query, query_as};
use tracing::{info, warn};

use crate::commands::feature_state::FeatureState;
use crate::commands::feature_state::FeatureState::*;
use crate::commands::remove_components_but_keep_embeds;
use crate::easy_embed::EasyEmbed;
use crate::{Context, Error};

#[allow(dead_code)]
pub(crate) struct Feature {
    id: i64,
    name: String,
    state: FeatureState,
    timestamp: Timestamp,
}

const FEATURES_VIEW_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const PER_PAGE: u64 = 5;

#[poise::command(slash_command, prefix_command, subcommands("list", "add", "update"))]
pub(crate) async fn features(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, prefix_command, ephemeral)]
pub(crate) async fn add(ctx: Context<'_>, name: String) -> Result<(), Error> {
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

#[poise::command(slash_command, prefix_command, owners_only, ephemeral)]
pub(crate) async fn update(
    ctx: Context<'_>,
    name: String,
    state: FeatureState,
) -> Result<(), Error> {
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
pub(crate) async fn list(ctx: Context<'_>) -> Result<(), Error> {
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

    let reply = ctx
        .send(|m| {
            make_feature_embeds(features, state, offset, feature_count, m).components(|b| {
                b.create_action_row(|b| {
                    b.create_button(|b| b.custom_id(&prev_button_id).emoji('◀'))
                        .create_button(|b| b.custom_id(&refresh_button_id).emoji('🔄'))
                        .create_button(|b| b.custom_id(&next_button_id).emoji('▶'))
                })
                .create_action_row(|b| {
                    b.create_select_menu(|s| {
                        s.custom_id(&filter_id)
                            .placeholder("Select a state")
                            .min_values(1)
                            .max_values(1)
                            .options(|menu| {
                                for state in [All, ToDo, Implemented, Rejected, Postponed] {
                                    menu.create_option(|o| FeatureState::menu(state, o));
                                }
                                menu
                            })
                    })
                })
            })
        })
        .await?;

    while let Some(press) = CollectComponentInteraction::new(ctx)
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
            let new_state = press.data.values[0]
                .parse::<i64>()
                .map_or(All, FeatureState::from);
            if new_state != state {
                state = new_state;
                offset = 0;
                feature_count = get_feature_count(db, state).await?;
            } else {
                press
                    .create_interaction_response(ctx.http(), |r| {
                        r.interaction_response_data(|d| {
                            d.content("Already showing that state").ephemeral(true)
                        })
                    })
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
        press
            .create_interaction_response(ctx, |response| {
                response
                    .kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|m| {
                        make_feature_embeds(features, state, offset, feature_count, m)
                    })
            })
            .await?;
    }
    remove_components_but_keep_embeds(ctx, reply).await
}

fn make_feature_embeds<T: EasyEmbed>(
    features: Vec<Feature>,
    state: FeatureState,
    offset: u64,
    pages: u64,
    reply: &mut T,
) -> &mut T {
    let all = state == All;
    if features.is_empty() {
        reply.easy_embed(|e| {
            e.title("No features found")
                .description("No features found")
        });
    }
    for feature in features {
        reply.easy_embed(|e| {
            if all {
                e.description(feature.state);
            }
            e.title(feature.name)
                .timestamp(feature.timestamp)
                .colour(feature.state)
        });
    }
    reply.content(if all {
        format!("**All** Features {}/{}", offset, pages)
    } else {
        format!("**{}** {}/{}", state, offset, pages)
    })
}

async fn get_feature_count(db: &PgPool, state: FeatureState) -> Result<u64, Error> {
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
) -> Result<Vec<Feature>, Error> {
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
