//! Input source: crossterm directly, or fed from an mpsc channel (tests,
//! external editor pumping).

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crossterm::event::Event;

pub enum EventSource {
    Crossterm,
    BufferedCrossterm {
        stashed: VecDeque<Event>,
    },
    Channel {
        rx: Receiver<Event>,
        stashed: VecDeque<Event>,
    },
}

impl EventSource {
    /// Return unread batch events before input that arrived after the batch.
    pub(crate) fn prepend(&mut self, events: impl IntoIterator<Item = Event>) {
        if matches!(self, Self::Crossterm) {
            *self = Self::BufferedCrossterm {
                stashed: VecDeque::new(),
            };
        }
        match self {
            Self::Channel { stashed, .. } | Self::BufferedCrossterm { stashed } => {
                let mut pending: VecDeque<_> = events.into_iter().collect();
                pending.append(stashed);
                *stashed = pending;
            }
            Self::Crossterm => unreachable!(),
        }
    }

    pub fn channel(rx: Receiver<Event>) -> Self {
        Self::Channel {
            rx,
            stashed: VecDeque::new(),
        }
    }

    pub fn poll(&mut self, timeout: Duration) -> std::io::Result<bool> {
        match self {
            Self::Crossterm => crossterm::event::poll(timeout),
            Self::BufferedCrossterm { stashed } => {
                if !stashed.is_empty() {
                    Ok(true)
                } else {
                    crossterm::event::poll(timeout)
                }
            }
            Self::Channel { rx, stashed } => {
                while let Ok(ev) = rx.try_recv() {
                    stashed.push_back(ev);
                }
                if !stashed.is_empty() {
                    return Ok(true);
                }
                match rx.recv_timeout(timeout) {
                    Ok(ev) => {
                        stashed.push_back(ev);
                        Ok(true)
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(false),
                }
            }
        }
    }

    pub fn read(&mut self) -> std::io::Result<Event> {
        match self {
            Self::Crossterm => crossterm::event::read(),
            Self::BufferedCrossterm { stashed } => {
                stashed.pop_front().map_or_else(crossterm::event::read, Ok)
            }
            Self::Channel { rx, stashed } => {
                while let Ok(ev) = rx.try_recv() {
                    stashed.push_back(ev);
                }
                stashed.pop_front().ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::WouldBlock, "EventSource channel empty")
                })
            }
        }
    }
}
