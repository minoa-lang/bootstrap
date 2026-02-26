
use clap::clap_derive::Parser;

use crate::stats::CompilerStatFlags;

#[derive(Parser)]
pub struct Arguments {

    /// Output lexed tokens to csv file
    #[arg(long)]
    pub output_token_csv: bool,

    /// Enable collection of compiler statistics
    #[arg(long)]
    pub stats: bool,
    
    /// Enable stats collection for per-file lexing
    #[arg(long)]
    pub stats_per_file_lex: bool,

    /// Output directory
    #[arg(short, default_value = "output")]
    pub output: String,

    /// Source files
    pub input_files: Vec<String>,
}

impl Arguments {
    pub fn get_stat_flags(&self) -> CompilerStatFlags {
        let mut flags = CompilerStatFlags::None;

        flags.set(CompilerStatFlags::Enabled, self.stats);
        flags.set(CompilerStatFlags::PerFileLex, self.stats_per_file_lex);

        flags
    }
}