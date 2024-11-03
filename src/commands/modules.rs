use itertools::Itertools;
use poise::builtins::register_in_guild;
use poise::{ChoiceParameter, Command};
use sqlx::{query, PgPool};

use crate::commands::*;
use crate::{Context, Data, Error};

#[derive(Clone, poise::ChoiceParameter)]
pub(crate) enum Module {
    #[name_localized("de", "Mensa")]
    Canteen,
    #[name_localized("de", "Bilder")]
    Images,
    #[name_localized("de", "Eigentümer")]
    Owner,
    #[name_localized("de", "Nützliches")]
    Utility,
    Events,
    #[name_localized("de", "Verschiedenes")]
    Misc,
}

impl From<i32> for Module {
    fn from(item: i32) -> Self {
        match item {
            0 => Module::Canteen,
            1 => Module::Images,
            2 => Module::Owner,
            3 => Module::Utility,
            4 => Module::Events,
            5 => Module::Misc,
            _ => panic!("Invalid module value"),
        }
    }
}

#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    subcommands("list", "enable", "disable")
)]
pub(crate) async fn modules(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let modules = get_active_modules(&ctx.data().database, ctx.guild_id().unwrap()).await?;
    ctx.reply(format!(
        "Aktive Modules: {}",
        modules.iter().map(Module::name).join(", ")
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn enable(ctx: Context<'_>, module: Module) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().unwrap();

    query!(
        "INSERT INTO modules (guild_id, module_id) VALUES ($1, $2)",
        guild.get() as i64,
        module.clone() as i32
    )
    .execute(&ctx.data().database)
    .await?;
    let modules = get_active_modules(&ctx.data().database, guild).await?;
    register_in_guild(ctx, &get_active_commands(modules), guild).await?;

    ctx.reply(format!("Module {} enabled", module.name()))
        .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, owners_only)]
pub(crate) async fn disable(ctx: Context<'_>, module: Module) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;
    let guild = ctx.guild_id().unwrap();

    query!(
        "DELETE FROM modules WHERE guild_id = $1 AND module_id = $2",
        guild.get() as i64,
        module.clone() as i32
    )
    .execute(&ctx.data().database)
    .await?;
    let modules = get_active_modules(&ctx.data().database, guild).await?;
    register_in_guild(ctx, &get_active_commands(modules), guild).await?;

    ctx.reply(format!("Module {} disabled", module.name()))
        .await?;
    Ok(())
}

pub(crate) async fn get_active_modules(
    database: &PgPool,
    guild: GuildId,
) -> Result<Vec<Module>, Error> {
    let modules = query!(
        "SELECT * FROM modules WHERE guild_id = $1",
        guild.get() as i64
    )
    .fetch_all(database)
    .await?;

    Ok(modules.iter().map(|m| Module::from(m.module_id)).collect())
}

pub(crate) fn get_all_commands() -> Vec<Command<Data, Error>> {
    let mut cmds = get_active_commands(vec![
        Module::Canteen,
        Module::Images,
        Module::Owner,
        Module::Utility,
        Module::Events,
        Module::Misc,
    ]);
    cmds.push(modules());
    cmds.push(register_commands());
    cmds
}

pub(crate) fn get_active_commands(modules: Vec<Module>) -> Vec<Command<Data, Error>> {
    let mut commands = Vec::new();
    for module in modules {
        commands.extend(match module {
            Module::Canteen => vec![mensa(), mp()],
            Module::Images => vec![floof(), capy(), cutie_pie(), obama()],
            Module::Owner => vec![activity(), inactive(), latency(), servers()],
            Module::Utility => vec![
                clear(),
                emoji(),
                features(),
                embed(),
                reminder(),
                react(),
                say(),
            ],
            Module::Events => vec![event(), export_events(), reaction_role()],
            Module::Misc => vec![boop(), keyword_statistics(), uwu(), uwu_text(), ping()],
        });
    }
    commands
}
