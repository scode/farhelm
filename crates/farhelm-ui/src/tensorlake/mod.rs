//! Settings → Tensorlake: the recipe list and the recipe editor.
//!
//! Both are self-contained components: the Tensorlake settings section
//! (`tensorlake_settings.rs`, which owns the account side) hosts the list, and
//! the add-host dialog will host the editor for "New recipe…".

pub(crate) mod recipe_editor;
pub(crate) mod recipes;
