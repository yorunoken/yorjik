use rand::rngs::OsRng;
use std::env;
use std::sync::Arc;

use tokio::time::Duration;

use rand::Rng;

use serenity::all::CreateCommand;
use serenity::builder::GetMessages;
use serenity::model::{application::Interaction, channel::Message, gateway::Ready};
use serenity::prelude::*;
use serenity::{
    all::{Command as CommandInteraction, CreateMessage},
    async_trait,
};

use crate::commands::Command;
use crate::database::Database;
use crate::utils::helpers::{generate_markov_message, get_most_popular_channel};

pub struct Handler {
    pub commands: Vec<Command>,
    pub registered: Vec<CreateCommand>,
    pub database: Arc<Database>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, bot: Ready) {
        println!("Bot has started as {}", bot.user.name);

        match CommandInteraction::set_global_commands(&ctx.http, self.registered.clone()).await {
            Err(e) => {
                eprintln!("There was an error while registering commands: {}", e);
            }
            Ok(_) => {}
        }

        // Random message generator on loop
        let mut rng = OsRng;
        let database_clone = self.database.clone();
        tokio::spawn(async move {
            loop {
                // Fetch vector of guilds the bot is in.
                let guild_ids = ctx.cache.guilds();

                // Loop over the guild ids
                for guild_id in guild_ids {
                    // Get the channel id of the most popular channel
                    let popular_channel_id =
                        get_most_popular_channel(guild_id, database_clone.clone()).await;
                    let all_channels = ctx.http.get_channels(guild_id).await.unwrap();

                    if let Some(channel_id) = all_channels
                        .iter()
                        .find(|channel| channel.id.get() == popular_channel_id)
                        .map(|channel| channel.id)
                    {
                        // Fetch the channel
                        let channel = ctx.http.get_channel(channel_id).await.unwrap();

                        match channel.guild() {
                            Some(channel) => {
                                let messages = channel
                                    .messages(&ctx.http, GetMessages::new().limit(100))
                                    .await
                                    .unwrap();

                                let mut messages_have_bot = false;
                                for message in messages {
                                    if message.author.id.get() == ctx.cache.current_user().id.get()
                                    {
                                        messages_have_bot = true;
                                    }
                                }

                                // Only send a message if builder is not None
                                if let Some(markov_message) = generate_markov_message(
                                    guild_id,
                                    channel.id,
                                    None,
                                    database_clone.clone(),
                                )
                                .await
                                {
                                    if !messages_have_bot {
                                        channel
                                            .send_message(
                                                &ctx.http,
                                                CreateMessage::new().content(markov_message),
                                            )
                                            .await
                                            .unwrap();
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                }

                // Wait a random second from 300 to 900
                let range = rng.gen_range(300..900);
                tokio::time::sleep(Duration::from_secs(range)).await;
            }
        });

        if let Ok(url) = env::var("UPTIME_KUMA_URL") {
            tokio::spawn(async move {
                loop {
                    match reqwest::get(&url).await {
                        Ok(_) => (),
                        Err(e) => eprintln!("Failed to ping Kuma: {}", e),
                    }

                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            });
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // return immediately if author is a bot
        if msg.author.bot {
            return;
        }

        let guild_id = match msg.guild_id {
            Some(s) => s,
            _ => return,
        };

        // write message into database
        if let Err(e) = self
            .database
            .insert_message(
                msg.id.get(),
                msg.author.id.get(),
                msg.channel_id.get(),
                guild_id.get(),
                &msg.content,
            )
            .await
        {
            eprintln!("Failed to insert message into database: {}", e);
        }

        if let Some(referenced_message) = &msg.referenced_message {
            if referenced_message.author.id == ctx.cache.current_user().id
                && !referenced_message.embeds.is_empty()
            {
                return;
            }
        }

        if msg.mentions_me(&ctx.http).await.unwrap_or(false) {
            let typing = ctx.http.start_typing(msg.channel_id);

            let builder = match generate_markov_message(
                guild_id,
                msg.channel_id,
                None,
                self.database.clone(),
            )
            .await
            {
                Some(markov_message) => CreateMessage::new()
                    .content(markov_message)
                    .reference_message(&msg),
                None => CreateMessage::new()
                    .content("Please wait until this channel has over 500 messages.")
                    .reference_message(&msg),
            };

            msg.channel_id
                .send_message(&ctx.http, builder)
                .await
                .unwrap();

            typing.stop();
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(interaction) = interaction {
            for command in &self.commands {
                if interaction.data.name.as_str() == command.name {
                    // Execute command
                    if let Err(reason) =
                        (command.exec)(&ctx, &interaction, self.database.clone()).await
                    {
                        println!(
                            "There was an error while handling command {}: {:#?}",
                            command.name, reason
                        )
                    }
                }
            }
        }
    }
}
