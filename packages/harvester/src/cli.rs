//! Command-line interface for the harvester.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::DEFAULT_MAX_RESPONSE_SIZE;
use crate::cvdr::download_cvdr_law;
use crate::error::{HarvesterError, Result};
use crate::harvester::download_law_with_max_size;
use crate::http::create_client;
use crate::yaml::save_yaml;

/// RegelRecht Harvester - Download Dutch legislation from BWB and CVDR repositories.
#[derive(Parser)]
#[command(name = "regelrecht-harvester")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Download a law by BWB or CVDR ID and convert to YAML.
    Download {
        /// Law identifier: BWB ID (e.g., BWBR0018451) or CVDR ID (e.g., CVDR681386)
        law_id: String,

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

/// Detected law source based on the ID prefix.
enum LawSource {
    /// National law from BWB (Basiswettenbestand).
    Bwb,
    /// Decentrale regelgeving from CVDR.
    Cvdr,
}

/// Detect whether a law ID is BWB or CVDR based on its prefix.
fn detect_law_source(law_id: &str) -> Result<LawSource> {
    if law_id.starts_with("BWBR") {
        Ok(LawSource::Bwb)
    } else if law_id.starts_with("CVDR") {
        Ok(LawSource::Cvdr)
    } else {
        Err(HarvesterError::InvalidLawId(law_id.to_string()))
    }
}

/// Run the CLI.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Download {
            law_id,
            date,
            output,
            max_size,
        } => download_command(&law_id, date.as_deref(), output.as_deref(), max_size).await,
    }
}

/// Execute the download command.
async fn download_command(
    law_id: &str,
    date: Option<&str>,
    output: Option<&std::path::Path>,
    max_size_mb: u64,
) -> Result<()> {
    // Detect source type
    let source = detect_law_source(law_id)?;

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

    let source_label = match source {
        LawSource::Bwb => "BWB",
        LawSource::Cvdr => "CVDR",
    };

    println!(
        "{} {} ({}) for date {}",
        style("Downloading").bold(),
        style(law_id).cyan(),
        source_label,
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

    // Create HTTP client
    let client = create_client()?;

    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let law = match source {
        LawSource::Bwb => {
            pb.set_message("Downloading WTI metadata...");
            match download_law_with_max_size(&client, law_id, &effective_date, max_size_mb).await {
                Ok(law) => law,
                Err(e) => {
                    pb.finish_and_clear();
                    return Err(e);
                }
            }
        }
        LawSource::Cvdr => {
            pb.set_message("Searching CVDR via SRU...");
            match download_cvdr_law(&client, law_id, date).await {
                Ok(law) => law,
                Err(e) => {
                    pb.finish_and_clear();
                    return Err(e);
                }
            }
        }
    };

    pb.set_message("Processing articles...");

    println!("  Title: {}", style(&law.metadata.title).green());
    println!("  Type: {}", law.metadata.regulatory_layer.as_str());
    if let Some(creator) = &law.metadata.creator {
        println!("  Creator: {}", style(creator).cyan());
    }
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
    fn test_cli_parse_download_bwb() {
        let cli = Cli::parse_from(["regelrecht-harvester", "download", "BWBR0018451"]);

        let Commands::Download {
            law_id,
            date,
            output,
            max_size,
        } = cli.command;
        assert_eq!(law_id, "BWBR0018451");
        assert!(date.is_none());
        assert!(output.is_none());
        assert_eq!(max_size, 100); // Default 100 MB
    }

    #[test]
    fn test_cli_parse_download_cvdr() {
        let cli = Cli::parse_from(["regelrecht-harvester", "download", "CVDR681386"]);

        let Commands::Download { law_id, .. } = cli.command;
        assert_eq!(law_id, "CVDR681386");
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

        let Commands::Download { law_id, date, .. } = cli.command;
        assert_eq!(law_id, "BWBR0018451");
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

    #[test]
    fn test_detect_law_source_bwb() {
        assert!(matches!(
            detect_law_source("BWBR0018451"),
            Ok(LawSource::Bwb)
        ));
    }

    #[test]
    fn test_detect_law_source_cvdr() {
        assert!(matches!(
            detect_law_source("CVDR681386"),
            Ok(LawSource::Cvdr)
        ));
    }

    #[test]
    fn test_detect_law_source_invalid() {
        assert!(detect_law_source("INVALID").is_err());
        assert!(detect_law_source("").is_err());
    }
}
