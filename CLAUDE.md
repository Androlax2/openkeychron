# openkeychron

The plan lives in GitHub Issues and the project board:
https://github.com/users/Androlax2/projects/2

## Git authorization

Claude may create feature branches, stage, commit, push feature branches and open pull
requests in this repository.

Claude may merge pull requests after reviewing them against their linked issue, once every
required check has passed. Merge with `gh pr merge --squash`, never with `--admin` or
`--delete-branch`.

Never commit or push to `main` directly, force-push, rebase pushed commits, delete branches
or create tags.
