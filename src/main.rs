use clap::{Parser, Subcommand};
use data_compression::algorithm;
use std::io::Result;
use std::fs::File;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand, Debug)]
enum Command {
    Huffman {
        action: Action,
        input: String,
        output: String,
        #[arg(short, long)]
        debug: bool,
    }
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum Action {
    Compress,
    Expand
}


fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Huffman{action, input, output, debug} => {
            let mut input = File::open(input)?;
    /*
    let mut output = BitFile::create("output.huffman")?;

    CompressFile(input, output)?;

    Ok(())
}
*/
        }
    }

    Ok(())
}
