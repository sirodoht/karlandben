use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub logged_in: bool,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
    pub logged_in: bool,
    pub email: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/verify.html")]
pub struct VerifyTemplate {
    pub form_email: String,
    pub error: Option<String>,
    pub logged_in: bool,
    pub email: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/profile.html")]
pub struct ProfileTemplate {
    pub form_email: String,
    pub error: Option<String>,
    pub logged_in: bool,
    pub email: Option<String>,
    pub name: Option<String>,
}
