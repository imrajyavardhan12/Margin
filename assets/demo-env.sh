# Sourced by assets/demo.tape (vhs) before recording starts: put the release
# binary on PATH and stage a small repo with reviewable changes, so the GIF
# opens with plain `margin diff`. Regenerate the GIF with: vhs assets/demo.tape
export PATH="$PWD/target/release:$PATH"
demo="$(mktemp -d)/aurora"
mkdir -p "$demo/src"
cd "$demo" || exit 1
git init -q
git config user.email demo@margin.dev
git config user.name "Margin Demo"

cat > src/retry.rs <<'EOF'
//! Retry with exponential backoff.

use std::time::Duration;

pub struct Retry {
    attempts: u32,
    base_delay: Duration,
}

impl Retry {
    pub fn new(attempts: u32) -> Self {
        Self { attempts, base_delay: Duration::from_millis(100) }
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        self.base_delay * 2u32.pow(attempt)
    }

    pub fn run<T, E>(&self, mut op: impl FnMut() -> Result<T, E>) -> Result<T, E> {
        let mut last = None;
        for attempt in 0..self.attempts {
            match op() {
                Ok(value) => return Ok(value),
                Err(err) => {
                    std::thread::sleep(self.delay_for(attempt));
                    last = Some(err);
                }
            }
        }
        Err(last.expect("attempts must be nonzero"))
    }
}
EOF

cat > src/config.rs <<'EOF'
//! Service configuration.

pub struct Config {
    pub endpoint: String,
    pub timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self { endpoint: "https://api.example.dev".into(), timeout_secs: 30 }
    }
}
EOF

git add -A
git commit -qm "initial import"

# Working-tree changes the demo reviews.
cat > src/retry.rs <<'EOF'
//! Retry with exponential backoff and jitter.

use std::time::Duration;

pub struct Retry {
    attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl Retry {
    pub fn new(attempts: u32) -> Self {
        Self {
            attempts,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
        }
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        let exp = self.base_delay * 2u32.saturating_pow(attempt);
        let jitter = Duration::from_millis(u64::from(attempt) * 7 % 50);
        (exp + jitter).min(self.max_delay)
    }

    pub fn run<T, E>(&self, mut op: impl FnMut() -> Result<T, E>) -> Result<T, E> {
        let mut last = None;
        for attempt in 0..self.attempts {
            match op() {
                Ok(value) => return Ok(value),
                Err(err) => {
                    tracing::warn!(attempt, "operation failed; backing off before the next retry attempt so transient downstream hiccups get a chance to clear");
                    std::thread::sleep(self.delay_for(attempt));
                    last = Some(err);
                }
            }
        }
        Err(last.expect("attempts must be nonzero"))
    }
}
EOF

cat > src/config.rs <<'EOF'
//! Service configuration.

pub struct Config {
    pub endpoint: String,
    pub timeout_secs: u64,
    pub retry_attempts: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: "https://api.example.dev".into(),
            timeout_secs: 30,
            retry_attempts: 4,
        }
    }
}
EOF

clear
