# Workspace rules

## Space Janitor (disk quota guard)
- An autonomous disk-optimizer daemon lives in `.space-janitor/` and starts on login. Never delete `.space-janitor/` or add its paths to any cleanup.
- If the sandbox home was wiped (no hook in `~/.bashrc`), re-run `bash /home/runner/workspace/.space-janitor/install.sh`.
- Check status with `cat .space-janitor/status.json`; pause with `touch .space-janitor/PAUSE`.

## Kaizen — autonomous improvement daemon
- An autonomous improvement daemon lives in `.kaizen/` and starts on login. Never delete `.kaizen/` or add its paths to any cleanup. It runs `run.sh --research` every hour, always innovating (deterministic probes + LLM research with proof validation).
- If the sandbox home was wiped, re-run `bash /home/runner/workspace/.kaizen/install.sh`.
- Check status with `cat .kaizen/status.json` and `cat .kaizen/logs/daemon.log`; pause with `touch .kaizen/PAUSE`; run once with `bash .kaizen/daemon.sh --once`.
- Improvements are tracked in `.agents/skills/kaizen/ledger.json` (mirrored to `railway-rs/static/data/kaizen.json`) and shown in the app at `/kaizen`.

## Git
- Always run `gh auth setup-git` before any `git push` (refreshes stored GitHub credentials; plain pushes fail with "Invalid username or token" otherwise).
