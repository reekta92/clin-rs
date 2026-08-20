//! Input source: crossterm directly, or fed from an mpsc channel (tests,
//! external editor pumping).

use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crossterm::event::Event;

pub enum EventSource {
    Crossterm,
    Channel {
        rx: Receiver<Event>,
        stashed: VecDeque<Event>,
    },
}

impl EventSource {
    pub fn channel(rx: Receiver<Event>) -> Self {
        Self::Channel {
            rx,
            stashed: VecDeque::new(),
        }
    }

    pub fn poll(&mut self, timeout: Duration) -> std::io::Result<bool> {
        match self {
            Self::Crossterm => crossterm::event::poll(timeout),
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
