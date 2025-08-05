//! Stream Infrastructure for Async Scanning
//! 
//! Provides buffered streaming, backpressure management, and progressive results.

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::Stream;
use futures::stream::BoxStream;
use pin_project::pin_project;
use crate::scanner::messages::ScanMessage;
use super::error::ScanResult;

/// Type alias for scan message streams
pub type ScanMessageStream = BoxStream<'static, ScanResult<ScanMessage>>;

/// Stream configuration for buffering and backpressure
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for stream items
    pub buffer_size: usize,
    /// Batch size for efficient processing
    pub batch_size: usize,
    /// Enable backpressure management
    pub backpressure_enabled: bool,
    /// Maximum memory usage for buffering (bytes)
    pub max_buffer_memory: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            batch_size: 50,
            backpressure_enabled: true,
            max_buffer_memory: 64 * 1024 * 1024, // 64MB
        }
    }
}

/// Buffered stream with backpressure management
#[pin_project]
pub struct BufferedStream<S> {
    #[pin]
    inner: S,
    buffer: Vec<ScanMessage>,
    config: StreamConfig,
    current_memory: usize,
    backpressure_active: bool,
}

impl<S> BufferedStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>>,
{
    /// Create a new buffered stream
    pub fn new(stream: S, config: StreamConfig) -> Self {
        Self {
            inner: stream,
            buffer: Vec::with_capacity(config.buffer_size),
            config,
            current_memory: 0,
            backpressure_active: false,
        }
    }
    
    /// Estimate memory usage of a message
    fn estimate_message_memory(msg: &ScanMessage) -> usize {
        // Basic estimation: header + data size
        std::mem::size_of::<ScanMessage>() + msg.estimate_memory_usage()
    }
}

impl<S> Stream for BufferedStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>>,
{
    type Item = ScanResult<Vec<ScanMessage>>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        
        // Check if we should emit current buffer due to backpressure
        if this.buffer.len() >= this.config.batch_size || *this.backpressure_active {
            if !this.buffer.is_empty() {
                let batch = std::mem::take(this.buffer);
                *this.current_memory = 0;
                *this.backpressure_active = false;
                return Poll::Ready(Some(Ok(batch)));
            }
        }
        
        // Try to fill buffer
        loop {
            match this.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(message))) => {
                    let msg_memory = Self::estimate_message_memory(&message);
                    *this.current_memory += msg_memory;
                    this.buffer.push(message);
                    
                    // Check for backpressure or batch completion
                    let should_apply_backpressure = this.config.backpressure_enabled && (
                        this.buffer.len() >= this.config.buffer_size ||
                        *this.current_memory >= this.config.max_buffer_memory
                    );
                    
                    if should_apply_backpressure || this.buffer.len() >= this.config.batch_size {
                        let batch = std::mem::take(this.buffer);
                        *this.current_memory = 0;
                        *this.backpressure_active = should_apply_backpressure;
                        return Poll::Ready(Some(Ok(batch)));
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(e)));
                }
                Poll::Ready(None) => {
                    // Stream ended, emit remaining buffer if any
                    if !this.buffer.is_empty() {
                        let batch = std::mem::take(this.buffer);
                        *this.current_memory = 0;
                        return Poll::Ready(Some(Ok(batch)));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    // If we have messages buffered and no more incoming, emit them
                    if !this.buffer.is_empty() && this.buffer.len() >= this.config.batch_size / 2 {
                        let batch = std::mem::take(this.buffer);
                        *this.current_memory = 0;
                        return Poll::Ready(Some(Ok(batch)));
                    }
                    return Poll::Pending;
                }
            }
        }
    }
}

/// Stream combinator for merging multiple scan streams
pub struct MergedScanStream {
    #[allow(dead_code)]
    streams: Vec<ScanMessageStream>,
    #[allow(dead_code)]
    active_streams: usize,
}

impl MergedScanStream {
    /// Create a new merged stream from multiple sources
    pub fn new(streams: Vec<ScanMessageStream>) -> Self {
        let active_streams = streams.len();
        Self {
            streams,
            active_streams,
        }
    }
    
    /// Merge streams with fair round-robin polling
    pub fn merge_fair(streams: Vec<ScanMessageStream>) -> impl Stream<Item = ScanResult<ScanMessage>> {
        futures::stream::select_all(streams)
    }
    
    /// Merge streams with priority ordering
    pub fn merge_priority(streams: Vec<(ScanMessageStream, u8)>) -> impl Stream<Item = ScanResult<ScanMessage>> {
        // Sort by priority (higher number = higher priority)
        let mut sorted_streams: Vec<_> = streams.into_iter().collect();
        sorted_streams.sort_by_key(|(_, priority)| std::cmp::Reverse(*priority));
        
        // Convert to stream
        let prioritized: Vec<_> = sorted_streams.into_iter().map(|(stream, _)| stream).collect();
        futures::stream::select_all(prioritized)
    }
}

/// Progress tracking for streaming operations
#[derive(Debug, Clone)]
pub struct StreamProgress {
    /// Total messages processed
    pub messages_processed: usize,
    /// Total bytes processed
    pub bytes_processed: usize,
    /// Current stream position (estimated)
    pub estimated_progress: f64,
    /// Start time of the operation
    pub start_time: std::time::Instant,
    /// Current throughput (messages/second)
    pub throughput: f64,
}

impl StreamProgress {
    /// Create new progress tracker
    pub fn new() -> Self {
        Self {
            messages_processed: 0,
            bytes_processed: 0,
            estimated_progress: 0.0,
            start_time: std::time::Instant::now(),
            throughput: 0.0,
        }
    }
    
    /// Update progress with new message
    pub fn update(&mut self, message: &ScanMessage) {
        self.messages_processed += 1;
        self.bytes_processed += message.estimate_memory_usage();
        
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.throughput = self.messages_processed as f64 / elapsed;
        }
    }
    
    /// Set estimated progress (0.0 to 1.0)
    pub fn set_estimated_progress(&mut self, progress: f64) {
        self.estimated_progress = progress.clamp(0.0, 1.0);
    }
    
    /// Get elapsed time
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
    
    /// Get estimated time remaining (if progress is available)
    pub fn estimated_remaining(&self) -> Option<std::time::Duration> {
        if self.estimated_progress > 0.0 && self.estimated_progress < 1.0 {
            let elapsed = self.elapsed().as_secs_f64();
            let total_estimated = elapsed / self.estimated_progress;
            let remaining = total_estimated - elapsed;
            Some(std::time::Duration::from_secs_f64(remaining.max(0.0)))
        } else {
            None
        }
    }
}

impl Default for StreamProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream wrapper that tracks progress
#[pin_project]
pub struct ProgressTrackingStream<S> {
    #[pin]
    inner: S,
    progress: StreamProgress,
    estimated_total: Option<usize>,
}

impl<S> ProgressTrackingStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>>,
{
    /// Create a new progress tracking stream
    pub fn new(stream: S, estimated_total: Option<usize>) -> Self {
        Self {
            inner: stream,
            progress: StreamProgress::new(),
            estimated_total,
        }
    }
    
    /// Get current progress
    pub fn progress(&self) -> &StreamProgress {
        &self.progress
    }
}

impl<S> Stream for ProgressTrackingStream<S>
where
    S: Stream<Item = ScanResult<ScanMessage>>,
{
    type Item = ScanResult<(ScanMessage, StreamProgress)>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        
        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(message))) => {
                this.progress.update(&message);
                
                // Update estimated progress if total is known
                if let Some(total) = this.estimated_total {
                    let progress_ratio = this.progress.messages_processed as f64 / *total as f64;
                    this.progress.set_estimated_progress(progress_ratio);
                }
                
                Poll::Ready(Some(Ok((message, this.progress.clone()))))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                this.progress.set_estimated_progress(1.0);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Utility functions for stream operations
pub mod stream_utils {
    use super::*;
    use tokio_stream::StreamExt;
    
    /// Convert any stream to a boxed stream
    pub fn boxed<S>(stream: S) -> ScanMessageStream
    where
        S: Stream<Item = ScanResult<ScanMessage>> + Send + 'static,
    {
        Box::pin(stream)
    }
    
    /// Create a buffered stream with default configuration
    pub fn buffered<S>(stream: S) -> BufferedStream<S>
    where
        S: Stream<Item = ScanResult<ScanMessage>>,
    {
        BufferedStream::new(stream, StreamConfig::default())
    }
    
    /// Create a progress tracking stream
    pub fn with_progress<S>(stream: S, estimated_total: Option<usize>) -> ProgressTrackingStream<S>
    where
        S: Stream<Item = ScanResult<ScanMessage>>,
    {
        ProgressTrackingStream::new(stream, estimated_total)
    }
    
    /// Filter stream by scan mode
    pub fn filter_by_mode<S>(stream: S, mode: crate::scanner::modes::ScanMode) -> impl Stream<Item = ScanResult<ScanMessage>>
    where
        S: Stream<Item = ScanResult<ScanMessage>>,
    {
        futures::StreamExt::filter_map(stream, move |result| {
            futures::future::ready(match result {
                Ok(message) if message.header.scan_mode.contains(mode) => Some(Ok(message)),
                Ok(_) => None, // Filter out messages not matching mode
                Err(e) => Some(Err(e)),
            })
        })
    }
    
    /// Transform stream with rate limiting
    pub fn rate_limited<S>(stream: S, max_per_second: f64) -> impl Stream<Item = ScanResult<ScanMessage>>
    where
        S: Stream<Item = ScanResult<ScanMessage>>,
    {
        
        let interval = std::time::Duration::from_secs_f64(1.0 / max_per_second);
        stream.throttle(interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use crate::scanner::modes::ScanMode;
    use crate::scanner::messages::{MessageHeader, MessageData};
    
    fn create_test_message(id: u64) -> ScanMessage {
        ScanMessage::new(
            MessageHeader::new(ScanMode::FILES, id),
            MessageData::FileInfo {
                path: format!("test_{}.rs", id),
                size: 1024,
                lines: 50,
            },
        )
    }
    
    #[tokio::test]
    async fn test_buffered_stream() {
        let messages: Vec<ScanResult<ScanMessage>> = (0..10)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let input_stream = stream::iter(messages);
        let config = StreamConfig {
            batch_size: 3,
            ..Default::default()
        };
        
        let mut buffered = BufferedStream::new(input_stream, config);
        
        let mut total_messages = 0;
        while let Some(batch_result) = buffered.next().await {
            let batch = batch_result.unwrap();
            assert!(batch.len() <= 3);
            total_messages += batch.len();
        }
        
        assert_eq!(total_messages, 10);
    }
    
    #[tokio::test]
    async fn test_progress_tracking() {
        let messages: Vec<ScanResult<ScanMessage>> = (0..5)
            .map(|i| Ok(create_test_message(i)))
            .collect();
        
        let input_stream = stream::iter(messages);
        let mut progress_stream = ProgressTrackingStream::new(input_stream, Some(5));
        
        let mut count = 0;
        while let Some(result) = progress_stream.next().await {
            let (_message, progress) = result.unwrap();
            count += 1;
            assert_eq!(progress.messages_processed, count);
            
            let expected_progress = count as f64 / 5.0;
            assert!((progress.estimated_progress - expected_progress).abs() < 0.01);
        }
        
        assert_eq!(count, 5);
    }
    
    #[tokio::test]
    async fn test_stream_merging() {
        // Create messages with explicit types to avoid closure type conflicts
        let messages1: Vec<_> = (0..3).map(|i| Ok(create_test_message(i))).collect();
        let messages2: Vec<_> = (10..13).map(|i| Ok(create_test_message(i))).collect();
        
        let stream1: ScanMessageStream = Box::pin(stream::iter(messages1));
        let stream2: ScanMessageStream = Box::pin(stream::iter(messages2));
        
        let streams = vec![stream1, stream2];
        let merged = MergedScanStream::merge_fair(streams);
        
        let results: Vec<_> = merged.collect().await;
        assert_eq!(results.len(), 6);
        
        // All results should be Ok
        for result in results {
            assert!(result.is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_stream_filtering() {
        let messages: Vec<ScanResult<ScanMessage>> = vec![
            Ok(ScanMessage::new(
                MessageHeader::new(ScanMode::FILES, 1),
                MessageData::FileInfo { path: "test.rs".to_string(), size: 100, lines: 10 }
            )),
            Ok(ScanMessage::new(
                MessageHeader::new(ScanMode::HISTORY, 2),
                MessageData::CommitInfo { 
                    hash: "abc123".to_string(), 
                    author: "test".to_string(), 
                    message: "test commit".to_string(), 
                    timestamp: 1234567890, 
                    changed_files: vec![crate::scanner::messages::FileChangeData {
                        path: "test.rs".to_string(),
                        lines_added: 5,
                        lines_removed: 2,
                    }] 
                }
            )),
            Ok(ScanMessage::new(
                MessageHeader::new(ScanMode::FILES, 3),
                MessageData::FileInfo { path: "test2.rs".to_string(), size: 200, lines: 20 }
            )),
        ];
        
        let input_stream = stream::iter(messages);
        let filtered = stream_utils::filter_by_mode(input_stream, ScanMode::FILES);
        
        let results: Vec<_> = filtered.collect().await;
        assert_eq!(results.len(), 2);
        
        for result in results {
            let message = result.unwrap();
            assert!(message.header.scan_mode.contains(ScanMode::FILES));
        }
    }
}