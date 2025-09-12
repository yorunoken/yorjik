use std::time::Instant;

use serenity::all::{CommandInteraction, CreateCommand, EditInteractionResponse};
use serenity::prelude::*;
use serenity::Error;

pub async fn execute(ctx: &Context, command: &CommandInteraction) -> Result<(), Error> {
    command.defer(&ctx.http).await?;
    let timer_start = Instant::now();

    let content = "Pong!";
    let builder = EditInteractionResponse::new().content(content);
    command.edit_response(&ctx.http, builder).await?;

    let elapsed = (Instant::now() - timer_start).as_millis();

    let builder = EditInteractionResponse::new().content(format!("{} ({:2}ms)", content, elapsed));
    command.edit_response(&ctx.http, builder).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("ping").description("Check if bot is alive.")
}
