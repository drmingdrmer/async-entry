#!/bin/sh

set -o errexit

echo "=== Build async-entry"
cargo expand -p async-entry --test it --color=never --tests > got
diff async-entry/want got

echo "=== Build test-async-entry: it-external"
cargo expand -p test-async-entry --test it-external --color=never --tests > got
diff test-async-entry/want-external got

echo "=== Build test-async-entry: it-tracing-lib"
cargo expand -p test-async-entry --test it-tracing-lib --color=never --tests > got
diff test-async-entry/want-tracing-lib got

echo "=== Compile"
cargo test -p async-entry
cargo test -p test-async-entry
