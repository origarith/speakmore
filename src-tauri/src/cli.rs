use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug, Clone, Default)]
#[command(name = "speakmore", about = "SpeakMore - Speech to Text")]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<CliDataCommand>,

    /// Start with the main window hidden
    #[arg(long)]
    pub start_hidden: bool,

    /// Disable the system tray icon
    #[arg(long)]
    pub no_tray: bool,

    /// Toggle transcription on/off (sent to running instance)
    #[arg(long)]
    pub toggle_transcription: bool,

    /// Toggle transcription with post-processing on/off (sent to running instance)
    #[arg(long)]
    pub toggle_post_process: bool,

    /// Cancel the current operation (sent to running instance)
    #[arg(long)]
    pub cancel: bool,

    /// Enable debug mode with verbose logging
    #[arg(long)]
    pub debug: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliDataCommand {
    /// Read and manage transcript history as JSON
    History(HistoryCommand),
    /// Read and patch app settings as JSON
    Config(ConfigCommand),
}

#[derive(Args, Debug, Clone)]
pub struct HistoryCommand {
    #[command(subcommand)]
    pub command: HistorySubcommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum HistorySubcommand {
    /// List history entries
    List {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        cursor: Option<i64>,
    },
    /// Get one history entry with run and event details
    Get { id: i64 },
    /// Save a user edit for a history entry
    Edit {
        id: i64,
        #[arg(long, conflicts_with = "stdin")]
        text: Option<String>,
        #[arg(long)]
        stdin: bool,
    },
    /// Clear the user edit for a history entry
    ClearEdit { id: i64 },
    /// Mark a history entry as saved
    Save { id: i64 },
    /// Mark a history entry as unsaved
    Unsave { id: i64 },
    /// Delete a history entry and its recording
    Delete { id: i64 },
}

#[derive(Args, Debug, Clone)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ConfigSubcommand {
    /// Get all settings, or a dotted settings path
    Get { path: Option<String> },
    /// Merge a JSON patch into settings and write it
    Patch {
        #[arg(long, conflicts_with = "stdin")]
        file: Option<std::path::PathBuf>,
        #[arg(long)]
        stdin: bool,
    },
    /// Validate a JSON patch without writing it
    Validate {
        #[arg(long, conflicts_with = "stdin")]
        file: Option<std::path::PathBuf>,
        #[arg(long)]
        stdin: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn legacy_flags_still_parse() {
        let args = CliArgs::parse_from(["speakmore", "--toggle-transcription", "--debug"]);

        assert!(args.toggle_transcription);
        assert!(args.debug);
        assert!(args.command.is_none());
    }

    #[test]
    fn history_subcommand_parses() {
        let args = CliArgs::parse_from(["speakmore", "history", "list", "--limit", "1"]);

        let Some(CliDataCommand::History(history)) = args.command else {
            panic!("expected history command");
        };
        let HistorySubcommand::List { limit, cursor } = history.command else {
            panic!("expected history list command");
        };
        assert_eq!(limit, 1);
        assert_eq!(cursor, None);
    }

    #[test]
    fn config_subcommand_parses() {
        let args = CliArgs::parse_from([
            "speakmore",
            "config",
            "get",
            "asr_family_settings.whisper.language",
        ]);

        let Some(CliDataCommand::Config(config)) = args.command else {
            panic!("expected config command");
        };
        let ConfigSubcommand::Get { path } = config.command else {
            panic!("expected config get command");
        };
        assert_eq!(
            path.as_deref(),
            Some("asr_family_settings.whisper.language")
        );
    }
}
