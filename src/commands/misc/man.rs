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
const MSG_MAX_LEN: usize = 1950;

fn remove_first_and_last_line(input: String) -> String {
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() <= 2 {
        return String::new();
    }
    lines[1..lines.len() - 1].join("\n")
}

#[poise::command(slash_command, prefix_command)]
pub(crate) async fn man(ctx: Context<'_>, text: String) -> Result<(), Error> {
    debug!("man {}", text);
    let com = Command::new("man").arg(text).output()?;
    let com_str = String::from_utf8_lossy(&com.stdout).parse()?;
    let page = remove_first_and_last_line(com_str);

    let ctx_id = ctx.id();
    let prev_button_id = format!("{}prev", ctx.id());
    let next_button_id = format!("{}next", ctx.id());

    let mut idx: usize = 0;
    let page_count = calculate_count(&page);

    let reply = {
        let components = vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&prev_button_id).emoji('◀'),
            CreateButton::new(&next_button_id).emoji('▶'),
        ])];

        let m = CreateReply::default().components(components);

        m.content(format!(
            "**Page {}/{}**\n```{}```",
            idx + 1,
            page_count + 1,
            get_segment(&page, idx)
        ))
    };

    ctx.send(reply).await?;

    while let Some(press) = ComponentInteractionCollector::new(ctx)
        .channel_id(ctx.channel_id())
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(MAN_VIEW_TIMEOUT)
        .await
    {
        if press.data.custom_id == next_button_id {
            idx = idx + 1;
            if idx > page_count {
                idx = 0;
            }
        } else if press.data.custom_id == prev_button_id {
            if idx > 0 {
                idx = idx - 1;
            } else {
                idx = page_count;
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
            "**page {}/{}**\n```{}```",
            idx + 1,
            page_count + 1,
            get_segment(&page, idx)
        ));

        press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(message),
            )
            .await?;
    }
    Ok(())
}

fn calculate_count(msg: &str) -> usize {
    let mut count: usize = 0;
    let mut lines: usize = 0;

    for line in msg.lines() {
        if lines + line.len() > MSG_MAX_LEN {
            count = count + 1;
            lines = line.len();
        } else {
            lines += line.len();
        }
    }
    count
}

fn get_segment(msg: &str, mut idx: usize) -> String {
    let mut ret: String = "".into();

    let mut lines: usize = 0;

    for line in msg.lines() {
        if lines + line.len() > MSG_MAX_LEN {
            if idx > 0 {
                idx = idx - 1;
                lines = 0;
            } else {
                return ret;
            }
        }

        lines += line.len();

        if idx == 0 {
            ret.push_str(format!("{}\n", line).as_str());
        }
    }

    ret
}
