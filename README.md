# async-entry

Entry macro for tokio based async test.

It does the same as `#[tokio::test]`, with two additional feature:

- Wrap a testing `fn` with a span with `tracing_span="<log_level>"`.

- Add an initializing statement with `init="<expr>"`.
  The value returned from the initializing expr will be held until test quit.
  Thus, it can be used to initialize logging or else.


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
//     use tracing::Instrument;
//     let body = body.instrument(tracing::info_span("foo");
//     let rt = tokio::runtime::Builder::new_current_thread()
//         .enable_all()
//         .build()
//         .expect("Failed building the Runtime");
//     #[allow(clippy::expect_used)]
//     rt.block_on(body);
// }
```

# Development

## Test

Run `test.sh` to check the expanded macro and compile them.