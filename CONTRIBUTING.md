# Contributing to cargo-feat

Thanks for your interest in contributing.

## What this project is

`cargo-feat` is a fast CLI tool designed to improve performance over traditional Cargo metadata/info workflows for displaying the features of a specific crate.

It focuses on:
- speed
- correctness
- minimal overhead
- reproducible builds

## How to contribute

### 1. Fork & clone
```bash
git clone https://github.com/your-username/cargo-feat.git
cd cargo-feat

### 2. Create a branch

```bash
git checkout -b feature/your-feature-name
```

### 3. Make changes

Keep changes focused and minimal when possible.

### 4. Run checks

Make sure the project builds and runs correctly:

```bash
cargo build
cargo test
```

### 5. Commit style

Use clear commit messages:

* feat: add X
* fix: resolve Y
* refactor: improve Z
* perf: speed up X path

### 6. Push & PR

```bash
git push origin feature/your-feature-name
```

Then open a pull request.

## Guidelines

* Keep performance in mind — avoid regressions
* Prefer simple solutions over complex abstractions
* Do not introduce unnecessary dependencies
* Ensure cross-platform compatibility (Windows, Linux)

## Reporting issues

If you find a bug or performance issue, please open an issue with:

* steps to reproduce
* expected behavior
* actual behavior
* environment details (OS, Rust version)

## Code of conduct

Be respectful. Focus on the code, not the person.

## Notes

This project prioritizes performance and stability over experimental changes.
