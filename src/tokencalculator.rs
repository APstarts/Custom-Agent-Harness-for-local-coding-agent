pub fn estimateTokens(text: &str) -> usize {
    let char_count = text.chars().count();
    (char_count + 3) / 4
}
