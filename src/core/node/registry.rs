use crate::core::node::{Node, NodeCategory, NodeIcon};

/// Describes a node type that can be created from the graph editor's "Add Node" menu. Node
/// implementations register one of these via `inventory::submit!`, so the menu never needs to
/// know about concrete node types - adding a new node type only means registering it where it's
/// defined, not editing the menu.
pub struct NodeDescriptor {
    /// Label shown for this node type in the "Add Node" menu.
    pub label: &'static str,
    pub category: NodeCategory,
    /// Must be one of `category.subcategories()`; groups this node within its category's menu.
    pub subcategory: &'static str,
    pub icon: NodeIcon,
    pub factory: fn() -> Box<dyn Node>
}
inventory::collect!(NodeDescriptor);

/// Iterates over every node type registered via `inventory::submit!`.
pub fn registered_nodes() -> impl Iterator<Item = &'static NodeDescriptor> {
    inventory::iter::<NodeDescriptor>()
}
