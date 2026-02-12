#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Codirigent - AI Coding Agent Orchestration IDE
//!
//! Entry point for the Codirigent application.
//!
//! # Features
//!
//! - `gpui-full`: Enable the full GPUI-based user interface
//! - `terminal`: Enable terminal rendering with alacritty_terminal
//!
//! # Running
//!
//! ```bash
//! # Run with GPUI interface
//! cargo run --features gpui-full
//!
//! # Run with GPUI and terminal support
//! cargo run --features "gpui-full,terminal"
//! ```

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting Codirigent...");

    // Launch GPUI application if feature is enabled
    #[cfg(feature = "gpui-full")]
    {
        tracing::info!("Launching GPUI application...");
        codirigent_ui::CodirigentApp::new().run();
    }

    // Without GPUI, just print info and exit
    #[cfg(not(feature = "gpui-full"))]
    {
        tracing::info!("Codirigent started without GPUI.");
        tracing::info!("To launch the GUI, run: cargo run --features gpui-full");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn test_tracing_subscriber_builds() {
        // Verify tracing subscriber can be built without panicking
        // Note: We don't call .init() because global subscriber can only be set once
        let result = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_default());
        // If we get here without panic, the subscriber built successfully
        drop(result);
    }
}
