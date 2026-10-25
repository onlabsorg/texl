---
name: update-summary
description: Update this repository's <name>.summary.md with the commits made since the summary was last checked. Committed changes only. Run by the user with /update-summary.
disable-model-invocation: true
---

# Update the summary

Bring `<name>.summary.md` (the one `*.summary.md` file at the repository
root) up to date with the **committed** history. The summary is the factual
helicopter view that the OnLabs.org coordination folder reads instead of the
code; see "Summary for OnLabs.org" in `CLAUDE.md` for what belongs in it.

## Steps

1. **Find the starting point.** Read the summary and take the commit from
   its `Checked against: <commit> (<date>)` line. The summary covers every
   commit up to and including that one. If the line is missing, or the
   commit is not an ancestor of `HEAD`
   (`git merge-base --is-ancestor <commit> HEAD`), stop and ask the user.

2. **List what is not covered:**

   ```bash
   git log --format='%h %ad %s' --date=short <commit>..HEAD -- . ':(exclude)<name>.summary.md'
   ```

   If the list is empty, tell the user the summary is up to date and stop
   without changing anything.

3. **Read the committed changes only.** Use the commit messages, then
   `git diff --stat <commit> HEAD` and `git show` / `git diff <commit> HEAD
   -- <path>` where you need detail. To read a file as committed, use
   `git show HEAD:<path>`. Never take content from the working tree or the
   index: uncommitted and unstaged changes are not part of the summary, even
   if they are on disk. If the summary file itself has uncommitted edits
   (`git status --short <name>.summary.md`), stop and ask the user whether
   to keep them.

   **Session context.** If this session's conversation covers some of the
   commits, use it only to understand and word them (why a change was made,
   what it means at helicopter level). Every statement you add must be
   backed by a commit in the range. Leave out anything discussed but not
   committed — plans, next steps you talked about, abandoned attempts,
   strategy — even if it seems certain. Commits the conversation does not
   cover are read from git as usual.

4. **Update the content.** Change only what the new commits affect: features,
   public API, commands, file formats, architecture, versions and releases
   (with absolute dates, from tags: `git tag --contains <commit>`), status,
   next steps, known limitations. Keep the existing structure. Stay at
   helicopter level, in plain words for a non-programmer reader; no code
   details, no strategy (why it matters, priorities, timing: those live in
   the coordination folder's `SUBPROJECTS.md`), no decisions. English,
   Markdown wrapped at about 80 characters.

5. **Stamp it.** Set the line to
   `Checked against: <short hash of HEAD> (<today, YYYY-MM-DD>)`, even when
   the new commits needed no content change.

6. **Report and stop.** List the commits you covered and what you changed
   in the summary. Do not commit: the user reviews and commits the summary.
