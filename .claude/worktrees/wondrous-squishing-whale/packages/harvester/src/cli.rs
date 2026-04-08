//! Command-line interface for the harvester.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::DEFAULT_MAX_RESPONSE_SIZE;
use crate::error::{HarvesterError, Result};
use crate::harvester::download_law_with_max_size;
use crate::yaml::save_yaml;

/// RegelRecht Harvester - Download Dutch legislation from BWB repository.
#[derive(Parser)]
#[command(name = "regelrecht-harvester")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Download a law by BWB ID and convert to YAML.
    Download {
        /// BWB identifier (e.g., BWBR0018451)
        bwb_id: String,

        /// Effective date in YYYY-MM-DD format (default: today)
        #[arg(short, long)]
        date: Option<String>,

        /// Output directory (default: regulation/nl/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum response size in MB (default: 100)
        ///
        /// Increase this for exceptionally large laws like Wet op het financieel
        /// toezicht (52.6 MB). Most laws are under 5 MB.
        #[arg(long, default_value_t = DEFAULT_MAX_RESPONSE_SIZE / (1024 * 1024))]
        max_size: u64,
    },
}

/// Run the CLI.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            bwb_id,
            date,
            output,
            max_size,
        } => download_command(&bwb_id, date.as_deref(), output.as_deref(), max_size),
    }
}

/// Execute the download command.
fn download_command(
    bwb_id: &str,
    date: Option<&str>,
    output: Option<&std::path::Path>,
    max_size_mb: u64,
) -> Result<()> {
    // Use today if no date provided
    let effective_date = date
        .map(String::from)
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    // Validate output directory exists (if specified) before downloading
    if let Some(output_dir) = output {
        if !output_dir.exists() {
            return Err(HarvesterError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Output directory does not exist: {}", output_dir.display()),
            )));
        }
        if !output_dir.is_dir() {
            return Err(HarvesterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Output path is not a directory: {}", output_dir.display()),
            )));
        }
    }

    println!(
        "{} {} for date {}",
        style("Downloading").bold(),
        style(bwb_id).cyan(),
        style(&effective_date).green()
    );
    println!();

    // Create progress spinner
    let pb = ProgressBar::new_spinner();
    #[allow(clippy::expect_used)] // Static template string that is guaranteed to be valid
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("valid template"),
    );

    // Download and parse
    pb.set_message("Downloading WTI metadata...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let law = match download_law_with_max_size(bwb_id, &effective_date, max_size_mb) {
        Ok(law) => law,
        Err(e) => {
            pb.finish_and_clear();
            return Err(e);
        }
    };

    pb.set_message("Processing articles...");

    println!("  Title: {}", style(&law.metadata.title).green());
    println!("  Type: {}", law.metadata.regulatory_layer.as_str());
    println!("  Articles: {}", law.articles.len());
    if !law.warnings.is_empty() {
        println!("  Warnings: {}", style(law.warnings.len()).yellow().bold());
    }

    // Save to YAML
    pb.set_message("Saving YAML...");

    let output_path = match save_yaml(&law, &effective_date, output) {
        Ok(path) => path,
        Err(e) => {
            pb.finish_and_clear();
            return Err(e);
        }
    };

    pb.finish_and_clear();

    println!();
    println!(
        "{} {}",
        style("Saved to:").green().bold(),
        output_path.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_download() {
        let cli = Cli::parse_from(["regelrecht-harvester", "download", "BWBR0018451"]);

        let Commands::Download {
            bwb_id,
            date,
            output,
            max_size,
        } = cli.command;
        assert_eq!(bwb_id, "BWBR0018451");
        assert!(date.is_none());
        assert!(output.is_none());
        assert_eq!(max_size, 100); // Default 100 MB
    }

    #[test]
    fn test_cli_parse_download_with_date() {
        let cli = Cli::parse_from([
            "regelrecht-harvester",
            "download",
            "BWBR0018451",
            "--date",
            "2025-01-01",
        ]);

        let Commands::Download { bwb_id, date, .. } = cli.command;
        assert_eq!(bwb_id, "BWBR0018451");
        assert_eq!(date, Some("2025-01-01".to_string()));
    }

    #[test]
    fn test_cli_parse_download_with_max_size() {
        let cli = Cli::parse_from([
            "regelrecht-harvester",
            "download",
            "BWBR0018451",
            "--max-size",
            "200",
        ]);

        let Commands::Download { max_size, .. } = cli.command;
        assert_eq!(max_size, 200);
    }
}
