# Logship

## Development
Manual Bumping: Any commit message that includes #major, #minor, #patch, or #none will trigger the respective version bump. If two or more are present, the highest-ranking one will take precedence. If #none is contained in the merge commit message, no release is created.
Automatic Bumping: All pull requests merged into master will create a new patch version.
