use poise::serenity_prelude::{CreateEmbed, CreateInteractionResponseData, CreateMessage, User};
use poise::CreateReply;

pub(crate) trait EasyEmbed {
    fn easy_embed(&mut self, e: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed) -> &mut Self;
    fn content(&mut self, content: String) -> &mut Self;
}

impl EasyEmbed for CreateReply<'_> {
    fn easy_embed(&mut self, e: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed) -> &mut Self {
        self.embed(e)
    }

    fn content(&mut self, content: String) -> &mut Self {
        self.content(content)
    }
}

impl EasyEmbed for CreateMessage<'_> {
    fn easy_embed(&mut self, e: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed) -> &mut Self {
        self.add_embed(e)
    }

    fn content(&mut self, content: String) -> &mut Self {
        self.content(content)
    }
}

impl EasyEmbed for CreateInteractionResponseData<'_> {
    fn easy_embed(&mut self, e: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed) -> &mut Self {
        self.embed(e)
    }

    fn content(&mut self, content: String) -> &mut Self {
        self.content(content)
    }
}

pub(crate) trait EasyEmbedAuthor {
    fn easy_author(&mut self, user: &User) -> &mut Self;
}

impl EasyEmbedAuthor for CreateEmbed {
    fn easy_author(&mut self, user: &User) -> &mut Self {
        self.author(|a| {
            a.name(&user.name)
                .icon_url(user.avatar_url().unwrap_or(user.default_avatar_url()))
        })
    }
}
