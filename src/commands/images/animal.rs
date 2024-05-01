use std::convert::identity;
use std::ops::Range;

use poise::serenity_prelude::CreateEmbed;
use poise::{command, CreateReply};
use rand::{thread_rng, Rng};
use serde::Deserialize;

use crate::constants::HTTP_CLIENT;
use crate::{Context, Error};

/// random animal, possible: Fox, Cat, Dog
#[command(slash_command, prefix_command)]
pub async fn animal(ctx: Context<'_>, cat_fact: Option<bool>) -> Result<(), Error> {
    ctx.defer().await?;

    let image = get_random_image(&ctx.data().cat_api_token, &ctx.data().dog_api_token).await?;
    let mut embed = CreateEmbed::default().title("Animal :)").image(image);

    if cat_fact.is_some_and(identity) {
        let fact = random_cat_fact().await?;
        embed = embed.title("Cat Fact").description(fact);
    }

    ctx.send(CreateReply::default().embed(embed)).await?;
    Ok(())
}

const API_RANGE: Range<i32> = 0..7;
async fn get_random_image(cat_api_token: &str, dog_api_token: &str) -> Result<String, Error> {
    #[cfg(test)]
    let api_choice = tests::API_CHOICE.load(std::sync::atomic::Ordering::SeqCst);
    #[cfg(not(test))]
    let api_choice = thread_rng().gen_range(API_RANGE);

    match api_choice {
        0 => do_json::<TopLevelImage>("https://randomfox.ca/floof/").await,
        1 => {
            do_json::<TopLevelImage>(
                "https://shibe.online/api/shibes?count=1&urls=true&httpsUrls=true",
            )
            .await
        }
        2 => do_json::<TopLevelURL>("https://random.dog/woof.json").await,
        3 => do_json::<DogCEO>("https://dog.ceo/api/breeds/image/random").await,
        4 => do_json::<CatAAS>("https://cataas.com/cat?json=true").await,
        5 => {
            let url = format!("https://api.thecatapi.com/v1/images/search?api_key={cat_api_token}");
            do_json::<TheCatAPI>(&url).await
        }
        6 => {
            let url = format!("https://api.thedogapi.com/v1/images/search?api_key={dog_api_token}");
            do_json::<TheCatAPI>(&url).await
        }
        _ => unreachable!(),
    }
}

async fn do_json<T: AnimalImage + for<'de> Deserialize<'de>>(url: &str) -> Result<String, Error> {
    let response = HTTP_CLIENT.get(url).send().await?;
    Ok(response.json::<T>().await?.extract_url())
}

trait AnimalImage {
    fn extract_url(self) -> String;
}

#[derive(Deserialize)]
struct TopLevelURL {
    url: String,
}

impl AnimalImage for TopLevelURL {
    fn extract_url(self) -> String {
        self.url
    }
}

#[derive(Deserialize)]
struct TopLevelImage {
    image: String,
}

impl AnimalImage for TopLevelImage {
    fn extract_url(self) -> String {
        self.image
    }
}

#[derive(Deserialize)]
struct DogCEO {
    message: String,
}

impl AnimalImage for DogCEO {
    fn extract_url(self) -> String {
        self.message
    }
}
#[derive(Deserialize)]
struct CatAAS {
    _id: String,
}

impl AnimalImage for CatAAS {
    fn extract_url(self) -> String {
        format!("https://cataas.com/cat/{}", self._id)
    }
}

#[derive(Deserialize)]
struct TheCatAPI {
    data: TopLevelURL,
}

impl AnimalImage for TheCatAPI {
    fn extract_url(self) -> String {
        self.data.extract_url()
    }
}

#[derive(Deserialize)]
struct MeowFacts {
    data: Vec<String>,
}

async fn random_cat_fact() -> Result<String, Error> {
    const API: &'static str = "https://meowfacts.herokuapp.com/";
    let response = HTTP_CLIENT.get(API).send().await?;
    Ok(response.json::<MeowFacts>().await?.data.remove(0))
}

#[allow(unused)]
mod tests {
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering::SeqCst;

    use super::{get_random_image, random_cat_fact, API_RANGE};

    pub(crate) static API_CHOICE: AtomicI32 = AtomicI32::new(0);

    #[tokio::test]
    async fn test_random_image() {
        for i in API_RANGE {
            API_CHOICE.store(i, SeqCst);
            get_random_image("", "")
                .await
                .expect(&format!("Failed to get random image from API {}", i));
        }
    }

    #[tokio::test]
    async fn test_random_cat_fact() {
        random_cat_fact().await.expect("Failed to get cat fact");
    }
}
