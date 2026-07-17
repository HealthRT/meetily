# Contributing to Meetily

Thank you for your interest in contributing to Meetily! This document provides guidelines and instructions for contributing to this project.

## Development Workflow

### Branch Strategy

- `main` - Production branch
- `devtest` - Development and testing branch
- Feature branches should be created from `devtest`

### Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/meetily.git
   ```
3. Add the original repository as upstream:
   ```bash
   git remote add upstream https://github.com/Zackriya-Solutions/meetily.git
   ```
4. Create a new branch from `devtest`:
   ```bash
   git checkout devtest
   git pull upstream devtest
   git checkout -b feat/your-feature-name
   ```

### Development Process

1. Always start your work from the `devtest` branch
2. Create a focused branch using `feat/`, `fix/`, `docs/`, `test/`, or `chore/`
3. Make your changes
4. Write or update tests as needed
5. Ensure all tests pass
6. Update documentation if necessary

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

1. Create a PR from your feature branch to `devtest`
2. Link the PR to the related issue using the issue number (e.g., "Fixes #123")
3. Use a Conventional Commit title such as `feat(audio): detect meeting activity`
4. Fill out the PR template completely
5. Ensure all required CI checks pass
6. Request review from at least one maintainer when an independent reviewer is available
7. Address every review comment
8. Once approved, the PR will be merged into `devtest`
9. Promote tested releases from `devtest` to `main` with a release PR

### PR Template

GitHub automatically loads `.github/pull_request_template.md`. Complete every
applicable section and replace placeholder text before requesting review.

## Code Style

- Follow the existing code style
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
- perf: Performance improvements
- test: Adding/updating tests
- build: Build system or dependency changes
- ci: CI/CD changes
- chore: Maintenance tasks

Use the same format for pull request titles. Do not add `[skip ci]`; required
checks must run for every pull request.

## Testing

- Write unit tests for new features and update affected existing tests
- Run `pnpm test` and `pnpm build` from `frontend/` for frontend changes
- Run focused Rust tests from the repository root, for example
  `cargo test -p meetily meeting_detection --lib`
- Run `git diff --check`
- Include integration or manual tests for platform-specific behavior
- Document any test that could not be run and why

## Documentation

- Update documentation for new features
- Keep README up to date
- Document API changes
- Add comments for complex code

## Review Process

1. PRs should receive at least one independent review when another maintainer is available
2. Address all review comments
3. Keep the PR up to date with `devtest`
4. Required CI checks must pass
5. Use squash or rebase merging to preserve linear history

## Getting Help

- Create an issue for questions
- Join our community chat
- Contact maintainers

## License

By contributing, you agree that your contributions will be licensed under the project's MIT License.
