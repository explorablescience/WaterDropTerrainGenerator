use std::time::{Duration, Instant};

/// Severity of a message a [`Node`](crate::core::node::Node) reports about its own state, shown
/// in the properties panel between the node's title and its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeMessageSeverity {
    Error,
    Warning,
    Info
}

/// A single error, warning, or informational message associated with a node.
#[derive(Debug, Clone)]
pub struct NodeMessage {
    pub severity: NodeMessageSeverity,
    pub text: String
}
impl NodeMessage {
    pub fn error(text: impl Into<String>) -> Self {
        Self { severity: NodeMessageSeverity::Error, text: text.into() }
    }
    pub fn warning(text: impl Into<String>) -> Self {
        Self { severity: NodeMessageSeverity::Warning, text: text.into() }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Self { severity: NodeMessageSeverity::Info, text: text.into() }
    }
}

/// How long a [`NodeMessage`] stays visible before it is dropped. Errors are `Persistent` - they
/// stay until superseded by the next attempt on the same node - while a success confirmation
/// times out on its own after a few seconds.
#[derive(Debug, Clone, Copy)]
pub enum MessageLifetime {
    Persistent,
    Timed(Duration)
}

/// A [`NodeMessage`] paired with when it was raised, so a timed one can be told apart from an
/// expired one.
#[derive(Debug, Clone)]
pub struct TimedNodeMessage {
    pub message: NodeMessage,
    raised_at: Instant,
    lifetime: MessageLifetime
}
impl TimedNodeMessage {
    pub fn new(message: NodeMessage, lifetime: MessageLifetime) -> Self {
        Self { message, raised_at: Instant::now(), lifetime }
    }

    pub fn is_expired(&self) -> bool {
        match self.lifetime {
            MessageLifetime::Persistent => false,
            MessageLifetime::Timed(duration) => self.raised_at.elapsed() >= duration
        }
    }

    /// Time left before this message expires on its own, or `None` if it never does (already
    /// expired, or persistent). Used to schedule a repaint so the UI updates the moment it should
    /// disappear, instead of only on the next unrelated redraw.
    pub fn remaining(&self) -> Option<Duration> {
        match self.lifetime {
            MessageLifetime::Persistent => None,
            MessageLifetime::Timed(duration) => duration.checked_sub(self.raised_at.elapsed())
        }
    }
}
