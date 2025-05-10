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
  #[clap(long, help="Enable quiet mode, only required output is displayed")]
  pub quiet: bool,
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
    check    := <scheme>://<content>

READINESS CHECKS
    Readiness checks are expressed as a URL. The following types are supported:

    http(s)://...      The check passes when the URL returns 2XX
    file://...         The check passes when the file exists
    shell://<command>  The check passes when the command exits with status 0

EXAMPLE

    $ psctl 'a: echo A' 'b: echo B=file:///tmp/file' 'c +a,b: echo C'
")]
  pub specs: Vec<String>,
}

impl Options {
  pub fn debug(&self) -> bool {
    self.debug
  }

  pub fn verbose(&self) -> bool {
    self.debug || self.verbose
  }

  pub fn quiet(&self) -> bool {
    self.quiet && !self.verbose()
  }

  pub fn prefix(&self) -> bool {
    !self.quiet()
  }
}
