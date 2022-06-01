// Use the imported mod e.g. `tracing` to build.

use lib_crate::tokio;
use lib_crate::tracing;

#[async_entry::test]
async fn empty() {}

#[async_entry::test()]
async fn empty_with_parentheses() {}

#[async_entry::test]
async fn empty_tail_expr() -> anyhow::Result<()> {
    Ok(())
}

#[async_entry::test]
async fn empty_return_expr() -> anyhow::Result<()> {
    return Ok(());
}

#[async_entry::test(tracing_span = "info")]
async fn empty_trace_span() {}

#[async_entry::test(tracing_span = "info", tracing_lib = "::")]
async fn specify_trace_lib_to_root() {}

fn g() {}

#[async_entry::test(init = "g()")]
async fn with_init() {}

#[async_entry::test(flavor = "current_thread")]
async fn current_thread() {}

#[async_entry::test(flavor = "multi_thread")]
async fn multi_thread() {}

#[async_entry::test(worker_threads = 0)]
async fn multi_thread_0() {}

#[async_entry::test(flavor = "multi_thread", worker_threads = 10)]
async fn multi_thread_n() {}

#[async_entry::test(worker_threads = 10)]
async fn multi_thread_n_without_flavor() {}

#[async_entry::test(start_paused = true)]
async fn start_paused() {}
