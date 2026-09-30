# Issue tracker: GitHub

Engineering issues and specs for this repo live in GitHub Issues. Use the `gh` CLI from this clone; the `origin` remote identifies `Dzahc/MacroLoom`.

## Conventions

- Create: `gh issue create --title "..." --body-file <path>` for a multiline body.
- Read: `gh issue view <number> --comments`; fetch labels with `gh issue view <number> --json labels`.
- List: `gh issue list --state open --json number,title,body,labels`, adding label or state filters as needed.
- Comment: `gh issue comment <number> --body-file <path>` for a multiline comment.
- Apply or remove labels: `gh issue edit <number> --add-label "..."` or `--remove-label "..."`.
- Close: `gh issue close <number> --comment "..."`.

## Pull requests as a triage surface

**PRs as a request surface: no.**

When set to `yes`, use `gh pr view <number> --comments` and `gh pr diff <number>` to inspect a PR. List open PRs with `gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`; triage external authors with association `CONTRIBUTOR`, `FIRST_TIME_CONTRIBUTOR`, or `NONE`. Use `gh pr comment`, `gh pr edit`, and `gh pr close` for updates. A bare issue number can identify a PR, so try `gh pr view <number>` before `gh issue view <number>` when its type is unclear.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --comments`.

## Wayfinding operations

Used by `/wayfinder`. The map is one issue with child issues as tickets.

- Map: create one issue labelled `wayfinder:map` with Notes, Decisions-so-far, and Fog sections.
- Child ticket: link an issue as a GitHub sub-issue with `gh api` when supported. Otherwise, add it to a task list in the map and put `Part of #<map>` at the top of the child. Use a `wayfinder:<type>` label (`research`, `prototype`, `grilling`, or `task`).
- Blocking: use native issue dependencies. Add a blocker with `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`. Get the blocker's database ID with `gh api repos/<owner>/<repo>/issues/<n> --jq .id`. If dependencies are unavailable, put `Blocked by: #<n>, #<n>` at the top of the child.
- Frontier: scan the map's open children in map order; take the first with no open blocker and no assignee.
- Claim: run `gh issue edit <n> --add-assignee @me` before starting work.
- Resolve: comment with the answer, close the child, then add a brief decision and link to the map's Decisions-so-far.
