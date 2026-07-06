use std::sync::LazyLock;
use std::time::Duration;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build()
        .new_agent()
});

pub fn agent() -> &'static ureq::Agent {
    &AGENT
}
