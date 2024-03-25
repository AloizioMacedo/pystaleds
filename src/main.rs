use anyhow::{anyhow, Result};
use clap::Parser;
use pydcstrngs::parse_file_contents;

#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    path: String,

    #[arg(long, default_value_t = false)]
    allow_no_docstring: bool,

    #[arg(long, default_value_t = false)]
    allow_no_arg_in_docstring: bool,

    #[arg(long, default_value_t = false)]
    allow_untyped_docstrings: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let contents = std::fs::read_to_string(args.path)?;

    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_python::language())?;

    let error_locations = parse_file_contents(
        &mut parser,
        &contents,
        None,
        args.allow_no_docstring,
        args.allow_no_arg_in_docstring,
        !args.allow_untyped_docstrings,
    );

    if error_locations.is_empty() {
        Ok(())
    } else {
        for error_location in error_locations {
            eprintln!("{:?}", error_location);
        }

        Err(anyhow!("found errors"))
    }
}
