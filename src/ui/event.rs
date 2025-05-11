//! Event handling module for the application
//!
//! This module provides a mechanism to handle events asynchronously
//! from the main application thread. It uses a channel to send and receive
//! events between the main application and a separate event handling thread.
//!
//! The `EventHandler` struct manages the event handling thread and provides
//! a way to receive events through a channel.
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
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

/// Handles terminal events asynchronously
///
/// This struct manages event handling in a separate thread and provides
/// a way to receive events through a channel.
pub struct EventHandler
{
    /// Sender side of the event channel, kept for ownership
    #[allow(dead_code)]
    sender: mpsc::Sender<Event>,
    /// Receiver side of the event channel to get events from the handler thread
    receiver: mpsc::Receiver<Event>,
    /// Thread handle for the event polling thread
    #[allow(dead_code)]
    handler: thread::JoinHandle<()>,
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
        let (sender, receiver) = mpsc::channel();

        // Spawn a thread that continuously polls for terminal events
        let handler = {
            let sender = sender.clone();
            thread::spawn(move || {
                let mut last_tick = Instant::now();

                loop
                {
                    // Calculate how long to wait before the next tick
                    // If more time than tick_rate has passed, don't wait at all
                    let timeout = tick_rate
                        .checked_sub(last_tick.elapsed())
                        .unwrap_or_else(|| Duration::from_secs(0));

                    // Poll for crossterm events, with timeout to ensure we generate tick events
                    if event::poll(timeout).expect("Error polling events")
                    {
                        match event::read().expect("Error reading event")
                        {
                            // Handle keyboard input
                            CrosstermEvent::Key(key) =>
                            {
                                // Break the loop if sending fails (receiver dropped)
                                if sender.send(Event::Key(key)).is_err()
                                {
                                    break;
                                }
                            }
                            // Handle terminal resize events
                            CrosstermEvent::Resize(width, height) =>
                            {
                                if sender
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
                        if sender.send(Event::Tick).is_err()
                        {
                            break;
                        }
                        last_tick = Instant::now();
                    }
                }
            })
        };

        Self {
            sender,
            receiver,
            handler,
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
        Ok(self.receiver.recv()?)
    }
}
