# Process Control
This is a simple process management tool. It runs processes as an interdependent graph.

You can think of Process Control as being like [Docker Compose](https://docs.docker.com/compose/) but for commands instead of containers. Or like [Foreman](https://ddollar.github.io/foreman/) but with support for process dependencies and availability checks.

<hr>

Processes can have **availability checks** associated with them, which are used to determine when it has finished starting up and has become available. Processes can also describe which other processes are their **dependencies**. Using all this information, Process Control will:

1. Determine the order processes should be run,
2. Execute each process in this order, in turn,
3. Wait for each process to become available, if availability checks are provided, and then
4. Wait forever for any process to exit.

Once any process exits, Process Control kills the other running processes and exits with the same exit code as the first exiting process.

The following process configuration file is illustrative:

```yaml
version: 1
tasks:
  -
    name: a
    run: sleep 3 && echo "A"
    checks:
      - https://hub.dummyapis.com/delay?seconds=2
      - https://hub.dummyapis.com/delay?seconds=3
    deps:
      - b
      - c

  -
    name: b
    run: sleep 10 && echo "B"
    checks:
      - https://hub.dummyapis.com/delay?seconds=2

  -
    name: c
    run: sleep 10 && echo "C"
    checks:
      - https://hub.dummyapis.com/delay?seconds=2
    deps:
      - b

```

It can be run as follows:

```
$ psctl --file test/example.yaml
====> b, c, a
----> b: sleep 10 && echo "B" (https://hub.dummyapis.com/delay?seconds=2)
----> c: sleep 10 && echo "C" (https://hub.dummyapis.com/delay?seconds=2)
----> a: sleep 3 && echo "A" (https://hub.dummyapis.com/delay?seconds=2; https://hub.dummyapis.com/delay?seconds=3)
A
====> finished
```

Notice that processes `b` and `c` are killed after `a` exits normally, so they never echo anything.
