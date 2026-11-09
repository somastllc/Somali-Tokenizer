//! Command-line interface. Argument parsing and printing only: all tokenizer logic lives in
//! the library, so everything here is also reachable through the library API.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use somali_tokenizer::SomaliTokenizer;

#[derive(Parser)]
#[command(
    name = "somali-tokenizer",
    version,
    about = "Somali ByteLevel BPE tokenizer"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Encode text to token ids. Reads TEXT, or one text per line from stdin.
    Encode {
        #[command(flatten)]
        source: Source,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Ids)]
        format: Format,
        /// Text to encode. Omit to read stdin line by line.
        text: Option<String>,
    },
    /// Decode token ids to text. Reads IDS, or one space-separated sequence per stdin line.
    Decode {
        #[command(flatten)]
        source: Source,
        /// Drop special tokens from the output.
        #[arg(long)]
        skip_special_tokens: bool,
        /// Token ids to decode. Omit to read stdin line by line.
        ids: Vec<u32>,
    },
    /// Print the vocabulary size and special tokens of an artifact.
    Inspect {
        #[command(flatten)]
        source: Source,
    },
}

#[derive(Args)]
struct Source {
    /// Path to a tokenizer.json. Defaults to the bundled v2 release.
    #[arg(long, global = true)]
    tokenizer: Option<PathBuf>,
    /// Load from a Hugging Face Hub repository instead, e.g. somast/somali-bpe-v2.
    #[cfg(feature = "http")]
    #[arg(long, global = true, conflicts_with = "tokenizer")]
    hub: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    /// Space-separated ids.
    Ids,
    /// Space-separated tokens.
    Tokens,
    /// One JSON object per input with ids and tokens.
    Json,
}

impl Source {
    fn load(&self) -> Result<SomaliTokenizer> {
        #[cfg(feature = "http")]
        if let Some(repo) = &self.hub {
            return SomaliTokenizer::from_pretrained(repo, None)
                .with_context(|| format!("loading {repo} from the Hugging Face Hub"));
        }
        match &self.tokenizer {
            Some(path) => SomaliTokenizer::from_file(path)
                .with_context(|| format!("loading {}", path.display())),
            None => SomaliTokenizer::v2().context("loading the bundled v2 tokenizer"),
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    match cli.command {
        Command::Encode {
            source,
            format,
            text,
        } => {
            let tokenizer = source.load()?;
            for_each_input(text, |line| {
                let encoding = tokenizer.encode(line)?;
                let rendered = match format {
                    Format::Ids => join(&encoding.ids),
                    Format::Tokens => encoding.tokens.join(" "),
                    Format::Json => serde_json::json!({
                        "ids": encoding.ids,
                        "tokens": encoding.tokens,
                    })
                    .to_string(),
                };
                writeln!(out, "{rendered}")?;
                Ok(())
            })?;
        }
        Command::Decode {
            source,
            skip_special_tokens,
            ids,
        } => {
            let tokenizer = source.load()?;
            if ids.is_empty() {
                for_each_input(None, |line| {
                    let ids = parse_ids(line)?;
                    writeln!(out, "{}", tokenizer.decode(&ids, skip_special_tokens)?)?;
                    Ok(())
                })?;
            } else {
                writeln!(out, "{}", tokenizer.decode(&ids, skip_special_tokens)?)?;
            }
        }
        Command::Inspect { source } => {
            let tokenizer = source.load()?;
            writeln!(out, "vocab_size: {}", tokenizer.vocab_size())?;
            writeln!(out, "special_tokens:")?;
            for token in tokenizer.special_tokens() {
                let id = tokenizer
                    .token_to_id(token)
                    .map_or_else(|| "missing".to_owned(), |id| id.to_string());
                writeln!(out, "  {id:>3}  {token}")?;
            }
        }
    }
    out.flush()?;
    Ok(())
}

/// Call `f` on `text` if given, otherwise on each line of stdin.
fn for_each_input(text: Option<String>, mut f: impl FnMut(&str) -> Result<()>) -> Result<()> {
    if let Some(text) = text {
        return f(&text);
    }
    for line in io::stdin().lock().lines() {
        f(&line.context("reading stdin")?)?;
    }
    Ok(())
}

fn parse_ids(line: &str) -> Result<Vec<u32>> {
    line.split_whitespace()
        .map(|part| {
            part.parse()
                .with_context(|| format!("not a token id: {part:?}"))
        })
        .collect()
}

fn join(ids: &[u32]) -> String {
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(" ")
}
