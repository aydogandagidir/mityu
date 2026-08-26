# Contributing to Mityu

Thank you for your interest in contributing to Mityu! This document provides guidelines and instructions for contributing to this project.

## Licensing and Sign-off

Mityu is derived from Meetily (MIT, Zackriya Solutions — that copyright notice
stays in `LICENSE.md`) and is distributed as a commercial product. So we need to be
able to say, without ambiguity, that we have the right to ship every line in the
tree.

We use a **Developer Certificate of Origin** rather than a separate contributor
licence agreement: there is nothing to sign, nothing for us to administer, and
you keep your copyright. Add a `Signed-off-by` line to every commit —
`git commit -s` writes it for you:

```
Signed-off-by: Your Name <your.email@example.com>
```

That line means you certify [DCO 1.1](https://developercertificate.org/): the
work is yours to contribute, and you are contributing it under this repository's
licence (MIT). Use your real name and an address you can be reached at.

If a contribution includes code, models, or binaries you did **not** write, say
so in the pull request and name the licence — including transitive dependencies.
`cargo deny check` runs in CI and rejects copyleft licences and unvetted git
sources (see `deny.toml`); a dependency that needs an exception needs an ADR, not
a config edit.

## Development Workflow

### Branch Strategy

- `main` — production branch
- Feature branches are created from `main`

### Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/mityu.git
   ```
3. Add the original repository as upstream:
   ```bash
   git remote add upstream https://github.com/aydogandagidir/mityu.git
   ```
4. Create a new branch from `main`:
   ```bash
   git checkout main
   git pull upstream main
   git checkout -b feature/your-feature-name
   ```

### Development Process

1. Always start your work from an up-to-date `main` branch
2. Create a new branch for each feature/fix
3. Make your changes
4. Write or update tests as needed
5. Ensure all tests pass
6. Update documentation if necessary

Please also read [`CLAUDE.md`](CLAUDE.md) and the design docs in [`docs/`](docs/) (architecture, conventions, security/privacy, and the decision log) before non-trivial changes — Mityu is local-first and tenant-aware by design, and those invariants are enforced in review.

### Issue Creation

Before starting work on a new feature or bug fix:

1. Check if an issue already exists
2. If not, create a new issue with:
   - Clear title
   - Detailed description
   - Steps to reproduce (for bugs)
   - Expected behavior
   - Screenshots (if applicable)
   - Labels (bug, enhancement, etc.)

### Pull Request Process

1. Create a PR from your feature branch to `main`
2. Link the PR to the related issue using the issue number (e.g., "Fixes #123")
3. Sign off every commit (`git commit -s`) — see **Licensing and Sign-off** above
4. Fill out the PR template completely
5. Ensure CI checks pass
6. Request review from at least one maintainer
7. Address any review comments
8. Once approved, the PR will be merged into `main`

### PR Template

```markdown
## Description
[Describe your changes here]

## Related Issue
[Link to the issue this PR addresses]

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring
- [ ] Other (please describe)

## Testing
- [ ] Unit tests added/updated
- [ ] Manual testing performed
- [ ] All tests pass

## Documentation
- [ ] Documentation updated
- [ ] No documentation needed

## Checklist
- [ ] Code follows project style
- [ ] Self-reviewed the code
- [ ] Added comments for complex code
- [ ] Updated README if needed
```

## Code Style

- Follow the existing code style (`cargo fmt` + `cargo clippy` for Rust; `pnpm lint` + `pnpm tsc --noEmit` for the frontend)
- Use meaningful variable and function names
- Add comments for complex logic
- Keep functions small and focused
- Write clear commit messages

## Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:
- feat: New feature
- fix: Bug fix
- docs: Documentation changes
- style: Code style changes
- refactor: Code refactoring
- test: Adding/updating tests
- chore: Maintenance tasks

## Testing

- Write unit tests for new features
- Update existing tests when modifying code
- Ensure all tests pass before submitting a PR
- Include integration tests for complex features

## Documentation

- Update documentation for new features
- Keep the README up to date
- Document API changes
- Add comments for complex code

## Review Process

1. PRs require at least one review
2. Address all review comments
3. Keep the PR up to date with `main`
4. Squash commits if requested

## Getting Help

- Create an issue for questions
- Contact the maintainers at info@bluedev.dev

## License

By contributing, you agree that your contributions will be licensed under the project's MIT License.
