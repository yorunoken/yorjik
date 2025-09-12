use std::sync::Arc;
use std::{thread, time};

use serenity::all::{
    CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption, CreateMessage,
    EditInteractionResponse, MessageId, MessagePagination,
};
use serenity::prelude::*;
use serenity::Error;

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

    let mut before_message_id = command
        .data
        .options
        .iter()
        .find(|opt| opt.name == "before")
        .and_then(|opt| opt.value.as_i64())
        .and_then(|n| n.try_into().ok());

    let channel_id = command.channel_id;
    let limit = 100;
    let mut loop_count = 0;
    let mut total_messages_collected = 0;

    println!(
        "Starting message collection for channel {} in guild {}",
        channel_id, guild_id
    );

    if let Err(e) = command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(format!(
                "Starting message collection for channel {} in guild {}",
                channel_id, guild_id
            )),
        )
        .await
    {
        eprintln!("Failed to update Discord progress: {}", e);
    }

    loop {
        loop_count += 1;
        println!(
            "Loop {}: Fetching messages before ID: {:#?}",
            loop_count, before_message_id
        );

        let pagination = before_message_id.map(|id| MessagePagination::Before(MessageId::new(id)));

        match ctx
            .http
            .get_messages(channel_id, pagination, Some(limit))
            .await
        {
            Ok(messages) => {
                println!("Fetched {} messages", messages.len());

                for msg in &messages {
                    if msg.author.bot {
                        continue;
                    }

                    let _ = database
                        .insert_message(
                            msg.id.get(),
                            msg.author.id.get(),
                            msg.channel_id.get(),
                            guild_id.get(),
                            &msg.content,
                        )
                        .await;
                }

                total_messages_collected += messages.len();
                println!(
                    "Inserted {} messages into database. Total collected: {}",
                    messages.len(),
                    total_messages_collected
                );

                if loop_count % 5 == 0 {
                    let progress_message = format!(
                        "**Collection Progress**\n\
                        Total messages collected: {}",
                        loop_count,
                    );

                    if let Err(e) = command
                        .edit_response(
                            &ctx.http,
                            EditInteractionResponse::new().content(progress_message),
                        )
                        .await
                    {
                        eprintln!("Failed to update Discord progress: {}", e);
                    }
                }

                before_message_id = Some(messages[99].id.get());

                if messages.len() < limit as usize {
                    println!("Reached end of messages. Collection complete!");

                    let final_message = format!(
                        "**Collection Complete!**\n\
                        Total messages collected: {}",
                        total_messages_collected
                    );

                    if let Err(e) = command
                        .channel_id
                        .send_message(&ctx.http, CreateMessage::new().content(final_message))
                        .await
                    {
                        eprintln!("Failed to send completion message: {}", e);
                    }

                    break;
                }
            }
            Err(err) => loop {
                let mut tries = 0;
                tries += 1;

                if tries > 5 {
                    panic!(
                        "Error fetching messages (loop {}, attempt {}): {}. Panicking!!",
                        loop_count, tries, err
                    );
                }

                let retry_second = tries * 2;
                eprintln!(
                    "Error fetching messages (loop {}, attempt {}): {}. Retrying in {} seconds...",
                    loop_count, tries, err, retry_second
                );

                thread::sleep(time::Duration::from_secs(retry_second));
            },
        }

        // sleep between cycles
        println!(
            "Loop {} complete. Sleeping for 2 seconds before next batch...",
            loop_count
        );
        thread::sleep(time::Duration::from_secs(2));
    }

    Ok(())
}

pub fn register() -> CreateCommand {
    CreateCommand::new("collect")
        .description("Collects and records previous messages.")
        .add_option(CreateCommandOption::new(
            CommandOptionType::Integer,
            "before",
            "The ID of the message the bot will check before.",
        ))
}
