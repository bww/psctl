pub mod proc;
pub mod util;
pub mod error;

use std::io;
use std::cmp::min;
use std::result;
use std::pin::Pin;

use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::sync::mpsc;
use tokio::process;
use futures::stream;
use futures::stream::TryStreamExt;
use futures::future::FutureExt;
use colored::Colorize;

use nix::unistd::Pid;
use nix::sys::signal;
use nix::sys::signal::Signal;

use crate::waiter;
use crate::config;
use crate::colorwheel;
use crate::runner::proc::Process;

type Result<T> = result::Result<T, error::Error>;

pub struct Pod {
  opts:  config::Options,
  procs: Vec<Process>,
  wheel: colorwheel::Wheel,
}

impl Pod {
  pub fn new(opts: config::Options, procs: Vec<Process>) -> Pod {
    Pod{
      opts: opts,
      procs: procs,
      wheel: colorwheel::Wheel::default(),
    }
  }

  pub fn overlay(&self, procs: Vec<Process>) -> Pod {
    Pod{
      opts: self.opts.clone(),
      procs: [self.procs.clone(), procs].concat(),
      wheel: self.wheel.clone(),
    }
  }

  pub async fn exec(&self, rx: &mut mpsc::Receiver<()>) -> Result<i32> {
    let ord: Vec<&Process> = proc::order_procs(self.procs.iter().map(|e| e).collect())?;
    let mut tset: Vec<(&Process, process::Command)> = Vec::new();
    let mut pset: Vec<(&Process, process::Child)> = Vec::new();
    for spec in &ord {
      tset.push((spec, spec.task()?));
    }

    // run processes
    let res = self._exec(&ord, &mut tset, &mut pset, rx).await;
    // explicitly clean up after processes
    Self::cleanup(&self.opts, &mut pset).await?;
    // return the result
    res
  }

  pub async fn _exec<'a>(&self, ord: &Vec<&Process>, tset: &mut Vec<(&'a Process, process::Command)>, pset: &mut Vec<(&'a Process, process::Child)>, rx: &mut mpsc::Receiver<()>) -> Result<i32> {
    if !self.opts.quiet() {
      eprintln!("{}", &format!("====> {}", ord.iter().map(|e| e.key()).collect::<Vec<&str>>().join(", ")).bold());
    }
    let maxkey: usize = min(32, tset.iter().map(|(spec, _)| spec.key().len()).max().unwrap_or(0));

    let mut i: usize = 0;
    for (spec, task) in tset {
      let mut proc = match task.spawn() {
        Ok(proc) => proc,
        Err(err) => return Err(error::ExecError::new(&format!("Could not run process: {}; because: {}", spec, err)).into()),
      };

      let mut stdout = match proc.stdout.take() {
        Some(stdout) => BufReader::new(stdout).lines(),
        None         => return Err(error::ExecError::new(&format!("Could not configure process STDOUT: {}", spec)).into()),
      };
      let mut stderr = match proc.stderr.take() {
        Some(stderr) => BufReader::new(stderr).lines(),
        None         => return Err(error::ExecError::new(&format!("Could not configure process STDERR: {}", spec)).into()),
      };

      let key_stdout = match self.opts.prefix() {
        true  => Some(format!("[ {} ]", self.wheel.colorize(i, spec.key_with_padding(maxkey)))),
        false => None,
      };
      tokio::spawn(async move {
        while let Some(line) = stdout.next_line().await.expect("Could not read from STDOUT") {
          if let Some(pfx) = &key_stdout {
            println!("{} {}", pfx, line);
          }
        }
      });

      let key_stderr = match self.opts.prefix() {
        true  => Some(format!("[ {} ]", self.wheel.colorize(i, spec.key_with_padding(maxkey)))),
        false => None,
      };
      tokio::spawn(async move {
        while let Some(line) = stderr.next_line().await.expect("Could not read from STDERR") {
          if let Some(pfx) = &key_stderr {
            println!("{} {}", pfx, line);
          }
        }
      });

      if !self.opts.quiet() {
        eprintln!("{}", &format!("----> {}", spec).bold());
      }
      let checks = spec.checks();
      let res = if !checks.is_empty(){
        let waitconf = waiter::Config::from_options(spec.key().to_owned(), &self.opts);
        tokio::select! {
          _   = rx.recv()   => Err(error::Error::CanceledError),
          _   = proc.wait() => Err(error::Error::NeverInitializedError(spec.key().to_owned())),
          res = waiter::wait_config(&waitconf, checks, spec.wait()) =>  match res {
            Ok(_)    => Ok((spec.key(), false)),
            Err(err) => Err(err.into()),
          }
        }
      } else {
        Ok((spec.key(), true)) // immediately available if we have no checks
      };
      pset.push((spec, proc));
      match res {
        Ok((key, dflt))  => if (!dflt && !self.opts.quiet()) || self.opts.verbose() {
          eprintln!("{}", &format!("----> {}: available", key).bold());
        },
        Err(err) => return Err(err),
      }

      i = i + 1;
    }

    let code = {
      let mut jobs: Vec<Pin<Box<dyn futures::Future<Output = Result<i32>>>>> = Vec::new();
      for (_, proc) in pset {
        jobs.push(Box::pin(proc.wait().map(|f| match f?.code() {
          Some(code) => Ok(code),
          None => Ok(0),
        })));
      }

      let mut jobs = stream::FuturesUnordered::from_iter(jobs);
      tokio::select! {
        _ = rx.recv() =>  return Err(error::Error::CanceledError),
        res = jobs.try_next() => (res?).unwrap_or_default(),
      }
    };

    if !self.opts.quiet() {
      eprintln!("{}", "====> finished".bold());
    }
    Ok(code)
  }

  async fn cleanup(opts: &config::Options, pset: &mut Vec<(&Process, process::Child)>) -> Result<()> {
    // explicitly clean up after remaining processes
    for (spec, proc) in pset {
      if let Some(pid) = proc.id() { // negative-pid addresses the process group
        if let Err(err) = signal::kill(Pid::from_raw(-(pid as i32)), Signal::SIGTERM) {
          eprintln!("{}", &format!("~~~~> {} [failed] {}", spec, err).bold());
          continue; // could not kill this one, it has possibly already exited; move on
        }
        match proc.wait().await {
          Ok(_) => {
            if !opts.quiet() {
              eprintln!("{}", &format!("~~~~> {} [{} killed]", spec, pid).bold());
            }
          },
          Err(err) => match err.kind() {
            io::ErrorKind::InvalidInput => {
              if !opts.quiet() {
                eprintln!("{}", &format!("~~~~> {} [{} ended]", spec, pid).bold());
              }
            },
            _ => return Err(error::Error::IOError(err)),
          },
        };
      }
    }
    Ok(())
  }
}
