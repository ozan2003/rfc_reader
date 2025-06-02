//! Event handling module for the application
//!
//! This module provides a mechanism to handle events
//! from the main application thread. It uses a channel to send and receive
//! events between the main application and a separate event handling thread.
//!
//! The `EventHandler` struct manages the event handling thread and provides
//! a way to receive events through a channel.
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};

/// Events that can be processed by the application
#[derive(Debug, Clone, Copy)]
pub enum Event
{
    /// Regular time tick for updating UI elements
    Tick,
    /// Keyboard input event
    Key(KeyEvent),
    /// Terminal resize event with new dimensions
    Resize(u16, u16),
}

/// Handles terminal events
///
/// Manages event handling in a separate thread and provides
/// a way to receive events through a channel.
pub struct EventHandler
{
    /// Receiver side of the event channel to get events from the handler thread
    event_receiver: mpsc::Receiver<Event>,
    /// Sender for shutdown the thread for graceful shutdown
    // The receiver is moved to the thread
    shutdown_sender: mpsc::Sender<()>,
    /// Handle to keep the thread alive
    // Option is used to move the handle in `drop`
    // since we can't move the handle out of the `&mut self`
    // for calling `join` in `drop`
    thread_handle: Option<JoinHandle<()>>,
}

impl EventHandler
{
    /// Creates a new event handler with the specified tick rate
    ///
    /// # Arguments
    ///
    /// * `tick_rate` - The duration between tick events
    ///
    /// # Returns
    ///
    /// A new `EventHandler` instance with a running background thread
    ///
    /// # Panics
    ///
    /// Panics if the channel is disconnected.
    #[must_use]
    pub fn new(tick_rate: Duration) -> Self
    {
        // Create a channel for sending events from the thread to the main application
        let (event_sender, event_receiver) = mpsc::channel();
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();

        // Spawn a thread that continuously polls for terminal events
        // Move the `shutdown_receiver` to the thread.
        let handle = thread::spawn(move || {
            let mut last_tick = Instant::now();

            loop
            {
                // Check for shutdown signal from the `shutdown_receiver`
                if shutdown_receiver.try_recv().is_ok()
                {
                    break;
                }

                // Calculate how long to wait before the next tick
                // If more time than tick_rate has passed, don't wait at all
                let timeout = tick_rate.saturating_sub(last_tick.elapsed());

                // Poll for crossterm events, with timeout to ensure we generate tick events
                if event::poll(timeout).expect("Error polling events")
                {
                    match event::read().expect("Error reading event")
                    {
                        // Handle keyboard input
                        CrosstermEvent::Key(key) =>
                        {
                            // Break the loop if sending fails (receiver dropped)
                            if event_sender.send(Event::Key(key)).is_err()
                            {
                                break;
                            }
                        }
                        // Handle terminal resize events
                        CrosstermEvent::Resize(width, height) =>
                        {
                            if event_sender
                                .send(Event::Resize(width, height))
                                .is_err()
                            {
                                break;
                            }
                        }
                        // Ignore other event types
                        _ =>
                        {}
                    }
                }

                // Generate tick events for animations and regular updates
                if last_tick.elapsed() >= tick_rate
                {
                    if event_sender.send(Event::Tick).is_err()
                    {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self {
            event_receiver,
            shutdown_sender,
            thread_handle: Some(handle),
        }
    }

    /// Gets the next event from the event channel
    ///
    /// This method blocks until an event is available
    ///
    /// # Returns
    ///
    /// The next event, or an error if the channel is disconnected
    ///
    /// # Errors
    ///
    /// Returns an error if the channel is disconnected.
    pub fn next(&self) -> Result<Event>
    {
        self.event_receiver
            .recv()
            .context("Event channel disconnected")
    }
}

impl Drop for EventHandler
{
    fn drop(&mut self)
    {
        // Signal shutdown (ignore if already closed)
        // Don't panic, so assign the entire result.
        let _ = self.shutdown_sender.send(());

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take()
        {
            let _ = handle.join();
        }
    }
}
