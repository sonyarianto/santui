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
    fn sign_out(&self);
}
