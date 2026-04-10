use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}
