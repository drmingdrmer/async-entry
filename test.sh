#!/bin/sh

set -o errexit

test_rt() {
  local feat="$1"
  local rt="$2"
  local channel="$3"

  echo "=== Test runtime: $rt"

  echo "=== Build async-entry"
  cargo $channel expand --features "$feat" -p async-entry --test it --color=never --tests > got
  diff async-entry/want-$rt got

  echo "=== Build test-async-entry: it-external"
  cargo $channel expand --features "$feat" -p test-async-entry --test it-external --color=never --tests > got
  diff test-async-entry/want-external-$rt got

  echo "=== Build test-async-entry: it-tracing-lib"
  cargo $channel expand --features "$feat" -p test-async-entry --test it-tracing-lib --color=never --tests > got
  diff test-async-entry/want-tracing-lib-$rt got

  echo "=== Compile"
  cargo $channel test --features "$feat" -p async-entry
  cargo $channel test --features "$feat" -p test-async-entry

}

# by default it uses tokio
test_rt "" "tokio" "+nightly"

test_rt "tokio" "tokio" "+nightly"

# monoio requires nightly rust
test_rt "monoio" "monoio" "+nightly"

