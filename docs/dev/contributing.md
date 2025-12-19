# Contributing

Thank you for your interest in contributing to charmer!

## Development Setup

### Prerequisites

- Rust 1.85+
- [Pixi](https://pixi.sh) (optional, for managing dependencies)

### Clone and Build

```bash
git clone https://github.com/rnabioco/charmer.git
cd charmer

# Using cargo
cargo build

# Or using pixi
pixi install
pixi run build
```

### Running Tests

```bash
cargo test
```

### Running Lints

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Code Style

- Follow Rust conventions and idioms
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common issues
- Write tests for new functionality
- Document public APIs

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `develop`
3. Make your changes
4. Run tests and lints
5. Submit a pull request to `develop`

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation changes

### Commit Messages

Use clear, descriptive commit messages:

```
Add log viewer component

- Implement scrollable log display
- Add follow mode for real-time updates
- Handle missing log files gracefully
```

## Architecture

See [Architecture](architecture.md) for an overview of the codebase.

## Testing

### Unit Tests

Add tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // ...
    }
}
```

### Integration Tests

Add integration tests in `tests/`:

```rust
// tests/integration_test.rs
use charmer_state::PipelineState;

#[test]
fn test_pipeline_state() {
    // ...
}
```

## Documentation

- Update relevant docs when changing functionality
- Use rustdoc comments for public APIs
- Preview docs with `cargo doc --open`

## Questions?

Open an issue for questions or discussion.
