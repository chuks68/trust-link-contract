# Pull Request Checklist

Thank you for contributing! Please review the following checklist to ensure your PR is ready for review.

## Summary
- [ ] Provide a clear and concise description of the changes.
- [ ] Link to any related issues using `Fixes #issue` or `Related to #issue`.

## Testing
- [ ] All new code is covered by unit tests where applicable.
- [ ] Existing tests pass locally (`cargo test`).
- [ ] Added tests for edge cases and error conditions.
- [ ] Updated any integration tests if necessary.

## Documentation
- [ ] Updated README.md if changes affect users.
- [ ] Updated docstrings/comments for new/modified functions.
- [ ] Added examples or updated existing examples if applicable.

## Code Quality
- [ ] Follows the project's coding style and conventions.
- [ ] No commented-out code or debug statements left in the codebase.
- [ ] Variables and functions are named descriptively.
- [ ] Code is properly formatted (`cargo fmt`).
- [ ] No new clippy warnings (`cargo clippy`).

## Breaking Changes
- [ ] If this PR introduces breaking changes, describe them and provide migration steps.
- [ ] Updated version in Cargo.toml if appropriate (following semver).

## Additional Notes
- [ ] Any other relevant information for reviewers.

Please ensure all checkboxes are checked before requesting a review.