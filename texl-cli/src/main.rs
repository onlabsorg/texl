use clap::Parser;
use is_terminal::IsTerminal;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use texl_core::context;
use texl_core::template;

#[derive(Parser, Debug)]
#[command(
    name = "texl",
    about = "Renders Texl template files or streams to the terminal or an output file."
)]
struct Args {
    /// Input file path. If omitted or set to "-", reads from stdin.
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Output file path. If specified, output is redirected to this file.
    #[arg(short, long, value_name = "OUTPUT")]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    // 1. Read input template from file or stdin
    let template_source = match &args.input {
        Some(path) if path.to_str() != Some("-") => std::fs::read_to_string(path)?,
        _ => {
            let mut stdin_buffer = String::new();
            io::stdin().read_to_string(&mut stdin_buffer)?;
            stdin_buffer
        }
    };

    // 2. Initialize default Texl evaluation scope
    let scope = context::create();

    // 3. Render template
    let rendered_output = template::render(&template_source, &scope).await;

    // 4. Determine output stream destination
    if let Some(out_path) = args.output {
        // Write to specified file (-o | --output)
        let mut file = File::create(out_path)?;
        file.write_all(rendered_output.as_bytes())?;
    } else {
        // Write to stdout (terminal or redirected pipe `>>`)
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(rendered_output.as_bytes())?;

        // Optional terminal detection logic for special terminal rendering vs raw stream output
        if handle.is_terminal() {
            // Output is attached directly to terminal/TTY
        } else {
            // Output is redirected via `>` or `>>` to another stream/pipe
        }
    }

    Ok(())
}
