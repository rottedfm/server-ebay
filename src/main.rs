// -- src/main.rs

// - TODO:
// - x11 window for geckodriver
// - allow ssh access to geckodriver window
// - build
// - teardown
// - view

mod cli;
mod client;
mod utils;

use anyhow::Context;
use clap::Parser;
use cli::{Cli, Commands};
use client::BrowserClient;
use dotenvy::dotenv;
use utils::setup_logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init Logger
    setup_logger()?;

    // Parse cli args
    let cli = Cli::parse();

    // Load .env file to std::env
    dotenv().ok();

    // Load email/password creds from .env file
    let email = std::env::var("EBAY_EMAIL").context("Missing EBAY_EMAIL")?;
    let password = std::env::var("EBAY_PASSWORD").context("Missing EBAY_PASSWORD")?;

    // Match the subcommand provided via the Cli structo
    match &cli.command {
        // Start eBay bot
        Commands::Build { offer_percentage } => {
            let mut driver = BrowserClient::build().await?;
            driver.ebay_signin(&email, &password).await?;
        }
        Commands::Teardown => {
            let driver = BrowserClient::teardown().await?;
        }
        Commands::View => {
            todo!();
        }
    }

    Ok(())
}
