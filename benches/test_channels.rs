use bytes::Bytes;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use kanal;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use tokio::sync::mpsc as tokio_mpsc;

const MESSAGE_COUNT: usize = 1_000_000;
const CHANNEL_CAPACITY: usize = 100;
const NUM_OPERATORS: usize = 3;

#[derive(Debug, Clone)]
enum FlvTag {
    Audio { data: Bytes },
    Video { data: Bytes, keyframe: bool },
    Script { data: Bytes },
}

// **Transformations**
fn extract_keyframes(tag: FlvTag) -> Option<FlvTag> {
    match tag {
        FlvTag::Video { data, keyframe } if keyframe => Some(FlvTag::Video { data, keyframe }),
        _ => None, // Drop non-keyframes
    }
}

fn resample_audio(tag: FlvTag) -> FlvTag {
    if let FlvTag::Audio { data } = tag {
        let transformed_data = Bytes::copy_from_slice(&data[..]); // Simulate processing
        FlvTag::Audio {
            data: transformed_data,
        }
    } else {
        tag
    }
}

fn buffer_script_tags(tag: FlvTag, buffer: &mut Vec<FlvTag>) -> Option<Vec<FlvTag>> {
    if let FlvTag::Script { ref data } = tag {
        buffer.push(tag.clone());
        if &data[..] == b"metadata complete" {
            return Some(buffer.drain(..).collect());
        }
    }
    None
}

// **1. Sync Processing**
fn sync_processing() {
    let mut buffer = Vec::new();
    for i in 0..MESSAGE_COUNT {
        buffer.push(FlvTag::Audio {
            data: Bytes::from(format!("{}", i)),
        });
    }
    let mut script_buffer = Vec::new();
    for tag in buffer.iter() {
        if let Some(tag) = extract_keyframes(tag.clone()) {
            black_box(tag);
        }
        black_box(resample_audio(tag.clone()));
        if let Some(tags) = buffer_script_tags(tag.clone(), &mut script_buffer) {
            for t in tags {
                black_box(t);
            }
        }
    }
}

// **2. Shared Arc<Mutex<VecDeque>>**
fn shared_mutex_queue() {
    let queue = Arc::new(Mutex::new(VecDeque::with_capacity(MESSAGE_COUNT)));
    let done = Arc::new(Mutex::new(false));
    let done_clone = Arc::clone(&done);

    let queue_clone = Arc::clone(&queue);
    let producer = thread::spawn(move || {
        let mut q = queue_clone.lock().unwrap();
        for i in 0..MESSAGE_COUNT {
            q.push_back(FlvTag::Audio {
                data: Bytes::from(format!("{}", i)),
            });
        }
        drop(q); // Release the lock

        // Signal we're done
        let mut done_flag = done_clone.lock().unwrap();
        *done_flag = true;
    });

    let queue_clone = Arc::clone(&queue);
    let consumer = thread::spawn(move || {
        let mut script_buffer = Vec::new();
        loop {
            let mut processed = 0;
            {
                let mut q = queue_clone.lock().unwrap();
                while let Some(tag) = q.pop_front() {
                    processed += 1;
                    if let Some(tag) = extract_keyframes(tag.clone()) {
                        black_box(tag);
                    }
                    black_box(resample_audio(tag.clone()));
                    if let Some(tags) = buffer_script_tags(tag, &mut script_buffer) {
                        for t in tags {
                            black_box(t);
                        }
                    }
                }
            } // Release the lock

            // Check if we're done and the queue is empty
            let done_flag = *done.lock().unwrap();
            if done_flag && processed == 0 {
                break;
            }

            // Small sleep to avoid tight spinning
            if processed == 0 {
                thread::yield_now();
            }
        }
    });

    producer.join().unwrap();
    consumer.join().unwrap();
}

// **3. Std MPSC with Operators**
fn std_mpsc_pipeline() {
    let (tx, rx) = mpsc::channel();

    // Last stage receiver
    let (final_tx, final_rx) = mpsc::channel();

    let producer = thread::spawn(move || {
        for i in 0..MESSAGE_COUNT {
            tx.send(FlvTag::Audio {
                data: Bytes::from(format!("{}", i)),
            })
            .unwrap();
        }
    });

    let mut handles = vec![];
    let mut rx_chain = rx;

    for i in 0..NUM_OPERATORS {
        let (next_tx, next_rx) = if i == NUM_OPERATORS - 1 {
            (final_tx.clone(), mpsc::channel().1) // Last operator sends to final, using dummy receiver
        } else {
            mpsc::channel()
        };

        let handle = thread::spawn(move || {
            let mut script_buffer = Vec::new();
            while let Ok(tag) = rx_chain.recv() {
                let mut transformed = tag.clone();
                if let Some(tag) = extract_keyframes(tag) {
                    transformed = tag;
                }
                transformed = resample_audio(transformed);
                if let Some(tags) = buffer_script_tags(transformed.clone(), &mut script_buffer) {
                    for t in tags {
                        black_box(t);
                    }
                }
                next_tx.send(transformed).unwrap();
            }
        });
        rx_chain = next_rx;
        handles.push(handle);
    }

    drop(final_tx); // Drop the cloned sender

    // Consumer thread for final output
    let consumer = thread::spawn(move || {
        while let Ok(tag) = final_rx.recv() {
            black_box(tag);
        }
    });

    producer.join().unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    consumer.join().unwrap();
}

// **4. Tokio MPSC**
fn tokio_mpsc_pipeline(rt :&Runtime) {
    rt.block_on(async {
        let (tx, mut rx) = tokio_mpsc::channel(CHANNEL_CAPACITY);
        let (final_tx, mut final_rx) = tokio_mpsc::channel(CHANNEL_CAPACITY);

        let producer = tokio::spawn(async move {
            for i in 0..MESSAGE_COUNT {
                tx.send(FlvTag::Audio {
                    data: Bytes::from(format!("{}", i)),
                })
                .await
                .unwrap();
            }
        });

        let mut handles = vec![];

        for i in 0..NUM_OPERATORS {
            let (next_tx, next_rx) = if i == NUM_OPERATORS - 1 {
                (
                    final_tx.clone(),
                    tokio_mpsc::channel::<FlvTag>(CHANNEL_CAPACITY).1,
                ) // Dummy, won't be used
            } else {
                tokio_mpsc::channel(CHANNEL_CAPACITY)
            };

            let mut current_rx = std::mem::replace(&mut rx, next_rx);

            let handle = tokio::spawn(async move {
                let mut script_buffer = Vec::new();
                while let Some(tag) = current_rx.recv().await {
                    let mut transformed = tag.clone();
                    if let Some(tag) = extract_keyframes(tag) {
                        transformed = tag;
                    }
                    transformed = resample_audio(transformed);
                    if let Some(tags) = buffer_script_tags(transformed.clone(), &mut script_buffer)
                    {
                        for t in tags {
                            black_box(t);
                        }
                    }
                    next_tx.send(transformed).await.unwrap();
                }
            });
            handles.push(handle);
        }

        drop(final_tx); // Drop the cloned sender

        // Consumer for final output
        let consumer = tokio::spawn(async move {
            while let Some(tag) = final_rx.recv().await {
                black_box(tag);
            }
        });

        producer.await.unwrap();
        for handle in handles {
            handle.await.unwrap();
        }
        consumer.await.unwrap();
    });
}

// **5. Kanal Pipeline**
fn kanal_pipeline() {
    let (tx, rx) = kanal::bounded(CHANNEL_CAPACITY);
    let (final_tx, final_rx) = kanal::bounded(CHANNEL_CAPACITY);

    let producer = thread::spawn(move || {
        for i in 0..MESSAGE_COUNT {
            tx.send(FlvTag::Audio {
                data: Bytes::from(format!("{}", i)),
            })
            .unwrap();
        }
    });

    let mut handles = vec![];
    let mut current_rx = rx;

    for i in 0..NUM_OPERATORS {
        let (next_tx, next_rx) = if i == NUM_OPERATORS - 1 {
            (final_tx.clone(), kanal::bounded::<FlvTag>(1).1) // Dummy, won't be used
        } else {
            kanal::bounded(CHANNEL_CAPACITY)
        };

        let rx_for_thread = current_rx;
        current_rx = next_rx; // Update for next iteration

        let handle = thread::spawn(move || {
            let mut script_buffer = Vec::new();
            while let Ok(tag) = rx_for_thread.recv() {
                let mut transformed = tag.clone();
                if let Some(tag) = extract_keyframes(tag) {
                    transformed = tag;
                }
                transformed = resample_audio(transformed);
                if let Some(tags) = buffer_script_tags(transformed.clone(), &mut script_buffer) {
                    for t in tags {
                        black_box(t);
                    }
                }
                next_tx.send(transformed).unwrap();
            }
        });
        handles.push(handle);
    }

    drop(final_tx); // Drop the cloned sender

    // Consumer for final output
    let consumer = thread::spawn(move || {
        while let Ok(tag) = final_rx.recv() {
            black_box(tag);
        }
    });

    producer.join().unwrap();
    for handle in handles {
        handle.join().unwrap();
    }
    consumer.join().unwrap();
}

fn benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("flv_processing");
    group.bench_function("sync", |b| b.iter(|| sync_processing()));
    group.bench_function("mutex_queue", |b| b.iter(|| shared_mutex_queue()));
    group.bench_function("std_mpsc", |b| b.iter(|| std_mpsc_pipeline()));
    group.bench_function("tokio_mpsc", |b| b.iter(|| tokio_mpsc_pipeline(&rt)));
    group.bench_function("kanal", |b| b.iter(|| kanal_pipeline()));
    group.finish();
}

criterion_group!(benches, benchmark);
criterion_main!(benches);
