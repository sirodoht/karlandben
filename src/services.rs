use rand::Rng;

/// Generate a 6-digit verification code
pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000))
}
