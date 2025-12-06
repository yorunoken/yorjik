use serenity::all::{
    CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbed,
    EditInteractionResponse,
};
use serenity::prelude::*;
use serenity::Error;
use std::sync::Arc;

use crate::database::Database;

const MAX_DESCRIPTION_LENGTH: usize = 4000;

pub async fn execute(
    ctx: &Context,
    command: &CommandInteraction,
    database: Arc<Database>,
) -> Result<(), Error> {
    command.defer(&ctx.http).await?;

    let guild_id = match command.guild_id {
        Some(s) => s,
        _ => return Ok(()),
    };

    let options = &command.data.options;

    let member_id = options
        .iter()
        .find(|opt| opt.name == "user")
        .and_then(|opt| opt.value.as_user_id())
        .map(|u| u.get());

    let excludes = options
        .iter()
        .find(|opt| opt.name == "exclude_word")
        .and_then(|opt| opt.value.as_str());

    let excludes_array: Option<Vec<String>> = excludes.map(|v| {
        v.split(",")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    });

    let min_word_length = options
        .iter()
        .find(|opt| opt.name == "min_word_length")
        .and_then(|opt| opt.value.as_i64())
        .unwrap_or(3);

    let selected_word = options
        .iter()
        .find(|opt| opt.name == "word")
        .and_then(|opt| opt.value.as_str());

    let limit = 50;

    let leaderboard = match database
        .get_leaderboard_data(
            guild_id.get(),
            member_id,
            selected_word,
            min_word_length,
            excludes_array,
            limit,
        )
        .await
    {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Failed to fetch leaderboard data: {}", e);
            EditInteractionResponse::new()
                .content("An error occurred while fetching the leaderboard.");
            return Ok(());
        }
    };

    let mut description = String::new();

    for (index, (word, author_id, count)) in leaderboard.iter().enumerate() {
        let entry = format!(
            "**{}**. `{}`  -  {} uses by <@{}>\n",
            index + 1,
            word,
            count,
            author_id
        );

        if description.len() + entry.len() > MAX_DESCRIPTION_LENGTH {
            description.push_str("...");
            break;
        }
        description.push_str(&entry);
    }

    if description.is_empty() {
        description = "No data found matching your criteria.".to_string();
    }

    description = description.trim_end().to_string();

    let embed = EditInteractionResponse::new().embed(
        CreateEmbed::new()
            .title("Word Usage Leaderboard")
            .description(format!("**Server:** {}\n\n{}", guild_id, description))
            .color(0x5865F2)
            .footer(serenity::all::CreateEmbedFooter::new(format!(
                "Showing top {} entries",
                leaderboard.len()
            ))),
    );

    command.edit_response(&ctx.http, embed).await?;
    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("leaderboard")
        .description("Get the leaderboard of a server")
        .add_option(CreateCommandOption::new(
            serenity::all::CommandOptionType::User,
            "user",
            "Get a user's messages",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "word",
            "Get the leaderboard of a word",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::String,
            "exclude_word",
            "Excludes a word, usage: `word,to,exclude`",
        ))
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "min_word_length",
            "Minimum word length to fetch from database",
        ))
}
