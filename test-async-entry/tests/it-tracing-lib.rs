use lib_crate::tokio;

/// Specify where to load tracing and tracing_future.
/// Do not need to import
#[async_entry::test(tracing_span = "info", tracing_lib = "lib_crate")]
async fn specify_trace_lib() {}
