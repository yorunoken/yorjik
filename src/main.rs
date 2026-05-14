use dotenvy::dotenv;
use serenity::prelude::*;
use std::env;
use std::sync::Arc;

mod commands;
mod database;
mod event_handler;
mod utils;

#[tokio::main]
async fn main() {
    // load env variables
    dotenv().ok();

    // initialize database
    let database = Arc::new(
        database::Database::new("sqlite:data.db")
            .await
            .expect("Failed to initialize database"),
    );

    let discord_token =
        env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN to be defined in environment.");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let commands = commands::commands_vecs();
    let registered = commands::register_vecs();

    // build the Discord client, and pass in our event handler
    let mut client = Client::builder(discord_token, intents)
        .event_handler(event_handler::Handler {
            commands,
            registered,
            database: database.clone(),
        })
        .await
        .expect("Error creating client.");

    // run the client
    if let Err(reason) = client.start().await {
        println!("Error starting client: {:?}", reason);
    }
}
