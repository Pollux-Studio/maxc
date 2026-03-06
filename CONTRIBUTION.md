# 🤝 Contributing to maxc

Thank you for your interest in contributing to **maxc**.

maxc is an open source project and contributions from the community are welcome.
Whether you are fixing bugs, improving documentation, or building new features, your help is valuable.

# 📌 Ways to Contribute

You can contribute to maxc in several ways.

### 🐞 Reporting bugs

If you find a bug, please open an issue describing:

* what happened
* expected behavior
* steps to reproduce
* screenshots if relevant
* operating system and environment

### 💡 Suggesting features

Feature requests are welcome.

Before submitting a feature request:

1. Check existing issues
2. Explain the use case
3. Describe how the feature would improve maxc

### 🧑‍💻 Contributing code

You can contribute by:

* implementing features
* fixing bugs
* improving performance
* improving documentation

# ⚙ Development Setup

### 1. Install Rust

Install Rust using:

```
https://rustup.rs
```

Verify installation:

```
rustc --version
cargo --version
```

### 2. Clone the repository

```
git clone https://github.com/:Pollux-Studio/maxc.git
cd maxc
```

### 3. Create a development branch

Never work directly on `main`.

```
git checkout develop
git checkout -b feature/my-feature
```

### 4. Build the project

```
cargo build
```

Run the project:

```
cargo run
```

Run tests:

```
cargo test
```

# 🧹 Code Style Guidelines

Please follow Rust best practices.

General rules:

* keep functions small and readable
* avoid unnecessary complexity
* write descriptive variable names
* document public APIs
* follow idiomatic Rust patterns

Run formatter before committing:

```
cargo fmt
```

Run linter:

```
cargo clippy
```

# 🧪 Testing

All major features should include tests.

Testing guidelines:

* write unit tests when possible
* test edge cases
* avoid flaky tests

Run tests using:

```
cargo test
```

# 🔀 Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push the branch
5. Open a pull request

Pull request checklist:

* code builds successfully
* tests pass
* formatting is correct
* documentation updated if necessary

# 📄 Commit Message Guidelines

Use clear and descriptive commit messages.

Examples:

```
Add browser surface engine
Fix terminal input freeze
Improve workspace manager performance
```

Avoid vague messages like:

```
fix stuff
update code
```

# 📚 Documentation

Documentation improvements are always welcome.

You can contribute by:

* improving README
* adding usage examples
* clarifying architecture
* improving comments in code

# 🧑‍⚖ Code of Conduct

Please be respectful and constructive when interacting with other contributors.

We want maxc to be a welcoming project for everyone.

# ❤️ Thank You

Every contribution helps improve maxc.

Whether it's a small typo fix or a major feature, we appreciate your effort in helping grow this project.
