# Continuous integration

GitHub Actions runs the [CI workflow](../.github/workflows/ci.yml) for every pull request and every push to `main`. The **Rust checks** job uses Ubuntu and stable Rust, which satisfies the project's Rust 1.88 minimum, with the `rustfmt` and `clippy` components installed. It runs the same formatting, test, lint, and build commands listed in the [development guide](development.md). A failed command fails the job and its GitHub check; successful runs appear in the repository's Actions tab and on pull requests.

## For contributors

Run these checks from the repository root before opening a pull request:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build
```

Open the pull request's **Checks** tab or the repository's **Actions** tab to inspect results and logs. Fix any failed command, then push the change to rerun CI.

## For maintainers

To prevent merging a pull request with failed CI, configure a rule for `main` in GitHub's branch protection or rulesets and require the **Rust checks** status check. The check becomes selectable after the workflow has run on GitHub at least once. The workflow itself reports pass or failure; merge protection is a repository setting.
