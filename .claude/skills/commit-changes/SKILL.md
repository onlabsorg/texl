---
name: commit-changes
description: Update this repository's <name>.summary.md with the pending changes (and any commits it does not cover yet), then commit the changes and the summary together. Run by the user with /commit-changes, optionally followed by a commit message.
disable-model-invocation: true
---

# Commit the changes, with the summary

Commit the pending changes together with an up-to-date `<name>.summary.md`
(the one `*.summary.md` file at the repository root), so that every commit
made with this skill carries a summary that describes it. The summary is
the factual helicopter view that the OnLabs.org coordination folder reads
instead of the code; see "Summary for OnLabs.org" in `CLAUDE.md` for what
belongs in it.

**Coverage rule.** The summary covers every commit up to and including the
last commit that changed it. Commits made after that one (for example with a
plain `git commit`) are not covered yet; this skill covers them too.

## Steps

1. **Decide what to commit.** Run `git status --short`.
   - If files are already staged, commit only those (plus the summary).
   - Otherwise commit every change in the working tree, untracked files
     included (`git add -A`; `.gitignore` applies).
   - If there is nothing to commit apart from the summary itself, and no
     uncovered commits (step 2), tell the user and stop.
   - If a file looks like it should not be committed (secrets such as
     `.env` or keys, large generated files, stray temporary files), stop and
     ask the user before staging it.

2. **Find the uncovered commits.** Take the last commit that changed the
   summary:

   ```bash
   git log -1 --format=%h -- <name>.summary.md
   ```

   and list the commits after it: `git log --format='%h %ad %s'
   --date=short <commit>..HEAD`. If the summary still has a legacy
   `Checked against: <commit> (<date>)` line, use that commit instead and
   delete the line (the date of coverage now comes from git). If the
   summary has never been committed and has no such line, ask the user
   which commit it was written against.

3. **Read the changes.** Read the uncovered commits (messages, then
   `git diff --stat <commit> HEAD` and `git show` where you need detail)
   and the changes about to be committed (`git diff --cached` after staging
   them, `git diff --cached --stat` first). Nothing else: changes left out
   of the commit are not part of the summary.

   If the summary itself already has uncommitted edits, keep them, build on
   them, and mention them in the report.

   **Session context.** If this session's conversation covers some of the
   changes, use it only to understand and word them (why a change was made,
   what it means at helicopter level). Every statement you add must be
   backed by an uncovered commit or by the changes being committed. Leave
   out anything discussed but not in them — plans, next steps you talked
   about, abandoned attempts, strategy — even if it seems certain.

4. **Update the summary.** Change only what the changes affect: features,
   public API, commands, file formats, architecture, versions and releases
   (with absolute dates, from tags: `git tag --contains <commit>`; a version
   bumped in this commit is not released until it is tagged and published),
   status, next steps, known limitations. Keep the existing structure. Stay
   at helicopter level, in plain words for a non-programmer reader; no code
   details, no strategy (why it matters, priorities, timing: those live in
   the coordination folder's `SUBPROJECTS.md`), no decisions. English,
   Markdown wrapped at about 80 characters.

   If nothing in the summary needs to change, commit without it. Those
   changes then stay uncovered, and the next run reads them again; that is
   harmless.

5. **Commit.** Stage the summary with the other changes and make one commit.
   Use the message the user passed with the command, if any; otherwise write
   one in the style of `git log` (short imperative subject line, about the
   changes, not the summary), following any commit conventions in
   `CLAUDE.md`. Do not push. If a hook fails, report the error and stop; do
   not bypass it.

6. **Report.** Give the commit hash and subject, the files committed, the
   uncovered commits you also covered, and what you changed in the summary.
