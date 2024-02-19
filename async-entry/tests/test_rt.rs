#[cfg(feature = "monoio")]
mod test_monoio {

    use std::sync::{Arc, Mutex};

    #[async_entry::test]
    async fn test_monoio_spawn() {
        let m = Arc::new(Mutex::new(0u64));

        let m2 = m.clone();

        let join_handle = monoio::spawn(async move {
            let mut v = m2.lock().unwrap();
            *v = 5;
        });

        join_handle.await;

        let got = *(m.lock().unwrap());
        assert_eq!(got, 5);
    }
}
