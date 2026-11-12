//! Command-line interface. Argument parsing and printing only: all tokenizer logic lives in
//! the library, so everything here is also reachable through the library API.

use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use somali_tokenizer::benchmark::{self, BenchmarkOptions};
use somali_tokenizer::corpus::{self, JsonlDocs, Split};
use somali_tokenizer::stats::{self, StatsOptions};
use somali_tokenizer::training::{self, TrainConfig, TrainingSource};
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
    /// Train a ByteLevel BPE, or derive smaller vocabularies from a trained one.
    Train(TrainArgs),
    /// Score tokenizers on the held-out evaluation split.
    Benchmark(BenchmarkArgs),
    /// Tokenize a whole corpus and report measured token counts.
    Stats(StatsArgs),
}

#[derive(Args)]
struct TrainArgs {
    /// Release corpus in JSONL; the evaluation split is held out automatically.
    #[arg(long, conflicts_with = "text", required_unless_present_any = ["text", "derive_from"])]
    corpus: Option<PathBuf>,
    /// Plain-text training file, one line per training line, instead of --corpus.
    #[arg(long)]
    text: Option<PathBuf>,
    /// Where to write the artifact. A run record is written beside it.
    #[arg(long, default_value = "artifacts/scratch/somali-bpe-v2.json")]
    output: PathBuf,
    /// Vocabulary size, including special tokens and the byte alphabet.
    #[arg(long, default_value_t = training::DEFAULT_VOCAB_SIZE)]
    vocab_size: usize,
    /// Minimum pair frequency for a merge.
    #[arg(long, default_value_t = training::DEFAULT_MIN_FREQUENCY)]
    min_frequency: u64,
    /// Comma-separated vocabulary sizes; writes somali-bpe-v2-SIZE.json into --sweep-dir.
    #[arg(long, value_delimiter = ',')]
    sweep: Vec<usize>,
    /// Directory for sweep candidates.
    #[arg(long, default_value = "artifacts/scratch/sweep")]
    sweep_dir: PathBuf,
    /// Derive each size by truncating this trained artifact instead of reading a corpus.
    #[arg(long)]
    derive_from: Option<PathBuf>,
    /// Train on every document, including the evaluation split.
    #[arg(long)]
    no_holdout: bool,
    /// Read at most this many documents (or lines, with --text).
    #[arg(long)]
    limit: Option<usize>,
    /// Replace an existing output file.
    #[arg(long)]
    force: bool,
    /// Do not draw a progress bar.
    #[arg(long)]
    quiet: bool,
}

#[derive(Args)]
struct BenchmarkArgs {
    /// Evaluation JSONL: eval_holdout.jsonl, or a release corpus to take the split from.
    #[arg(long)]
    eval: PathBuf,
    /// Tokenizer to score, as LABEL=PATH; repeatable. Defaults to the bundled v2.
    #[arg(long = "tokenizer", value_name = "LABEL=PATH")]
    tokenizers: Vec<String>,
    /// Score every document instead of only the evaluation split.
    #[arg(long)]
    all_documents: bool,
    /// Score at most this many documents.
    #[arg(long)]
    limit: Option<usize>,
    /// Verify decode(encode(x)) on the first this-many documents.
    #[arg(long, default_value_t = benchmark::DEFAULT_ROUNDTRIP_LIMIT)]
    roundtrip_limit: usize,
    /// Where to write the JSON report.
    #[arg(long, default_value = "reports/benchmark_results.json")]
    output: PathBuf,
}

#[derive(Args)]
struct StatsArgs {
    /// Release corpus in JSONL.
    #[arg(long)]
    corpus: PathBuf,
    /// Tokenizer to run, as LABEL=PATH; repeatable. Defaults to the bundled v2 as `v2`.
    #[arg(long = "tokenizer", value_name = "LABEL=PATH")]
    tokenizers: Vec<String>,
    /// Skip the round-trip check on every 500th document.
    #[arg(long)]
    no_roundtrip: bool,
    /// Where to write the JSON report.
    #[arg(long, default_value = "reports/corpus_token_stats.json")]
    output: PathBuf,
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
        Command::Train(args) => run_train(&args)?,
        Command::Benchmark(args) => run_benchmark(&args, &mut out)?,
        Command::Stats(args) => run_stats(&args)?,
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

fn run_train(args: &TrainArgs) -> Result<()> {
    let sizes = if args.sweep.is_empty() {
        vec![args.vocab_size]
    } else {
        args.sweep.clone()
    };
    for &vocab_size in &sizes {
        let output = if args.sweep.is_empty() {
            args.output.clone()
        } else {
            args.sweep_dir
                .join(format!("somali-bpe-v2-{vocab_size}.json"))
        };
        check_writable(&output, args.force)?;
        if let Some(source) = &args.derive_from {
            let artifact: Value = serde_json::from_str(
                &std::fs::read_to_string(source)
                    .with_context(|| format!("reading {}", source.display()))?,
            )?;
            let derived = training::derive(&artifact, vocab_size)?;
            write_file(&output, &serde_json::to_string(&derived)?)?;
            eprintln!("derived vocab={vocab_size} -> {}", output.display());
            continue;
        }
        let config = TrainConfig {
            vocab_size,
            min_frequency: args.min_frequency,
            show_progress: !args.quiet,
        };
        let source = match (&args.corpus, &args.text) {
            (Some(path), _) => TrainingSource::Jsonl {
                path: path.clone(),
                holdout: !args.no_holdout,
                limit: args.limit,
            },
            (None, Some(path)) => TrainingSource::Text {
                path: path.clone(),
                limit: args.limit,
            },
            (None, None) => anyhow::bail!("pass --corpus, --text, or --derive-from"),
        };
        let (tokenizer, record) = training::train_from(&config, &source)?;
        write_file(&output, &tokenizer.to_json(true)?)?;
        let record_path = output.with_extension("run.json");
        write_file(
            &record_path,
            &(serde_json::to_string_pretty(&record)? + "\n"),
        )?;
        eprintln!(
            "trained vocab={} on {} lines in {:.1}s -> {}",
            record.vocab_size,
            record.lines,
            record.elapsed_seconds,
            output.display()
        );
    }
    Ok(())
}

fn run_benchmark(args: &BenchmarkArgs, out: &mut impl Write) -> Result<()> {
    let tokenizers = load_labelled(&args.tokenizers, "v2-bytelevel")?;
    let split = if args.all_documents {
        Split::All
    } else {
        Split::Eval
    };
    let docs = JsonlDocs::open(&args.eval, split)?;
    let options = BenchmarkOptions {
        roundtrip_limit: args.roundtrip_limit,
        limit: args.limit,
    };
    let started = Instant::now();
    let report = benchmark::run(&tokenizers, docs, &options)?;
    eprintln!("scored in {:.1}s", started.elapsed().as_secs_f64());

    let mut full = json!({"eval_split": args.eval.display().to_string()});
    merge(&mut full, report);
    write_file(&args.output, &(serde_json::to_string_pretty(&full)? + "\n"))?;

    writeln!(
        out,
        "{:24} {:>7} {:>7} {:>7} {:>7} {:>9} {:>11}",
        "tokenizer", "mean", "median", "p95", "B/tok", "unk docs", "round-trip"
    )?;
    for (label, summary) in full["tokenizers"].as_object().into_iter().flatten() {
        let number = |key: &str| summary[key].as_f64().unwrap_or(f64::NAN);
        let fidelity = summary["roundtrip_fidelity"]
            .as_f64()
            .map_or_else(|| "n/a".to_owned(), |f| format!("{f:.6}"));
        writeln!(
            out,
            "{label:24} {:7.4} {:7.4} {:7.4} {:7.3} {:>9} {fidelity:>11}",
            number("mean_tokens_per_word"),
            number("median_tokens_per_word"),
            number("p95_tokens_per_word"),
            number("bytes_per_token"),
            summary["unk_documents"],
        )?;
    }
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

fn run_stats(args: &StatsArgs) -> Result<()> {
    let tokenizers = load_labelled(&args.tokenizers, "v2")?;
    let docs = JsonlDocs::open(&args.corpus, Split::All)?;
    let options = StatsOptions {
        check_roundtrip: !args.no_roundtrip,
    };
    let started = Instant::now();
    let summaries = stats::run(&tokenizers, docs, &options)?;
    let report = json!({
        "corpus": args.corpus.display().to_string(),
        "corpus_fingerprint": corpus::fingerprint(&args.corpus)?,
        "elapsed_seconds": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
        "roundtrip_stride": stats::ROUNDTRIP_STRIDE,
        "ratio_bin_width": stats::RATIO_BIN_WIDTH,
        "tokenizers": summaries,
    });
    write_file(
        &args.output,
        &(serde_json::to_string_pretty(&report)? + "\n"),
    )?;
    eprintln!("wrote {}", args.output.display());
    Ok(())
}

/// Parse LABEL=PATH entries (a bare PATH is labelled by its file stem). With none, use the
/// bundled v2 under `default_label`.
fn load_labelled(
    entries: &[String],
    default_label: &str,
) -> Result<Vec<(String, tokenizers::Tokenizer)>> {
    if entries.is_empty() {
        let bundled = SomaliTokenizer::v2()?;
        return Ok(vec![(default_label.to_owned(), bundled.inner().clone())]);
    }
    entries
        .iter()
        .map(|entry| {
            let (label, path) = match entry.split_once('=') {
                Some((label, path)) => (label.to_owned(), PathBuf::from(path)),
                None => {
                    let path = PathBuf::from(entry);
                    let label = path
                        .file_stem()
                        .map_or_else(|| entry.clone(), |s| s.to_string_lossy().into_owned());
                    (label, path)
                }
            };
            let tokenizer = benchmark::load_artifact(&path)
                .with_context(|| format!("loading {}", path.display()))?;
            Ok((label, tokenizer))
        })
        .collect()
}

/// Refuse to overwrite without --force, and never write into a released artifact path.
fn check_writable(path: &Path, force: bool) -> Result<()> {
    let components: Vec<Component> = path.components().collect();
    let in_releases = components
        .windows(2)
        .any(|pair| pair[0].as_os_str() == "artifacts" && pair[1].as_os_str() == "releases");
    if in_releases {
        anyhow::bail!(
            "{} is inside artifacts/releases/; released artifacts are immutable. Train into \
             artifacts/scratch/ and promote by hand.",
            path.display()
        );
    }
    if path.exists() && !force {
        anyhow::bail!(
            "{} already exists; pass --force to replace it",
            path.display()
        );
    }
    Ok(())
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

fn merge(target: &mut Value, extra: Value) {
    if let (Some(target), Value::Object(extra)) = (target.as_object_mut(), extra) {
        target.extend(extra);
    }
}
