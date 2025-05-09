use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[clap(
  author, version,
  about = "Process Control is an operator for interdependent processes.
https://crates.io/crates/psctl",
  long_about = None,
  after_help = "EXAMPLES:

    $ psctl 'a: echo A' 'b: echo B=file:///tmp/file' 'c +a,b: echo C'"
)]
pub struct Options {
  #[clap(long, help="Enable debugging mode")]
  pub debug: bool,
  #[clap(long, help="Enable verbose output")]
  pub verbose: bool,
  #[clap(long, help="Load process specifiers from a taskfile")]
  pub file: Option<String>,
  #[clap(
    help_heading="SPECIFIERS",
    help="Task specifiers to run and manage. When a taskfile is provided, it is preferred over specifiers provided on the command line.

Any number of task specifiers may be provided as arguments. Each specifier has the following form:

    spec     := <label> [<deps>]: <command>[=<check>]
    labels   := <label1> [... <labelN>]
    label    := /[a-zA-Z0-9]+/
    deps     := '+' <labels>
    command  := /[^=]+/
    check    := a file:// or http(s):// url

EXAMPLE

    $ psctl 'a: echo A' 'b: echo B=file:///tmp/file' 'c +a,b: echo C'
")]
  pub specs: Vec<String>,
}
