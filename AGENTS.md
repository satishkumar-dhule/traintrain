# Workspace rules

## Space Janitor (disk quota guard)
- An autonomous disk-optimizer daemon lives in `.space-janitor/` and starts on login. Never delete `.space-janitor/` or add its paths to any cleanup.
- If the sandbox home was wiped (no hook in `~/.bashrc`), re-run `bash /home/runner/workspace/.space-janitor/install.sh`.
- Check status with `cat .space-janitor/status.json`; pause with `touch .space-janitor/PAUSE`.

## Git
- Always run `gh auth setup-git` before any `git push` (refreshes stored GitHub credentials; plain pushes fail with "Invalid username or token" otherwise).
