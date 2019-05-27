// Borrowed lovingly from Burntsushi:
// https://www.reddit.com/r/rust/comments/8fecqy/can_someone_show_an_example_of_failure_crate_usage/dy2u9q6/
// Chains errors into a big string.
pub fn pretty_error(err: &failure::Error) -> String {
    let mut pretty = err.to_string();
    let mut prev = err.as_fail();
    while let Some(next) = prev.cause() {
        pretty.push_str(":\n");
        pretty.push_str(&next.to_string());
        if let Some(bt) = next.backtrace() {
            let mut bts = bt.to_string();
            // If RUST_BACKTRACE is not defined, next.backtrace() gives us
            // Some(bt), but bt.to_string() gives us an empty string.
            // If we push a newline to the return value and nothing else,
            // we get something like:
            // ```
            // Some errror
            // :
            // Its cause
            // ```
            if !bts.is_empty() {
                bts.push_str("\n");
                pretty.push_str(&bts);
            }
        }
        prev = next;
    }
    pretty
}
