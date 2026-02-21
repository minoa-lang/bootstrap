
use clap::clap_derive::Parser;

#[derive(Parser)]
pub struct Arguments {

    #[arg(long)]
    pub output_token_csv: bool,


    #[arg(short, default_value = "output")]
    pub output: String,

    
    pub input_files: Vec<String>,
}