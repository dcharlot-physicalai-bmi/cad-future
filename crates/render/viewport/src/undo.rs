//! Undo/Redo system — command-pattern action history for scene mutations.
//!
//! Each user action is captured as an `Action` that can be applied (do)
//! and reversed (undo). The `UndoStack` manages the history.

use crate::scene::{Scene, SceneNode};

/// A reversible scene mutation.
#[derive(Clone, Debug)]
pub enum Action {
    /// Add an object to the scene. Stores the node and the resulting index.
    AddObject {
        node: SceneNode,
        index: usize,
    },
    /// Remove an object. Stores what was removed so it can be re-added.
    RemoveObject {
        node: SceneNode,
        index: usize,
    },
    /// Move/transform an object.
    SetTransform {
        index: usize,
        old_transform: glam::Mat4,
        new_transform: glam::Mat4,
    },
    /// Toggle visibility.
    SetVisibility {
        index: usize,
        old_visible: bool,
        new_visible: bool,
    },
    /// Rename an object.
    Rename {
        index: usize,
        old_name: String,
        new_name: String,
    },
    /// Duplicate an object (stores the new index).
    Duplicate {
        source_index: usize,
        new_node: SceneNode,
        new_index: usize,
    },
    /// Batch of actions applied together.
    Batch(Vec<Action>),
}

impl Action {
    /// Apply this action to the scene (do / redo).
    pub fn apply(&self, scene: &mut Scene) {
        match self {
            Action::AddObject { node, .. } => {
                scene.add(node.clone());
            }
            Action::RemoveObject { index, .. } => {
                scene.remove(*index);
            }
            Action::SetTransform { index, new_transform, .. } => {
                scene.set_transform(*index, *new_transform);
            }
            Action::SetVisibility { index, new_visible, .. } => {
                scene.node_mut(*index).visible = *new_visible;
            }
            Action::Rename { index, new_name, .. } => {
                scene.node_mut(*index).name = new_name.clone();
            }
            Action::Duplicate { new_node, .. } => {
                scene.add(new_node.clone());
            }
            Action::Batch(actions) => {
                for a in actions {
                    a.apply(scene);
                }
            }
        }
    }

    /// Reverse this action on the scene (undo).
    pub fn undo(&self, scene: &mut Scene) {
        match self {
            Action::AddObject { index, .. } => {
                scene.remove(*index);
            }
            Action::RemoveObject { node, index, .. } => {
                scene.insert(*index, node.clone());
            }
            Action::SetTransform { index, old_transform, .. } => {
                scene.set_transform(*index, *old_transform);
            }
            Action::SetVisibility { index, old_visible, .. } => {
                scene.node_mut(*index).visible = *old_visible;
            }
            Action::Rename { index, old_name, .. } => {
                scene.node_mut(*index).name = old_name.clone();
            }
            Action::Duplicate { new_index, .. } => {
                scene.remove(*new_index);
            }
            Action::Batch(actions) => {
                // Undo in reverse order
                for a in actions.iter().rev() {
                    a.undo(scene);
                }
            }
        }
    }

    pub fn description(&self) -> String {
        match self {
            Action::AddObject { node, .. } => format!("Add {}", node.name),
            Action::RemoveObject { node, .. } => format!("Delete {}", node.name),
            Action::SetTransform { .. } => "Transform".into(),
            Action::SetVisibility { index, new_visible, .. } => {
                format!("{} #{}", if *new_visible { "Show" } else { "Hide" }, index)
            }
            Action::Rename { old_name, new_name, .. } => {
                format!("Rename {} → {}", old_name, new_name)
            }
            Action::Duplicate { source_index, .. } => format!("Duplicate #{}", source_index),
            Action::Batch(actions) => format!("Batch ({})", actions.len()),
        }
    }
}

/// Manages undo/redo history.
pub struct UndoStack {
    history: Vec<Action>,
    /// Points to the next action index. Everything before this is "done".
    cursor: usize,
    /// Maximum history depth.
    max_depth: usize,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            cursor: 0,
            max_depth: 256,
        }
    }

    /// Push a new action. Clears any redo history.
    pub fn push(&mut self, action: Action) {
        // Truncate redo history
        self.history.truncate(self.cursor);
        self.history.push(action);
        self.cursor += 1;

        // Enforce max depth
        if self.history.len() > self.max_depth {
            let excess = self.history.len() - self.max_depth;
            self.history.drain(0..excess);
            self.cursor -= excess;
        }
    }

    /// Undo the last action. Returns the action description if successful.
    pub fn undo(&mut self, scene: &mut Scene) -> Option<String> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        let action = &self.history[self.cursor];
        let desc = action.description();
        action.undo(scene);
        Some(desc)
    }

    /// Redo the next action. Returns the action description if successful.
    pub fn redo(&mut self, scene: &mut Scene) -> Option<String> {
        if self.cursor >= self.history.len() {
            return None;
        }
        let action = &self.history[self.cursor];
        let desc = action.description();
        action.apply(scene);
        self.cursor += 1;
        Some(desc)
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.history.len()
    }

    pub fn undo_description(&self) -> Option<&str> {
        if self.cursor > 0 {
            Some(&self.history[self.cursor - 1].description()).map(|_| {
                // Return a static-ish description
                match &self.history[self.cursor - 1] {
                    Action::AddObject { .. } => "Add",
                    Action::RemoveObject { .. } => "Delete",
                    Action::SetTransform { .. } => "Transform",
                    Action::SetVisibility { .. } => "Visibility",
                    Action::Rename { .. } => "Rename",
                    Action::Duplicate { .. } => "Duplicate",
                    Action::Batch(_) => "Batch",
                }
            })
        } else {
            None
        }
    }

    pub fn depth(&self) -> usize {
        self.cursor
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::SceneNode;
    use glam::Mat4;

    #[test]
    fn undo_add_object() {
        let mut scene = Scene::new();
        let mut undo = UndoStack::new();

        let node = SceneNode::new("Cube", 0, 0, Mat4::IDENTITY);
        let idx = scene.add(node.clone());
        undo.push(Action::AddObject { node, index: idx });

        assert_eq!(scene.len(), 1);
        undo.undo(&mut scene);
        assert_eq!(scene.len(), 0);
        undo.redo(&mut scene);
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn undo_remove_object() {
        let mut scene = Scene::new();
        let mut undo = UndoStack::new();

        let node = SceneNode::new("Cube", 0, 0, Mat4::IDENTITY);
        scene.add(node.clone());

        undo.push(Action::RemoveObject { node: node.clone(), index: 0 });
        scene.remove(0);
        assert_eq!(scene.len(), 0);

        undo.undo(&mut scene);
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn undo_transform() {
        let mut scene = Scene::new();
        let mut undo = UndoStack::new();

        let node = SceneNode::new("Cube", 0, 0, Mat4::IDENTITY);
        scene.add(node);

        let new_t = Mat4::from_translation(glam::Vec3::new(5.0, 0.0, 0.0));
        undo.push(Action::SetTransform {
            index: 0,
            old_transform: Mat4::IDENTITY,
            new_transform: new_t,
        });
        scene.set_transform(0, new_t);

        assert_eq!(scene.transform(0), new_t);
        undo.undo(&mut scene);
        assert_eq!(scene.transform(0), Mat4::IDENTITY);
    }

    #[test]
    fn redo_clears_on_new_action() {
        let mut scene = Scene::new();
        let mut undo = UndoStack::new();

        let n1 = SceneNode::new("A", 0, 0, Mat4::IDENTITY);
        let n2 = SceneNode::new("B", 1, 0, Mat4::IDENTITY);

        scene.add(n1.clone());
        undo.push(Action::AddObject { node: n1, index: 0 });

        scene.add(n2.clone());
        undo.push(Action::AddObject { node: n2.clone(), index: 1 });

        // Undo B
        undo.undo(&mut scene);
        assert_eq!(scene.len(), 1);
        assert!(undo.can_redo());

        // Push new action — clears redo
        let n3 = SceneNode::new("C", 2, 0, Mat4::IDENTITY);
        scene.add(n3.clone());
        undo.push(Action::AddObject { node: n3, index: 1 });

        assert!(!undo.can_redo());
        assert_eq!(scene.len(), 2);
    }

    #[test]
    fn max_depth_enforced() {
        let mut scene = Scene::new();
        let mut undo = UndoStack::new();

        for i in 0..300 {
            let node = SceneNode::new(&format!("Obj{}", i), 0, 0, Mat4::IDENTITY);
            let idx = scene.add(node.clone());
            undo.push(Action::AddObject { node, index: idx });
        }

        assert!(undo.depth() <= 256);
    }
}
