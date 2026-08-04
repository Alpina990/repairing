#[derive(Debug, Clone)]
pub struct WelcomeContext<'a> {
    pub first_name: &'a str,
    pub username: Option<&'a str>,
    pub user_id: u64,
    pub chat_title: &'a str,
}

pub fn render_welcome(template: &str, context: &WelcomeContext<'_>) -> String {
    template
        .replace("{first_name}", context.first_name)
        .replace("{username}", context.username.unwrap_or("username yo'q"))
        .replace("{user_id}", &context.user_id.to_string())
        .replace("{chat_title}", context.chat_title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_supported_placeholders() {
        let value = render_welcome(
            "Salom {first_name}, {chat_title}ga xush kelibsiz!",
            &WelcomeContext {
                first_name: "Ali",
                username: Some("ali"),
                user_id: 1,
                chat_title: "Rust UZ",
            },
        );
        assert_eq!(value, "Salom Ali, Rust UZga xush kelibsiz!");
    }
}
