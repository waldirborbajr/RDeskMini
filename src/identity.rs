//! Device IDs: 9-digit numbers issued by the relay server, shown as "123 456 789".

pub(crate) const ID_DIGITS: usize = 9;

/// Groups the digits for display: "123456789" -> "123 456 789".
pub(crate) fn format_id(id: &str) -> String {
    let d: Vec<char> = id.chars().collect();
    if d.len() != ID_DIGITS {
        return id.to_string();
    }
    format!(
        "{}{}{} {}{}{} {}{}{}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8]
    )
}

/// Reads an ID typed by the user. Spaces and dashes are ignored.
pub(crate) fn parse_id(input: &str) -> Option<String> {
    let digits: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    (digits.len() == ID_DIGITS && digits.chars().all(|c| c.is_ascii_digit())).then_some(digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_in_groups_of_three() {
        assert_eq!(format_id("123456789"), "123 456 789");
    }

    #[test]
    fn parses_with_spaces_and_dashes() {
        assert_eq!(parse_id("123 456-789"), Some("123456789".to_string()));
        assert_eq!(parse_id(" 000000001 "), Some("000000001".to_string()));
    }

    #[test]
    fn rejects_invalid_ids() {
        assert_eq!(parse_id("12345678"), None);
        assert_eq!(parse_id("12345678a"), None);
        assert_eq!(parse_id(""), None);
    }
}
