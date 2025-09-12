use serenity::all::{
    CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption, CreateEmbed,
    EditInteractionResponse,
};
use serenity::prelude::*;
use serenity::Error;
use std::sync::Arc;

use std::collections::HashMap;

use crate::database::Database;

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
        .unwrap_or(0);

    let selected_word = options
        .iter()
        .find(|opt| opt.name == "word")
        .and_then(|opt| opt.value.as_str());

    let limit = 50;

    let prefix_list: Vec<&str> = vec![
        "$", "&", "!", ".", "m.", ">", "<", "[", "]", "@", "#", "%", "^", "*", ",",
    ];

    let embed = {
        let sentences = match database
            .get_messages_for_leaderboard(guild_id.get(), member_id)
            .await
        {
            Ok(sentences) => sentences,
            Err(e) => {
                eprintln!("Failed to fetch messages for leaderboard: {}", e);
                return Ok(());
            }
        };

        let mut word_counts: HashMap<String, HashMap<u64, usize>> = HashMap::new();

        for (content, author_id) in sentences {
            for word in content.split_whitespace() {
                let word = word.to_lowercase();

                if word.len() < min_word_length as usize {
                    continue;
                }

                if let Some(selected_word) = &selected_word {
                    if *selected_word != word {
                        continue;
                    }
                }

                if let Some(excludes) = &excludes_array {
                    if excludes.contains(&word) {
                        continue;
                    }
                }

                if prefix_list.iter().any(|&prefix| word.starts_with(prefix)) {
                    continue;
                }

                let author_counts = word_counts.entry(word).or_insert_with(HashMap::new);
                *author_counts.entry(author_id).or_insert(0) += 1;
            }
        }

        let mut leaderboard: Vec<(String, u64, usize)> = if let Some(selected_word) = selected_word
        {
            word_counts
                .get(selected_word)
                .map(|author_counts| {
                    author_counts
                        .iter()
                        .map(|(&author_id, &count)| (selected_word.to_string(), author_id, count))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            word_counts
                .into_iter()
                .map(|(word, author_counts)| {
                    let (top_author, top_count) = author_counts
                        .into_iter()
                        .max_by_key(|&(_, count)| count)
                        .unwrap();
                    (word, top_author, top_count)
                })
                .collect()
        };

        leaderboard.sort_by_key(|&(_, _, count)| std::cmp::Reverse(count));
        leaderboard.truncate(limit);

        let mut description = String::new();
        const MAX_DESCRIPTION_LENGTH: usize = 4000;

        for (index, (word, author_id, count)) in leaderboard.iter().enumerate() {
            let entry = format!(
                "**{}**. `{}`  —  {} uses by <@{}>\n",
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

        description = description.trim_end().to_string();

        EditInteractionResponse::new().embed(
            CreateEmbed::new()
                .title("Word Usage Leaderboard")
                .description(format!("**Server:** {}\n\n{}", guild_id, description))
                .color(0x5865F2)
                .footer(serenity::all::CreateEmbedFooter::new(format!(
                    "Showing top {} entries",
                    leaderboard.len()
                ))),
        )
    };

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
