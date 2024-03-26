use anyhow::{anyhow, Result};
use clap::Parser;
use pydcstrngs::parse_file_contents;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    path: String,

    #[arg(long, default_value_t = false, alias = "nd")]
    /// Will consider an error for a docstring to be absent.
    forbid_no_docstring: bool,

    #[arg(long, default_value_t = false, alias = "na")]
    /// Will consider an error for an "Args" section to be absent.
    forbid_no_args_in_docstring: bool,

    #[arg(long, default_value_t = false, alias = "ud")]
    /// Will consider an error for an arg in docstring to be untyped.
    forbid_untyped_docstrings: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    let args = Args::parse();

    let contents = std::fs::read_to_string(args.path)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::language())?;

    let success = parse_file_contents(
        &mut parser,
        &contents,
        None,
        !args.forbid_no_docstring,
        !args.forbid_no_args_in_docstring,
        !args.forbid_untyped_docstrings,
    );

    if success {
        Ok(())
    } else {
        Err(anyhow!("found errors"))
    }
}
