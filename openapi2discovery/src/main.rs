use clap::Parser;
use openapi2discovery::{parse_openapi, transform};
use std::process;

#[derive(Parser)]
#[command(
    name = "openapi2discovery",
    about = "Convert OpenAPI 3.x specs to Discovery-style nested JSON"
)]
struct Cli {
    /// Input file path (use "-" for stdin)
    input: String,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,

    /// Pretty-print the JSON output
    #[arg(long)]
    pretty: bool,

    /// Override service name
    #[arg(long)]
    name: Option<String>,

    /// Override service version
    #[arg(long)]
    version: Option<String>,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let spec = parse_openapi(&cli.input)?;
    let doc = transform(&spec, cli.name.as_deref(), cli.version.as_deref());

    let json = if cli.pretty {
        serde_json::to_string_pretty(&doc)?
    } else {
        serde_json::to_string(&doc)?
    };

    match &cli.output {
        Some(path) => std::fs::write(path, &json)?,
        None => println!("{json}"),
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
