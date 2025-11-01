# hotswap-config

> Zero-downtime configuration management with lock-free hot-reloads and atomic updates

[![Crates.io](https://img.shields.io/crates/v/hotswap-config.svg)](https://crates.io/crates/hotswap-config)
[![Documentation](https://docs.rs/hotswap-config/badge.svg)](https://docs.rs/hotswap-config)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

## Why hotswap-config?

Traditional configuration libraries require application restarts to pick up changes. **hotswap-config** provides:

- ⚡ **Lock-free reads** - Sub-10ns read latency using `arc-swap`
- 🔄 **Zero-downtime updates** - Change config without dropping requests
- ✅ **Validation** - Reject invalid configs, keep the old one
- 📁 **Standard precedence** - Files → environment variables (like the `config` crate)
- 🎯 **Type-safe** - Full Rust type system guarantees
- 🔌 **Pluggable sources** - Files, HTTP, etcd, Consul, Vault

Built on patterns battle-tested at scale in high-throughput microservices.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
hotswap-config = "0.1"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }
```

Define your config:

```rust
use hotswap_config::prelude::*;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct AppConfig {
    server: ServerConfig,
    rate_limit: RateLimitConfig,
}

#[derive(Debug, Deserialize, Clone)]
struct ServerConfig {
    port: u16,
    host: String,
}

#[derive(Debug, Deserialize, Clone)]
struct RateLimitConfig {
    requests_per_second: u32,
}
```

Load and use:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load with standard precedence: default.yaml → production.yaml → env vars
    let config = HotswapConfig::builder()
        .with_file("config/default.yaml")
        .with_file("config/production.yaml")
        .with_env_overrides("APP", "__")  // APP_SERVER__PORT=8080
        .build::<AppConfig>()
        .await?;

    // Clone is cheap - just an Arc clone
    let config_clone = config.clone();

    // Spawn a task that uses config
    tokio::spawn(async move {
        loop {
            // Zero-cost read (no locks!)
            let cfg = config_clone.get();
            println!("Rate limit: {}", cfg.rate_limit.requests_per_second);
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // Your main application logic
    let cfg = config.get();
    println!("Starting server on {}:{}", cfg.server.host, cfg.server.port);

    Ok(())
}
```

## Features

### Core Features (Enabled by Default)

- **File watching**: Automatic reload when config files change
- **Validation**: Validate configs before applying updates

### Advanced Features

Enable in `Cargo.toml`:

```toml
[dependencies]
hotswap-config = { version = "0.1", features = ["partial-updates", "rollback", "gradual-rollout"] }
```

#### Partial Updates

Update specific fields without full reload using JSON Patch:

```rust
// Update just the rate limit
config.update_field("/rate_limit/requests_per_second", 5000).await?;
```

#### Rollback Support

Time-travel to previous configurations:

```rust
let config = HotswapConfig::builder()
    .with_file("config.yaml")
    .with_history(20)  // Keep last 20 versions
    .build::<AppConfig>()
    .await?;

// Oh no, bad config!
config.rollback(1).await?;  // Back to previous version
```

#### Gradual Rollout

A/B test configuration changes:

```rust
// Start with 10% traffic on new config
config.enable_gradual_rollout(10).await?;
config.load_canary("config/canary.yaml").await?;

// Monitor metrics, then increase
config.increase_rollout(50).await?;  // Now 60%

// Promote to 100%
config.promote_rollout().await?;
```

### All Feature Flags

| Feature | Description |
|---------|-------------|
| `file-watch` | Automatic reload on file changes (default) |
| `validation` | Config validation support (default) |
| `yaml` | YAML file format support |
| `toml` | TOML file format support |
| `json` | JSON file format support |
| `all-formats` | Enable all file formats |
| `partial-updates` | JSON Patch for surgical updates |
| `rollback` | Configuration history and rollback |
| `gradual-rollout` | A/B testing and percentage rollout |
| `remote` | HTTP/HTTPS remote config sources |
| `secrets-vault` | HashiCorp Vault integration |
| `secrets-aws` | AWS Secrets Manager integration |
| `secrets-gcp` | GCP Secret Manager integration |
| `metrics` | Built-in OpenTelemetry metrics |
| `tracing` | Structured logging support |

## Configuration Precedence

hotswap-config follows standard precedence (highest to lowest):

1. **Environment variables** (e.g., `APP_SERVER__PORT=8080`)
2. **Environment-specific file** (e.g., `config/production.yaml`)
3. **Default file** (e.g., `config/default.yaml`)

This matches the behavior of the popular `config` crate.

## Performance

Measured on Apple M1 Pro:

| Operation | Latency | Notes |
|-----------|---------|-------|
| Read config | ~5-8ns | Lock-free `arc-swap` |
| Clone handle | ~4ns | Just an `Arc` clone |
| Reload (small) | ~50µs | Full validation + atomic swap |
| Reload (large) | ~200µs | 1000+ field config |

**Zero dropped requests during reload** - readers never block.

## How It Works

hotswap-config uses a copy-on-write pattern with `arc-swap`:

1. **Reads** use `arc-swap::ArcSwap::load()` - lock-free atomic read
2. **Updates** build a new config off-to-the-side, validate it, then atomically swap
3. **Old readers** continue using the old config until their `Arc` is dropped
4. **New readers** get the new config immediately

This is the same pattern used in high-performance services for permission checks (1M+ ops/sec).

## Examples

See the [`examples/`](examples/) directory:

- [`basic_usage.rs`](examples/basic_usage.rs) - Simple config loading
- [`hot_reload.rs`](examples/hot_reload.rs) - File watching and auto-reload
- [`validation.rs`](examples/validation.rs) - Custom validation
- [`partial_updates.rs`](examples/partial_updates.rs) - JSON Patch updates
- [`rollback.rs`](examples/rollback.rs) - Time-travel configuration
- [`gradual_rollout.rs`](examples/gradual_rollout.rs) - A/B testing

Run an example:

```bash
cargo run --example basic_usage --features yaml
```

## Comparison with Other Crates

| Feature | hotswap-config | config | figment | confy |
|---------|---------------|--------|---------|-------|
| Lock-free reads | ✅ | ❌ | ❌ | ❌ |
| Hot reload | ✅ | ❌ | ❌ | ❌ |
| File watching | ✅ | ❌ | ❌ | ❌ |
| Validation | ✅ | Limited | ✅ | ❌ |
| Rollback | ✅ | ❌ | ❌ | ❌ |
| Partial updates | ✅ | ❌ | ❌ | ❌ |
| Type-safe | ✅ | ✅ | ✅ | ✅ |
| Precedence | ✅ | ✅ | ✅ | Limited |

## Migration from `config` Crate

hotswap-config is designed to be a drop-in replacement:

```diff
- use config::{Config, File, Environment};
+ use hotswap_config::prelude::*;

- let settings = Config::builder()
+ let settings = HotswapConfig::builder()
      .add_source(File::with_name("config/default"))
      .add_source(Environment::with_prefix("APP"))
-     .build()?
-     .try_deserialize::<AppConfig>()?;
+     .build::<AppConfig>()
+     .await?;

- // Access fields directly
- println!("Port: {}", settings.server.port);
+ // Get config handle (zero-cost)
+ let cfg = settings.get();
+ println!("Port: {}", cfg.server.port);
```

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

- Inspired by the `config` crate's precedence model
- Uses the excellent `arc-swap` crate for lock-free atomic updates
- Pattern proven at scale in production microservices

---

**Status**: 🚧 Early development (v0.1) - API may change

Built with ❤️ by [Daniel Curtis](https://github.com/danielrcurtis)
