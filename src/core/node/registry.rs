use crate::core::node::{Node, NodeCategory, NodeIcon};

/// A node type registered in the system, with its label, category, icon, and factory function.
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
