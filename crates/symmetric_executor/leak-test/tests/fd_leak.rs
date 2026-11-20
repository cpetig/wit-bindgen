//! A task waiting for several events registers all of them again at every wake-up. The registration
//! of an event that did not fire has to be reused, not repeated: every repetition keeps an eventfd
//! (and the task) alive in the symmetric executor, e.g. for each sample of an event stream while
//! the task also waits for something which never happens (a shutdown request).

use std::alloc::Layout;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use wit_bindgen_symmetric_rt::async_support::{
    block_on, spawn, stream_new, wait_on, StreamReader, StreamVtable,
};
use wit_bindgen_symmetric_rt::EventGenerator;

static VTABLE: StreamVtable<u64> = StreamVtable {
    layout: Layout::new::<u64>(),
    lower: None,
    lift: None,
};

fn open_fds() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

fn record(value: u64, fds: &Mutex<Vec<usize>>) {
    let mut fds = fds.lock().unwrap();
    assert_eq!(value as usize, fds.len());
    fds.push(open_fds());
}

async fn receive(mut reader: StreamReader<u64>, fds: Arc<Mutex<Vec<usize>>>) {
    while let Some(value) = reader.next().await {
        record(value, &fds);
    }
}

async fn receive_until_shutdown(
    mut reader: StreamReader<u64>,
    fds: Arc<Mutex<Vec<usize>>>,
    shutdown: EventGenerator,
) {
    // from the first wake-up on the task runs inside the executor
    record(reader.next().await.unwrap(), &fds);
    let shutdown_wait = wait_on(shutdown.subscribe());
    let receive = receive(reader, fds);
    futures::pin_mut!(shutdown_wait, receive);
    futures::future::select(shutdown_wait, receive).await;
    // let the executor release the registration for the shutdown, to end run()
    shutdown.activate();
}

#[test]
fn stream_reads_keep_fd_count_constant() {
    const ITEMS: usize = 50;
    let (mut writer, reader) = stream_new(&VTABLE);
    let fds = Arc::new(Mutex::new(Vec::new()));
    // not activated until the stream ended
    let shutdown = EventGenerator::new();
    spawn(receive_until_shutdown(reader, Arc::clone(&fds), shutdown));
    let producer = std::thread::spawn(move || {
        block_on(async move {
            for i in 0..ITEMS {
                // the samples of an event come in at a slow pace
                std::thread::sleep(Duration::from_millis(5));
                writer.write(vec![i as u64]).await;
            }
        });
    });
    wit_bindgen_symmetric_rt::run();
    producer.join().unwrap();
    let fds = fds.lock().unwrap();
    assert_eq!(fds.len(), ITEMS);
    println!("open fds: {fds:?}");
    assert!(
        fds[ITEMS - 1] <= fds[4],
        "fd count grows from {} to {}",
        fds[4],
        fds[ITEMS - 1]
    );
}
