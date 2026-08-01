/// Convert an mpv return code to a `Result`, erroring with context on failure.
pub fn to_rc(rc: i32, ctx: &str) -> Result<(), Box<dyn std::error::Error>> {
    if rc >= 0 {
        Ok(())
    } else {
        Err(format!("mpv {ctx} failed: {rc}").into())
    }
}
