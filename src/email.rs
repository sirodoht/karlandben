use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use std::env;
use std::time::Duration;

#[derive(Clone)]
pub struct EmailService {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from_address: String,
    backend: EmailBackend,
}

#[derive(Clone, Copy)]
enum EmailBackend {
    Smtp,
    Console,
}

impl EmailService {
    pub fn new() -> Option<Self> {
        let from_address = env::var("SMTP_FROM").ok()?;
        let backend = match env::var("EMAIL_BACKEND").as_deref() {
            Ok("console") => EmailBackend::Console,
            _ => EmailBackend::Smtp,
        };

        let transport = match backend {
            EmailBackend::Console => None,
            EmailBackend::Smtp => {
                let smtp_host = env::var("SMTP_HOST").ok()?;
                let smtp_user = env::var("SMTP_USER").ok()?;
                let smtp_password = env::var("SMTP_PASSWORD").ok()?;
                let smtp_port = env::var("SMTP_PORT").ok()?.parse().ok()?;
                let smtp_tls = env::var("SMTP_TLS").ok()?;

                let creds = Credentials::new(smtp_user, smtp_password);

                let mut transport_builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_host)
                    .ok()?
                    .credentials(creds)
                    .port(smtp_port)
                    .timeout(Some(Duration::from_secs(10)));

                transport_builder = match smtp_tls.as_str() {
                    "required" | "starttls" => {
                        let tls_params = TlsParameters::new(smtp_host.clone()).ok()?;
                        transport_builder.tls(Tls::Required(tls_params))
                    }
                    "opportunistic" => {
                        let tls_params = TlsParameters::new(smtp_host.clone()).ok()?;
                        transport_builder.tls(Tls::Opportunistic(tls_params))
                    }
                    "none" | "off" => transport_builder.tls(Tls::None),
                    _ => {
                        let tls_params = TlsParameters::new(smtp_host.clone()).ok()?;
                        transport_builder.tls(Tls::Required(tls_params))
                    }
                };

                Some(transport_builder.build())
            }
        };

        Some(Self {
            transport,
            from_address,
            backend,
        })
    }

    pub async fn send_sign_in_code(&self, to_email: &str, code: &str) -> Result<(), String> {
        let subject = format!("{} is your fogpub sign-in code", code);
        let body = format!(
            "Your 6-digit sign-in code is: {}\n\nThis code will expire in 15 minutes.",
            code
        );

        self.send_email(to_email, &subject, &body).await
    }

    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        let email = Message::builder()
            .from(
                self.from_address
                    .parse()
                    .map_err(|_| "Invalid from address")?,
            )
            .to(to.parse().map_err(|_| "Invalid to address")?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())
            .map_err(|e| format!("Failed to build message: {}", e))?;

        match self.backend {
            EmailBackend::Console => {
                println!("\n========== EMAIL ==========");
                println!("To: {}", to);
                println!("From: {}", self.from_address);
                println!("Subject: {}", subject);
                println!("Content-Type: text/plain");
                println!("\n---------- BODY ----------");
                println!("{}", body);
                println!("===========================\n");
            }
            EmailBackend::Smtp => {
                if let Some(transport) = &self.transport {
                    transport
                        .send(email)
                        .await
                        .map_err(|e| format!("Failed to send email: {}", e))?;
                } else {
                    return Err("SMTP transport not initialized".to_string());
                }
            }
        }

        Ok(())
    }
}
