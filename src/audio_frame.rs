//! Portable PCM framing: shared sample-value budget, output only, one format per tick.
pub(crate) fn append(
    output: &mut Vec<i16>,
    format: &mut u32,
    samples: &[i16],
    channels: u32,
    allowed: bool,
) -> bool {
    if !allowed
        || !matches!(channels, 1 | 2)
        || !samples.len().is_multiple_of(channels as usize)
        || output.len() + samples.len() > 1600
        || (!output.is_empty() && *format != channels)
    {
        return false;
    }
    if !samples.is_empty() {
        *format = channels;
        output.extend_from_slice(samples);
    }
    true
}
#[cfg(test)]
mod tests {
    use super::append;
    #[test]
    fn shared_budget_and_format_are_atomic() {
        let mut output = Vec::new();
        let mut channels = 1;
        assert!(append(&mut output, &mut channels, &[], 2, true));
        assert_eq!(channels, 1);
        assert!(!append(&mut output, &mut channels, &[1], 2, true));
        assert!(output.is_empty());
        assert!(!append(&mut output, &mut channels, &[1, 2], 2, false));
        assert!(append(
            &mut output,
            &mut channels,
            &[32767, -32768],
            2,
            true
        ));
        assert_eq!(channels, 2);
        assert!(!append(&mut output, &mut channels, &[7], 1, true));
        assert_eq!(output, [32767, -32768]);
        assert!(append(&mut output, &mut channels, &vec![3; 1598], 2, true));
        assert_eq!(output.len(), 1600);
        assert!(!append(&mut output, &mut channels, &[1, 2], 2, true));
        assert_eq!(output.len(), 1600);
    }
}
