use std::time::{Duration, Instant};

/// Shown in the properties panel between the node's title and its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMessageSeverity {
    Error,
    Warning,
    Info
}

#[derive(Debug, Clone)]
pub struct NodeMessage {
    pub severity: NodeMessageSeverity,
    pub text: String
}
impl NodeMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            severity: NodeMessageSeverity::Error,
            text: text.into()
        }
    }
    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            severity: NodeMessageSeverity::Warning,
            text: text.into()
        }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            severity: NodeMessageSeverity::Info,
            text: text.into()
        }
    }
}

/// `Persistent` messages stay until superseded by the next attempt on the same node.
#[derive(Debug, Clone, Copy)]
pub enum MessageLifetime {
    Persistent,
    Timed(Duration)
}

/// Paired with when it was raised, so a timed message can be told apart from an expired one.
#[derive(Debug, Clone)]
pub struct TimedNodeMessage {
    pub message: NodeMessage,
    raised_at: Instant,
    lifetime: MessageLifetime
}
impl TimedNodeMessage {
    pub fn new(message: NodeMessage, lifetime: MessageLifetime) -> Self {
        Self {
            message,
            raised_at: Instant::now(),
            lifetime
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.lifetime {
            MessageLifetime::Persistent => false,
            MessageLifetime::Timed(duration) => self.raised_at.elapsed() >= duration
        }
    }

    /// `None` if it never expires (already expired, or persistent). Used to schedule a repaint so the UI updates the moment it should disappear.
    pub fn remaining(&self) -> Option<Duration> {
        match self.lifetime {
            MessageLifetime::Persistent => None,
            MessageLifetime::Timed(duration) => duration.checked_sub(self.raised_at.elapsed())
        }
    }
}
