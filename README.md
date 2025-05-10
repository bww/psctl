[![GitHub Release](https://img.shields.io/github/v/release/bww/psctl)](https://github.com/bww/psctl/releases) [![Crates.io Version](https://img.shields.io/crates/v/psctl)](https://crates.io/crates/psctl)

# Process Control (or PSCTL, if you prefer)
PSCTL is a simple command-line process management tool. It runs processes as an interdependent graph.

You can think of _Process Control_ as being like [Docker Compose](https://docs.docker.com/compose/) but for commands instead of containers. Or like [Foreman](https://ddollar.github.io/foreman/) but with support for process dependencies and availability checks.

_Process Control relies on certain POSIX process management features. As such, it only works on Unix/Linux systems. It is tested on Linux and macOS._

## Installing
Install _Process Control_ by [downloading a release binary](https://github.com/bww/psctl/releases), or by using [Homebrew](https://brew.sh/) on macOS:

```
$ brew install bww/stable/psctl
```

If you have a Rust toolchain installed, you can also install from [crates.io](https://crates.io/crates/psctl):

```
$ cargo install psctl
```

## How to use this thing

Processes can have **availability checks** associated with them, which are used to determine when it has finished starting up and has become available. Processes can also describe which other processes are their **dependencies**. Using all this information, _Process Control_ will:

1. Build a graph and determine the order processes should be run,
2. Start each process in this order, in turn,
3. Wait for each process to become available, if availability checks are provided, and then
4. Wait forever for any process to exit.

Once any process exits, _Process Control_ terminates the other running processes and exits itself with the same exit code as the first exiting process. This makes it possible to propagate an error status from the managed process that exited abnormally.

### Example configuration
The following process configuration file (called a _taskfile_) is illustrative:

```yaml
version: 1
tasks:
  -
    # Process 'a' depends on 'b' and 'c', it is started after both 'a' and
    # 'b' are available.
    name: a
    run: sleep 3 && echo "A"
    deps:
      - b
      - c

  -
    # Process 'b' has no direct dependencies so it it started first.
    name: b
    run: sleep 10 && echo "B"
    # Availability checks are used to determine when a process has become
    # available. Once a process is available, processes that depend on it
    # will be started.
    checks:
      - shell:sleep 2
    # Wait up to 30 seconds for this process to become available. If all our
    # availability checks don't pass by this deadline, an error is produced.
    # The default duration to wait is 10 seconds.
    wait: 30s

  -
    # Process 'c' depends on 'b', it is started after process 'b' becomes
    # available.
    name: c
    run: sleep 10 && echo "C"
    checks:
      - https://hub.dummyapis.com/delay?seconds=2
    deps:
      - b

```

### Command line task definition
If you were so inclined, it is also possible to specify the tasks to manage on the command line, although this can quickly become difficult to read. For example, here is the same configuration as the above taskfile defines but specified on the command line:

```
$ psctl 'a+b,c: sleep 3 && echo "A"' \
        'b: sleep 10 && echo "B"'='shell:sleep 2' \
        'c+b: sleep 10 && echo "C"'='https://hub.dummyapis.com/delay?seconds=2'
```

For a reference of the command line task specification format, use `psctl -h`.

### Types of Availability Checks
Several types of availability checks are supported. They are described as a URL:

| Type            | Example                        | Description                                   |
|-----------------|--------------------------------|-----------------------------------------------|
| `http`, `https` | `http://localhost:8001/status` | Available when the service returns `2XX`      |
| `file`          | `file:///tmp/sock`             | Available when the file exists                |
| `shell`         | `shell:nc -z localhost 8001`   | Available when the command exits w/ status `0`|

### Running PSCTL
The example above can be run as follows:

```
$ psctl --file test/example.yaml
====> b, c, a
----> b: sleep 10 && echo "B" (https://hub.dummyapis.com/delay?seconds=2)
----> c: sleep 10 && echo "C" (https://hub.dummyapis.com/delay?seconds=2)
----> a: sleep 3 && echo "A" (https://hub.dummyapis.com/delay?seconds=2; https://hub.dummyapis.com/delay?seconds=3)
[ a ] A
====> finished
```

Notice that processes `b` and `c` are killed after `a` exits normally, so they never echo anything.
