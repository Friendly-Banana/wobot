# WoBot

## Features

WoBot comes with a ton of features, including:

##### Improved Events:

* create a Discord Event, a thread and allow RSVP via reactions with a **single** command
* export all events to your calendar (works on mobile)

##### Improved Emojis:

* `add`, `rename` and `remove`
* `upload` to convert images into emojis
* `copy` emojis from other servers to your own

##### Easy Reaction Roles

simply react with the emojis you want

##### Memes:

* Obama: when someone congratulates themselves
* Cutie Pie: tell your friends how cute they are

##### ToDo list

store anything you like

#### Reminder

schedule whatever you like for later

#### Mensa plan

* know what's up for lunch
* show the next available plan
* automatically skips weekends
* find your friends

## Images

Not yet convinced? Have some images:

![meme with four labeled sections: cherry pie, apple pie, pumpkin pie and the WoBot logo, a cartoon robot, tagged as 'cutie pie'.](images/cutie_pie.png)
![Obama giving himself a medal, both Obamas are labelled 'WoBot'](images/obama.png)
![list of reaction roles each with corresponding message and emoji](images/reaction_roles.png)
![Discord embed asking whether to create an emoji](images/emoji.png)
![list of dishes in the Mensa Garching](images/mensa.png)
![list of features as embeds with selector for feature state](images/todo.png)

## Contributing

If you have a great idea or suggestion, feel free to open [an issue](https://github.com/Friendly-Banana/wobot/issues).
If you want a feature right now and can code, open [a pull request](https://github.com/Friendly-Banana/wobot/pulls).
Please make sure to run `cargo fmt` before committing.

### Running the Bot

1. [Install Rust](https://www.rust-lang.org/tools/install)
2. [Install Shuttle](https://docs.shuttle.dev/getting-started/installation)
3. [Install Docker](https://docs.docker.com/install/) (or use
   your [local PostgresQL](https://docs.shuttle.dev/docs/local-run#local-runs-with-databases))
4. Create a Discord Bot on the [Discord Developer Portal](https://discord.com/developers/applications)
5. Copy the bot token and put it in a `Secrets.toml` file in the root directory:
    ```toml
    DISCORD_TOKEN = "<your token>"
    ```
6. Invite the bot to your server with all permissions you need (`ADMINISTRATOR` is the easiest).
7. Run the bot with `shuttle run`

## Technical Overview

WoBot is a Discord Bot written in [Rust](https://www.rust-lang.org/)
with [the  Poise framework](https://github.com/serenity-rs/poise/).
It's hosted on [Shuttle](https://www.shuttle.rs/) and uses a PostgresQL database.

The mensa plan uses the [Eat API](https://tum-dev.github.io/eat-api), the mensa coordinates link
to [Google Maps](https://www.google.com/maps).

The [Mensaplan API](https://github.com/Friendly-Banana/mensaplan) is written by myself in Elixir.

### Configuration

`config.hjson` uses a human-friendly JSON version, [HJson](https://hjson.github.io/).

You can set up automatic reactions and replies based on keywords. All of them are case-insensitive.
Auto-reactions match only on word boundaries, ignoring punctuation around them.
For example, `wobot` would match `WoBot!` but not `wo bot`.
WoBot can react with both Unicode and custom Discord emojis, even animated ones.

#### Example Config

```json
// channel for event threads
event_channel_per_guild: {
  // guild_id: channel_id
  1: 1
},
auto_reactions: {
  robot: {
    name: "🤖"
  }
  vibing: {
    animated: true
    name: vibing
    // emoji id
    id: 1
  }
},
auto_replies: [
  {
    keywords: [
      "wobot info"
      "wobot help"
    ]
    // discord user id
    user: 1
    title: About WoBot
    description: "Hi, I'm **WoBot**, your friendly neighborhood bot. Please send any questions or feedback to my author, {user}. This message was sent {count} times. Have a nice day!"
    colour: 15844367
  }
]
```
