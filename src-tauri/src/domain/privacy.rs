/// Produces a stable, non-secret email hint for logs and authenticated UI status.
/// The full local part must never cross this policy boundary.
pub fn redact_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_owned();
    };
    let mut characters = local.chars();
    let first = characters.next().unwrap_or('*');
    let last = characters.last().unwrap_or(first);
    format!("{first}***{last}@{domain}")
}

#[cfg(test)]
mod tests {
    use super::redact_email;

    #[test]
    fn redacts_local_part_while_preserving_a_useful_hint() {
        assert_eq!(redact_email("student@example.test"), "s***t@example.test");
        assert_eq!(redact_email("a@example.test"), "a***a@example.test");
        assert_eq!(redact_email("invalid"), "***");
    }
}
