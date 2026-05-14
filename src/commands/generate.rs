use serenity::all::{
    CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption,
    EditInteractionResponse,
};
use serenity::prelude::*;
use serenity::Error;
use std::sync::Arc;

use crate::database::Database;

pub async fn execute(
    ctx: &Context,
    command: &CommandInteraction,
    database: Arc<Database>,
) -> Result<(), Error> {
    command.defer(&ctx.http).await?;

    let guild = match command.guild_id {
        Some(s) => s,
        _ => return Ok(()),
    };
    let channel = command.channel_id;

    let options = &command.data.options;

    // let it stay here for now
    let word = options
        .iter()
        .find(|opt| opt.name == "word")
        .and_then(|opt| opt.value.as_str());

    let builder = match database
        .generate_random_sentence(guild.get(), channel.get())
        .await
    {
        Some(markov_message) => EditInteractionResponse::new().content(markov_message),
        None => {
            EditInteractionResponse::new().content("There was a problem generating your message.")
        }
    };

    command.edit_response(&ctx.http, builder).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("generate")
        .description("Generates a markov message.")
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "word",
            "What the sentence will start with",
        ))
}
