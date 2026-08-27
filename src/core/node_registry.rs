use crate::core::node::{Node, NodeCategory, NodeIcon};

/// Node implementations register one of these via `inventory::submit!`, so adding a new node type only means registering it where it's defined, not editing the "Add Node" menu.
pub struct NodeDescriptor {
    pub label: &'static str,
    pub category: NodeCategory,
    pub icon: NodeIcon,
    pub factory: fn() -> Box<dyn Node>
}
inventory::collect!(NodeDescriptor);

/// Iterates over every node type registered via `inventory::submit!`.
pub fn registered_nodes() -> impl Iterator<Item = &'static NodeDescriptor> {
    inventory::iter::<NodeDescriptor>()
}
