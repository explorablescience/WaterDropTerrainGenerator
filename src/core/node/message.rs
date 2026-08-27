use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::core::graph::GraphNodeId;
use crate::core::node::error::NodeError;

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

/// Per-node feedback from the most recent `on_action`/`set_param` call, keyed by the node it came
/// from: an error persists until the next call on that node, while a success confirmation fades
/// out on its own.
#[derive(Default)]
pub struct NodeMessageLog {
    messages: HashMap<GraphNodeId, TimedNodeMessage>
}
impl NodeMessageLog {
    /// How long a success confirmation stays visible before it fades out.
    pub const ACTION_MESSAGE_DURATION: Duration = Duration::from_secs(3);

    /// Records the feedback of an `on_action`/`set_param` call on `node_id`, replacing whatever
    /// was shown before it.
    pub fn set_result(&mut self, node_id: GraphNodeId, result: Result<String, NodeError>) {
        let timed = match result {
            Ok(text) => TimedNodeMessage::new(
                NodeMessage::info(text),
                MessageLifetime::Timed(Self::ACTION_MESSAGE_DURATION)
            ),
            Err(err) => TimedNodeMessage::new(
                NodeMessage {
                    severity: err.severity(),
                    text: err.to_string()
                },
                MessageLifetime::Persistent
            )
        };
        self.messages.insert(node_id, timed);
    }

    /// Drops any feedback currently shown for `node_id`.
    pub fn clear(&mut self, node_id: GraphNodeId) {
        self.messages.remove(&node_id);
    }

    /// The still-live feedback for `node_id`, if any.
    pub fn get(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.messages
            .get(&node_id)
            .filter(|m| !m.is_expired())
            .map(|m| &m.message)
    }

    /// Time left before `node_id`'s feedback expires on its own, if it's timed and still live.
    pub fn remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.messages
            .get(&node_id)
            .and_then(TimedNodeMessage::remaining)
    }

    /// Drops every message that has expired.
    pub fn prune_expired(&mut self) {
        self.messages.retain(|_, m| !m.is_expired());
    }
}
