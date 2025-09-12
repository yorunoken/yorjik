use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serenity::all::{
    ButtonStyle, CommandInteraction, CreateButton, CreateCommand, CreateEmbed,
    CreateInteractionResponse, CreateMessage, EditInteractionResponse, Message, User, UserId,
};
use serenity::prelude::*;
use serenity::Error;

use crate::database::Database;
use crate::utils::string_cmp::{gestalt_pattern_matching, levenshtein_similarity};

pub fn register() -> CreateCommand {
    CreateCommand::new("guess").description("Guess who a random message belongs to.")
}

pub async fn execute(
    ctx: &Context,
    command: &CommandInteraction,
    database: Arc<Database>,
) -> Result<(), Error> {
    command.defer(&ctx.http).await?;

    let game_stop_seconds = 180;
    let embed = CreateEmbed::new()
        .title("Message Guesser")
        .description(format!(
            "**How to play:**\n\
            • Bot picks a random message from this server\n\
            • Guess who wrote it using their nickname, username, or user ID\n\
            • Game automatically ends after {} minutes of inactivity\n\n\
            Ready to test your memory?",
            game_stop_seconds / 60
        ))
        .color(0x5865F2);

    let start_button = CreateButton::new("start")
        .style(ButtonStyle::Success)
        .label("Start");

    let cancel_buton = CreateButton::new("cancel")
        .style(ButtonStyle::Danger)
        .label("Cancel");

    let message = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embed)
                .button(start_button.clone())
                .button(cancel_buton.clone()),
        )
        .await?;

    let interaction = match message
        .await_component_interaction(&ctx.shard)
        .timeout(Duration::from_secs(60))
        .await
    {
        Some(x) => x,
        None => {
            let embed = CreateEmbed::new()
                .title("Message Guesser")
                .description("**Game Cancelled**\n\nNo response received within 60 seconds.")
                .color(0xED4245);

            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .embed(embed)
                        .button(start_button.clone().disabled(true))
                        .button(cancel_buton.clone().disabled(true)),
                )
                .await?;

            return Ok(());
        }
    };

    interaction
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await?;

    match interaction.data.custom_id.as_str() {
        "start" => {
            start_game(ctx, command, database).await?;
        }
        "cancel" => {
            let embed = CreateEmbed::new()
                .title("Message Guesser")
                .description("**Game Cancelled**\n\nThe game has been cancelled by user request.")
                .color(0xED4245);

            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .embed(embed)
                        .button(start_button.clone().disabled(true))
                        .button(cancel_buton.clone().disabled(true)),
                )
                .await?;
        }
        _ => {}
    };

    Ok(())
}

async fn start_game(
    ctx: &Context,
    command: &CommandInteraction,
    database: Arc<Database>,
) -> Result<(), Error> {
    let embed = CreateEmbed::new()
        .title("Message Guesser")
        .description("**Game Started!**\n\nPreparing your first message...")
        .color(0x57F287);

    let start_button = CreateButton::new("start")
        .style(ButtonStyle::Success)
        .label("Start");

    let cancel_buton = CreateButton::new("cancel")
        .style(ButtonStyle::Danger)
        .label("Cancel");

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .embed(embed)
                .button(start_button.clone().disabled(true))
                .button(cancel_buton.clone().disabled(true)),
        )
        .await?;

    let mut game = Game::new(ctx, command, database);
    game.start_game().await?;

    Ok(())
}

struct Game<'a> {
    pub ctx: &'a Context,
    pub command: &'a CommandInteraction,
    pub database: Arc<Database>,
    pub game_ended: bool,
}

impl<'a> Game<'a> {
    pub fn new(ctx: &'a Context, command: &'a CommandInteraction, database: Arc<Database>) -> Self {
        Self {
            ctx,
            command,
            database,
            game_ended: false,
        }
    }

    pub async fn start_game(&mut self) -> Result<(), Error> {
        loop {
            if self.game_ended {
                break;
            }

            self.new_sentence().await?;
        }

        Ok(())
    }

    pub async fn new_sentence(&mut self) -> Result<(), Error> {
        let min_letters_amount = 30; // Minimum amount of characters in the content

        let (random_message, random_author) = match self
            .get_random_message(&self.command.guild_id.unwrap().get(), &min_letters_amount)
            .await
        {
            Some(s) => s,
            None => {
                self.end_game("**Game Ended**\n\nNo messages found that meet the requirements.")
                    .await?;
                return Ok(());
            }
        };
        let random_author = UserId::new(random_author).to_user(&self.ctx.http).await?;

        let embed = self.create_embed_with_color(
            format!(
                "**Can you guess who wrote this message?**\n\n```\n{}\n```",
                random_message
            ),
            0xFEE75C,
        );

        let skip_buton = CreateButton::new("skip")
            .style(ButtonStyle::Primary)
            .label("Reveal Answer");

        let end_button = CreateButton::new("end")
            .style(ButtonStyle::Danger)
            .label("End Game");

        let mut message = self
            .command
            .channel_id
            .send_message(
                &self.ctx.http,
                CreateMessage::new()
                    .embed(embed.clone())
                    .button(skip_buton.clone())
                    .button(end_button.clone()),
            )
            .await?;

        loop {
            let mut interaction_stream = message
                .await_component_interaction(&self.ctx.shard)
                .stream();
            let mut message_stream = self.command.channel_id.await_reply(&self.ctx).stream();

            tokio::select! {
                interaction = interaction_stream.next() => {
                    match interaction {
                        Some(interaction) => {
                            match interaction.data.custom_id.as_str() {
                                "skip" => {
                                    message.edit(&self.ctx.http,
                                        serenity::all::EditMessage::new()
                                            .embed(embed.clone())
                                            .button(skip_buton.clone().disabled(true))
                                            .button(end_button.clone().disabled(true))
                                    ).await?;

                                    self.command
                                        .channel_id
                                        .send_message(&self.ctx.http, CreateMessage::new().content(format!(
                                            "**Answer Revealed:** The message was written by `{}`", random_author.name
                                        )))
                                        .await?;

                                    interaction
                                        .create_response(&self.ctx.http, CreateInteractionResponse::Acknowledge)
                                        .await?;
                                    break;
                                }
                                "end" => {
                                    message.edit(&self.ctx.http,
                                        serenity::all::EditMessage::new()
                                            .embed(embed.clone())
                                            .button(skip_buton.clone().disabled(true))
                                            .button(end_button.clone().disabled(true))
                                    ).await?;

                                    interaction
                                        .create_response(&self.ctx.http, CreateInteractionResponse::Acknowledge)
                                        .await?;
                                    self.end_game("**Game Ended**\n\nThe game has been ended by user request.").await?;
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                        None => {}
                    }
                }

                message_collector = message_stream.next() => {
                    match message_collector {
                        Some(user_message) => {
                            if self.check_msg_content(user_message, &random_author).await? {
                                message.edit(&self.ctx.http,
                                    serenity::all::EditMessage::new()
                                        .embed(embed.clone())
                                        .button(skip_buton.clone().disabled(true))
                                        .button(end_button.clone().disabled(true))
                                ).await?;
                                break;
                            }
                        }
                        None => {
                                message.edit(&self.ctx.http,
                                    serenity::all::EditMessage::new()
                                        .embed(embed.clone())
                                        .button(skip_buton.clone().disabled(true))
                                        .button(end_button.clone().disabled(true))
                                ).await?;

                            self.end_game("**Time's Up!**\n\nNo one guessed correctly within the time limit.")
                                .await?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn end_game(&mut self, reason: impl Into<String>) -> Result<(), Error> {
        let embed = self.create_embed_with_color(reason, 0xED4245);

        self.command
            .channel_id
            .send_message(&self.ctx.http, CreateMessage::new().embed(embed))
            .await?;

        self.game_ended = true;

        Ok(())
    }

    fn create_embed_with_color(&self, content: impl Into<String>, color: u32) -> CreateEmbed {
        CreateEmbed::new()
            .title("Message Guesser")
            .description(content)
            .color(color)
    }

    async fn check_msg_content(
        &self,
        user_message: Message,
        random_author: &User,
    ) -> Result<bool, Error> {
        let display_name = random_author.display_name();
        let correct_guesses = vec![random_author.name.as_str(), &display_name];

        if correct_guesses.iter().any(|&correct_guess| {
            self.matches(
                &correct_guess.to_lowercase(),
                &user_message.content.to_lowercase(),
            )
            .is_some()
        }) {
            self.command
                .channel_id
                .send_message(
                    &self.ctx.http,
                    CreateMessage::new().content(format!(
                        "**Correct!** <@{}> got it right! The message was written by `{}`",
                        user_message.author.id.get(),
                        random_author.name
                    )),
                )
                .await?;

            return Ok(true);
        }

        // wrong guess
        return Ok(false);
    }

    fn matches(&self, src: &str, content: &str) -> Option<bool> {
        let difficulty = 1.0;

        if src == content {
            Some(true)
        } else if levenshtein_similarity(src, content) > difficulty
            || gestalt_pattern_matching(src, content) > difficulty + 0.1
        {
            Some(false)
        } else {
            None
        }
    }

    async fn get_random_message(
        &self,
        guild_id: &u64,
        min_letters_amount: &u64,
    ) -> Option<(String, u64)> {
        match self
            .database
            .get_random_message(*guild_id, *min_letters_amount)
            .await
        {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to get random message: {}", e);
                None
            }
        }
    }
}
