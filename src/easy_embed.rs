use poise::CreateReply;
use poise::serenity_prelude::{CreateEmbed, CreateInteractionResponseMessage, CreateMessage};

pub(crate) trait EasyEmbed {
    fn easy_embed(self, e: CreateEmbed) -> Self;
    fn content(self, content: String) -> Self;
}

impl EasyEmbed for CreateReply {
    fn easy_embed(self, e: CreateEmbed) -> Self {
        self.embed(e)
    }

    fn content(self, content: String) -> Self {
        self.content(content)
    }
}

impl EasyEmbed for CreateMessage {
    fn easy_embed(self, e: CreateEmbed) -> Self {
        self.add_embed(e)
    }

    fn content(self, content: String) -> Self {
        self.content(content)
    }
}

impl EasyEmbed for CreateInteractionResponseMessage {
    fn easy_embed(self, e: CreateEmbed) -> Self {
        self.add_embed(e)
    }

    fn content(self, content: String) -> Self {
        self.content(content)
    }
}
