mod loader;

use clap::Parser;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use swan_core::value::Value;
use texl_core::helpers::spawner_fn;
use texl_core::{Engine, loader_fn};
use tokio::task::LocalSet;
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "texl",
    about = "Renders a Texl template file to the terminal or an output file."
)]
struct Args {
    /// Template file to render. Relative imports resolve against its location.
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output file path. If omitted, the rendered document is written to stdout.
    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();

    // The engine is single-threaded (`Rc`-based): its tasks run on a LocalSet
    match LocalSet::new().run_until(run(args)).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("texl: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Creates an engine that spawns its tasks on the current LocalSet and loads
/// documents from the local file system, both as `file://` URLs and as plain paths.
fn create_engine() -> Engine {
    let engine = Engine::new(Some(spawner_fn(|task| {
        tokio::task::spawn_local(task);
    })));
    engine.add_loader("file", loader_fn(loader::load_file));
    engine.add_default_loader(loader_fn(loader::load_file));
    engine
}

async fn run(args: Args) -> Result<(), String> {
    // The main document is loaded by its absolute URL, so its relative imports can be resolved
    let path = std::fs::canonicalize(&args.input)
        .map_err(|e| format!("{}: {e}", args.input.display()))?;
    let url = Url::from_file_path(&path)
        .map_err(|_| format!("{}: not a valid file path", path.display()))?;

    let engine = create_engine();
    let rendering = match engine.load(url.as_str()).await {
        Value::Namespace(doc) => match doc.get("rendering") {
            Value::Text(text) => text,
            _ => return Err(format!("{}: rendering failed", path.display())),
        },
        _ => return Err(format!("{}: could not be loaded", path.display())),
    };

    let write_result = match &args.output {
        Some(out_path) => File::create(out_path).and_then(|mut f| f.write_all(rendering.as_bytes())),
        None => io::stdout().lock().write_all(rendering.as_bytes()),
    };
    write_result.map_err(|e| match &args.output {
        Some(out_path) => format!("{}: {e}", out_path.display()),
        None => format!("stdout: {e}"),
    })
}
