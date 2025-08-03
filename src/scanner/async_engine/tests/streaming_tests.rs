//! Streaming and Producer Integration Tests

use crate::scanner::async_engine::*;
use crate::scanner::modes::ScanMode;
use crate::scanner::messages::{ScanMessage, MessageHeader, MessageData};
use crate::queue::MemoryQueue;
use futures::stream;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;

fn create_test_message(id: u64, mode: ScanMode) -> ScanMessage {
    ScanMessage::new(
        MessageHeader::new(mode, id),
        match mode {
            ScanMode::FILES => MessageData::FileInfo {
                path: format!("file_{}.rs", id),
                size: 1024 * (id as usize + 1),
                lines: 50 * (id as usize + 1),
            },
            ScanMode::HISTORY => MessageData::CommitInfo {
                hash: format!("commit_{:06x}", id),
                author: format!("author_{}", id % 3),
                message: format!("Commit message {}", id),
            },
            _ => MessageData::FileInfo {
                path: format!("generic_{}.txt", id),
                size: 512,
                lines: 25,
            },
        },
    )
}

#[tokio::test]
async fn test_buffered_stream_basic() {
    let messages: Vec<ScanResult<ScanMessage>> = (0..10)
        .map(|i| Ok(create_test_message(i, ScanMode::FILES)))
        .collect();
    
    let input_stream = stream::iter(messages);
    let config = StreamConfig {
        buffer_size: 5,
        batch_size: 3,
        ..Default::default()
    };
    
    let buffered = BufferedStream::new(input_stream, config);
    let batches: Vec<_> = buffered.collect().await;
    
    // Should get batches, with total messages = 10
    let total_messages: usize = batches.iter()
        .map(|batch_result| batch_result.as_ref().unwrap().len())
        .sum();
    
    assert_eq!(total_messages, 10);
    
    // Each batch should be ≤ batch_size
    for batch_result in batches {
        let batch = batch_result.unwrap();
        assert!(batch.len() <= 3);
    }
}

#[tokio::test]
async fn test_progress_tracking_stream() {
    let messages: Vec<ScanResult<ScanMessage>> = (0..20)
        .map(|i| Ok(create_test_message(i, ScanMode::FILES)))
        .collect();
    
    let input_stream = stream::iter(messages);
    let mut progress_stream = ProgressTrackingStream::new(input_stream, Some(20));
    
    let mut last_progress = 0.0;
    let mut message_count = 0;
    
    while let Some(result) = progress_stream.next().await {
        let (_message, progress) = result.unwrap();
        message_count += 1;
        
        // Progress should be monotonically increasing
        assert!(progress.estimated_progress >= last_progress);
        assert_eq!(progress.messages_processed, message_count);
        
        last_progress = progress.estimated_progress;
    }
    
    assert_eq!(message_count, 20);
    assert!((last_progress - 1.0).abs() < 0.01); // Should end at ~100%
}

#[tokio::test]
async fn test_stream_merging() {
    // Create individual streams with explicit types
    let files_messages: Vec<_> = (0..5).map(|i| Ok(create_test_message(i, ScanMode::FILES))).collect();
    let history_messages: Vec<_> = (10..15).map(|i| Ok(create_test_message(i, ScanMode::HISTORY))).collect();
    
    let stream1: ScanMessageStream = Box::pin(stream::iter(files_messages));
    let stream2: ScanMessageStream = Box::pin(stream::iter(history_messages));
    
    let streams = vec![stream1, stream2];
    
    let merged = MergedScanStream::merge_fair(streams);
    let results: Vec<_> = merged.collect().await;
    
    assert_eq!(results.len(), 10);
    
    // Check that we got messages from both modes
    let file_count = results.iter().filter(|r| {
        matches!(r, Ok(msg) if msg.header.scan_mode == ScanMode::FILES)
    }).count();
    let history_count = results.iter().filter(|r| {
        matches!(r, Ok(msg) if msg.header.scan_mode == ScanMode::HISTORY)
    }).count();
    
    assert_eq!(file_count, 5);
    assert_eq!(history_count, 5);
}

#[tokio::test]
async fn test_streaming_queue_producer() {
    let queue = Arc::new(MemoryQueue::new(100, 1024 * 1024));
    let producer = StreamingQueueProducer::with_defaults(
        Arc::clone(&queue),
        "TestProducer".to_string(),
    ).unwrap();
    
    let messages: Vec<ScanResult<ScanMessage>> = (0..15)
        .map(|i| Ok(create_test_message(i, ScanMode::FILES)))
        .collect();
    
    let test_stream = stream::iter(messages);
    let stats = producer.process_stream(test_stream).await.unwrap();
    
    assert_eq!(stats.messages_produced, 15);
    assert!(stats.batches_sent > 0);
    assert!(stats.current_throughput > 0.0);
    
    // Give background task time to process
    producer.flush().await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check that messages made it to the queue
    assert!(queue.size() > 0);
}

#[tokio::test]
async fn test_stream_to_queue_adapter() {
    let queue = Arc::new(MemoryQueue::new(100, 1024 * 1024));
    let adapter = StreamToQueueAdapter::new(
        Arc::clone(&queue),
        "AdapterTest".to_string(),
    ).unwrap();
    
    let messages: Vec<ScanResult<ScanMessage>> = (0..8)
        .map(|i| Ok(create_test_message(i, ScanMode::HISTORY)))
        .collect();
    
    let test_stream = stream::iter(messages);
    let stats = adapter.process_stream(test_stream).await.unwrap();
    
    assert_eq!(stats.messages_produced, 8);
    assert!(stats.elapsed().as_millis() > 0);
}