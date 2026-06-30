/// Generic interface for plugin-accessible key-value storage.
/// The host implements this using the central `santui.db` `user_data` table.
pub trait DbAccess: Send {
    /// Read a value stored by a plugin for a specific user.
    /// Returns `None` if the key does not exist.
    fn get_value(&self, plugin: &str, user_id: &str, key: &str) -> Option<String>;

    /// Write a value (insert or update) for a plugin/user/key tuple.
    fn set_value(&mut self, plugin: &str, user_id: &str, key: &str, value: &str);
}
