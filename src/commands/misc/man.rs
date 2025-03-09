use crate::commands::utils::remove_components_but_keep_embeds;
use crate::{Context, Error};
use poise::serenity_prelude::{
    ComponentInteractionCollector, CreateActionRow, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseMessage,
};
use poise::CreateReply;
use std::process::Command;
use std::time::Duration;
use tracing::{debug, warn};

const MAN_VIEW_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MSG_MAX_LEN: usize = 2000 - 25; // -25 to account for the title

fn get_pages(input: &str) -> Vec<String> {
    let lines: Vec<&str> = input.lines().collect();

    if lines.len() <= 2 {
        return vec!["No man page available!".into()];
    }

    let stripped_lines = &lines[1..lines.len() - 1];

    let mut res: Vec<String> = vec![];

    let mut lines: usize = 0;

    let mut collector: String = "".into();

    for line in stripped_lines {
        // +1 because of newline
        if lines + line.len() + 1 > MSG_MAX_LEN {
            lines = 0;

            res.push(collector);
            collector = "".into();
        }

        lines += line.len() + 1;
        collector.push_str(format!("{}\n", line).as_str());
    }

    res.push(collector);

    res
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn man(ctx: Context<'_>, text: String) -> Result<(), Error> {
    ctx.defer().await?;
    debug!("man {}", text);
    let com = Command::new("man").args(text.split(' ')).output()?;
    let com_str = String::from_utf8_lossy(&com.stdout);
    let pages = get_pages(&com_str);

    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx.id());
    let next_button_id = format!("{}next", ctx.id());

    let mut idx: usize = 0;
    let page_count = pages.len();

    let reply = {
        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&prev_button_id).emoji('◀'),
            CreateButton::new(&next_button_id).emoji('▶'),
        ])];

        let m = CreateReply::default()
            .components(components)
            .content(format!(
                "**Page {}/{}**```{}```",
                idx + 1,
                page_count,
                pages[idx]
            ));

        m
    };

    let reply_handle = ctx.send(reply).await?;

    while let Some(press) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(MAN_VIEW_TIMEOUT)
        .await
    {
        if press.data.custom_id == next_button_id {
            idx = idx + 1;
            if idx >= page_count {
                idx = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            if idx > 0 {
                idx = idx - 1;
            } else {
                idx = page_count - 1;
            }
        } else {
            // This is an unrelated button interaction
            warn!(
                "unrelated button interaction with same ctx id: {:?}",
                press.data
            );
            continue;
        }

        // Update the message with the new page contents

        let message = CreateInteractionResponseMessage::default().content(format!(
            "**Page {}/{}**```{}```",
            idx + 1,
            page_count,
            pages[idx]
        ));

        press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(message),
            )
            .await?;
    }
    remove_components_but_keep_embeds(ctx, CreateReply::default(), reply_handle).await
}
