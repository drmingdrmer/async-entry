# async-entry
entry macro for tokio based async test.


```rust
#[async_entry::test(init="g()", tracing_span="info")]
async fn foo() {
    assert!(true);
}
// Will build a test fn:
// #[test]
// fn foo() {
//     let _g = g();
//     let body = async {
//         assert!(true);
//     };
//     use tracing_futures::Instrument;
//     let body = body.instrument(tracing::info_span("foo");
//     let rt = tokio::runtime::Builder::new_current_thread()
//         .enable_all()
//         .build()
//         .expect("Failed building the Runtime");
//     #[allow(clippy::expect_used)]
//     rt.block_on(body);
// }

```
