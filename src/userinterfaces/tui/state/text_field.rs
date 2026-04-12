use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone)]
pub struct TextField {
    pub value: String,
    pub cursor: usize,
    clear_on_next_edit: bool,
}

impl TextField {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            value,
            cursor,
            clear_on_next_edit: false,
        }
    }

    pub(crate) fn arm_clear_on_next_edit(&mut self) {
        if !self.value.is_empty() {
            self.clear_on_next_edit = true;
        }
    }

    pub fn apply_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    self.consume_clear_on_next_edit();
                    self.insert(c);
                    true
                } else {
                    false
                }
            }
            KeyCode::Backspace => {
                self.consume_clear_on_next_edit();
                self.backspace();
                true
            }
            KeyCode::Delete => {
                self.consume_clear_on_next_edit();
                self.delete();
                true
            }
            KeyCode::Left => {
                self.clear_on_next_edit = false;
                self.move_left();
                true
            }
            KeyCode::Right => {
                self.clear_on_next_edit = false;
                self.move_right();
                true
            }
            KeyCode::Home => {
                self.clear_on_next_edit = false;
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.clear_on_next_edit = false;
                self.cursor = self.value.chars().count();
                true
            }
            _ => false,
        }
    }

    fn consume_clear_on_next_edit(&mut self) {
        if self.clear_on_next_edit {
            self.value.clear();
            self.cursor = 0;
            self.clear_on_next_edit = false;
        }
    }

    fn insert(&mut self, c: char) {
        let mut chars: Vec<char> = self.value.chars().collect();
        let idx = self.cursor.min(chars.len());
        chars.insert(idx, c);
        self.value = chars.into_iter().collect();
        self.cursor = idx + 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.value.chars().collect();
        let idx = self.cursor - 1;
        if idx < chars.len() {
            chars.remove(idx);
            self.value = chars.into_iter().collect();
            self.cursor = idx;
        }
    }

    fn delete(&mut self) {
        let mut chars: Vec<char> = self.value.chars().collect();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.value = chars.into_iter().collect();
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        let max = self.value.chars().count();
        if self.cursor < max {
            self.cursor += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::TextField;

    #[test]
    fn handles_basic_editing() {
        let mut field = TextField::new("ab");
        assert!(field.apply_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)));
        assert!(field.apply_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE)));
        assert_eq!(field.value, "aXb");
        assert!(field.apply_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)));
        assert_eq!(field.value, "ab");
    }

    #[test]
    fn replaces_existing_value_on_first_typed_character() {
        let mut field = TextField::new("3d");
        field.arm_clear_on_next_edit();

        assert!(field.apply_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE)));
        assert_eq!(field.value, "7");
    }

    #[test]
    fn arrow_navigation_keeps_existing_value_for_editing() {
        let mut field = TextField::new("3d");
        field.arm_clear_on_next_edit();

        assert!(field.apply_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)));
        assert!(field.apply_key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE)));
        assert_eq!(field.value, "37d");
    }
}
