use std::time::Duration;

use waterdrop_terrain_editor::core::node_message::{MessageLifetime, NodeMessage, NodeMessageSeverity, TimedNodeMessage};

#[test]
fn constructors_set_the_expected_severity() {
    assert_eq!(NodeMessage::error("x").severity, NodeMessageSeverity::Error);
    assert_eq!(NodeMessage::warning("x").severity, NodeMessageSeverity::Warning);
    assert_eq!(NodeMessage::info("x").severity, NodeMessageSeverity::Info);
}

#[test]
fn constructors_preserve_the_text() {
    assert_eq!(NodeMessage::error("boom").text, "boom");
    assert_eq!(NodeMessage::info(String::from("done")).text, "done");
}

#[test]
fn persistent_messages_never_expire() {
    let msg = TimedNodeMessage::new(NodeMessage::error("boom"), MessageLifetime::Persistent);
    assert!(!msg.is_expired());
    assert_eq!(msg.remaining(), None, "a persistent message has no countdown");
}

#[test]
fn timed_messages_expire_after_their_duration() {
    let msg = TimedNodeMessage::new(
        NodeMessage::info("saved"),
        MessageLifetime::Timed(Duration::from_millis(1))
    );
    std::thread::sleep(Duration::from_millis(20));
    assert!(msg.is_expired());
}

#[test]
fn timed_messages_report_remaining_time_before_expiring() {
    let msg = TimedNodeMessage::new(
        NodeMessage::info("saved"),
        MessageLifetime::Timed(Duration::from_secs(60))
    );
    assert!(!msg.is_expired());
    let remaining = msg.remaining().expect("a fresh timed message should have time left");
    assert!(remaining <= Duration::from_secs(60));
    assert!(remaining > Duration::from_secs(50));
}

#[test]
fn expired_timed_messages_report_no_remaining_time() {
    let msg = TimedNodeMessage::new(
        NodeMessage::info("saved"),
        MessageLifetime::Timed(Duration::from_millis(1))
    );
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(msg.remaining(), None);
}
