use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub logged_in: bool,
    pub name: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/signup.html")]
pub struct SignupTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/verify.html")]
pub struct VerifyTemplate {
    pub email: String,
    pub purpose: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/profile.html")]
pub struct ProfileTemplate {
    pub email: String,
    pub error: Option<String>,
}
