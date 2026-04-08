// Form data structures for authentication

#[derive(Debug, serde::Deserialize)]
pub struct EmailForm {
    pub email: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct VerifyForm {
    pub email: String,
    pub code: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct NameForm {
    pub email: String,
    pub name: String,
}
