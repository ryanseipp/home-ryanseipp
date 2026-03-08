/// Validate a username: 1-64 ASCII alphanumeric, underscore, or hyphen.
pub(crate) fn validate_username(username: &str) -> Result<(), &'static str> {
    if username.is_empty() || username.len() > 64 {
        return Err("must be 1-64 characters, alphanumeric, underscore, or hyphen");
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("must be 1-64 characters, alphanumeric, underscore, or hyphen");
    }
    Ok(())
}

/// Validate a password: 8-128 characters.
pub(crate) fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("must be at least 8 characters");
    }
    if password.len() > 128 {
        return Err("must be at most 128 characters");
    }
    Ok(())
}

/// Validate an email: basic length and `@` check.
pub(crate) fn validate_email(email: &str) -> Result<(), &'static str> {
    if email.len() < 3 || email.len() > 254 || !email.contains('@') {
        return Err("invalid email address");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_usernames() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("bob-123").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("A").is_ok());
    }

    #[test]
    fn invalid_usernames() {
        assert!(validate_username("").is_err());
        assert!(validate_username("user name").is_err());
        assert!(validate_username("user@name").is_err());
        assert!(validate_username(&"a".repeat(65)).is_err());
    }

    #[test]
    fn valid_emails() {
        assert!(validate_email("a@b").is_ok());
        assert!(validate_email("user@example.com").is_ok());
    }

    #[test]
    fn invalid_emails() {
        assert!(validate_email("").is_err());
        assert!(validate_email("ab").is_err());
        assert!(validate_email("no-at-sign").is_err());
        assert!(validate_email(&format!("{}@b", "a".repeat(253))).is_err());
    }

    #[test]
    fn valid_passwords() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password(&"a".repeat(128)).is_ok());
    }

    #[test]
    fn invalid_passwords() {
        assert!(validate_password("short").is_err());
        assert!(validate_password(&"a".repeat(129)).is_err());
    }
}
