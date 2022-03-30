#!/bin/sh

set -o errexit
cargo expand --test it --color=never --tests > got
diff want got
cargo test
