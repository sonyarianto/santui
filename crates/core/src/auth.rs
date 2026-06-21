#[derive(Clone, Debug)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub provider: String,
}

pub trait AuthHandle: Send + Sync {
    fn current_user(&self) -> Option<User>;
    fn bearer_token(&self) -> Option<String>;
    fn sign_in(&self, provider: &str) -> Result<User, Box<dyn std::error::Error>>;

    /// Start a non-blocking sign-in. Returns immediately.
    /// Call `drain_pending_sign_in` periodically to get the result.
    fn start_sign_in(&self, provider: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn drain_pending_sign_in(&self) -> Option<Result<User, Box<dyn std::error::Error>>>;
    /// A user-facing message about an ongoing auth flow (e.g. "Enter code XXXXX").
    fn auth_message(&self) -> Option<String>;
    fn sign_out(&self);
}
