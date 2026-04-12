//! Shared hidden-entry rules for filesystem browsing.
//! Keeps the current definition explicit so platform behavior can evolve in one place.

pub fn is_hidden_entry_name(name: &str) -> bool {
    name.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::is_hidden_entry_name;

    #[test]
    fn dot_prefixed_names_are_hidden() {
        assert!(is_hidden_entry_name(".secret"));
    }

    #[test]
    fn regular_names_are_not_hidden() {
        assert!(!is_hidden_entry_name("visible.txt"));
    }
}
