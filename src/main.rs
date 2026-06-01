use std::path::PathBuf;

use clap::Parser;
use mapx::{Config, PipelineMode, run_pipeline, output};

#[derive(Parser)]
#[command(name = "mapx", about = "Fast code context mapper for code aid tools", version = "0.1.0")]
struct Args {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long)]
    query: String,

    #[arg(long, default_value = "full")]
    mode: String,

    #[arg(long, default_value = "json")]
    format: String,

    #[arg(long)]
    ranker: Option<String>,

    #[arg(long, default_value = "20")]
    max: usize,

    #[arg(long, default_value = "http://localhost:11434")]
    ollama_base: String,

    #[arg(long)]
    lang_dir: Option<PathBuf>,

    /// Include caller→callee call graph edges in output
    #[arg(long)]
    call_graph: bool,
}

fn main() {
    let args = Args::parse();

    let config = Config {
        root: args.root,
        query: args.query,
        mode: PipelineMode::from_str(&args.mode),
        max_results: args.max,
        rank_model: args.ranker,
        ollama_base: args.ollama_base,
        format: args.format,
        lang_dir: args.lang_dir,
        call_graph: args.call_graph,
    };

    let result = output::with_spinner("Mapping code context", || run_pipeline(&config));

    match result {
        Ok(result) => {
            let stdout = std::io::stdout();
            let mut handle = stdout.lock();
            match config.format.as_str() {
                "lines" => output::write_lines(&result.tags, &mut handle).ok(),
                _ => output::write_json(&result, &mut handle).ok(),
            };
        }
        Err(e) => {
            eprintln!("[mapx] Error: {e}");
            std::process::exit(1);
        }
    }
}
