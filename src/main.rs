
pub mod util;
pub mod os;
pub mod stats;
pub mod args;
pub mod context;
pub mod err_warn;

pub mod tokens;

pub mod lex;

use std::{collections::HashSet, fmt::Display, fs::File, io::{self, BufWriter, Write}, path::Path};

use clap::Parser;

use util::log;

use crate::{
    args::Arguments,
    context::Context,
    lex::{LexError, Lexer},
    tokens::TokenStream,
    util::{
        log::{get_logger, init_global_logger},
        time::Timer
    }
};


const LOG_PATH:            &'static str = "/logs/minoa-compiler.log";
const CSV_TOKEN_PATH_ROOT: &'static str = "/csv/";

fn main() {
    let mut timer = Timer::new(true);

    let args = Arguments::parse();

    let log_file = args.output.clone() + LOG_PATH;
    init_global_logger(true, Some(&log_file), log::Level::Debug, false).unwrap();

    let ctx = Context::new(args.get_stat_flags());

    let mut parsed_files = HashSet::new();
    for file_path in &args.input_files {
        if parsed_files.contains(file_path) { continue; }

        let Ok(toks) = lex_file(file_path, &ctx) else { continue; };

        if args.output_token_csv {
            let path = args.output.clone() + CSV_TOKEN_PATH_ROOT + &file_path[..file_path.len() - 3] + ".csv";
            _ = fmt_to_file(&path, &toks.csv_formatter());
            _ = log!(Verbose, "\n{}", toks.csv_formatter());
        }

        if args.log_token_tree {
            _ = log!(Verbose, "\nToken Tree: \n{}", toks.tree().get_formatter());
        }

        parsed_files.insert(file_path.clone());
    }

    _ = ctx.err_warn.lock().log();
    ctx.stats.lock().log_current();


    timer.stop();
    let total_time = timer.duration().as_secs_f32();
    _ = log!(Info, "Finished after {total_time:.03}s");
    
    _ = get_logger().flush();
}

pub fn lex_file(path: &str, ctx: &Context) -> Result<TokenStream, ()> {
    _ = log!(Debug, "Processing file: {path}");

    let path: &Path = path.as_ref();
    if path.extension().map_or("", |ext| ext.to_str().unwrap_or("")) != "mn" {
        return Err(());
    }

    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) => {
            ctx.err_warn.lock().add_lexer_error(LexError::IO(err));
            return Err(())
        },
    };

    let mut timer = Timer::new(true);

    let mut lexer = Lexer::new(file);
    let toks = match lexer.lex() {
        Ok(toks) => toks,
        Err(errs) => {
            for (span, err) in errs {
                _ = log!(Error, "{span}: {err}");
            }
            return Err(());
        },
    };

    timer.stop();

    ctx.stats.lock().add_file_lex_stats(path, timer.duration(), toks.len());

    Ok(toks)
}

fn fmt_to_file<T: Display>(path: &str, val: &T) -> io::Result<()> {
    let file = util::create_file_and_dirs(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_fmt(format_args!("{}", val))
}