## About qosmap

The goal of qosmap is to analyze the traffic rate configured by service
providers like residential ISPs. It measures the effective bandwidth and
derives the per packet overhead.
This information can be used to set up a precise upstream QoS on your local
router, wasting as little bandwidth as possible.


## Usage
```
USAGE:
    qosmap [FLAGS] [OPTIONS] <host>

FLAGS:
    -h, --help       Prints help information
    -s, --server     server mode
    -V, --version    Prints version information

OPTIONS:
    -d, --duration <duration>    duration of the test in seconds [default: 1]
    -p, --port <port>            server port [default: 4801]
    -r, --rate <rate>            packet rate in packets per second [default: 1000]

ARGS:
    <host>    server address
```
