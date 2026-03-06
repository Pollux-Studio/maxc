# 🌿 Branching Strategy

This document describes the branching model used in **maxc**.
The goal of this strategy is to keep development organized, stable, and easy for contributors.


# 📌 Overview

The repository follows a **simple trunk-based workflow with feature branches**.

Main branches:

```
main
develop
feature/*
bugfix/*
hotfix/*
release/*
```

Each branch has a specific purpose.

# 🌳 Branch Types

## main

The **main** branch contains the latest stable version of maxc.

Characteristics:

* production ready code
* tagged releases
* protected branch
* no direct commits allowed

All stable releases are created from this branch.

Example:

```
main
 └ v1.0.0
 └ v1.1.0
```

## develop

The **develop** branch is the primary integration branch.

Characteristics:

* active development happens here
* features merge into develop
* may contain unstable code

Typical workflow:

```
feature → develop → main
```

## feature branches

Feature branches are used for developing new features.

Naming format:

```
feature/<feature-name>
```

Examples:

```
feature/browser-surfaces
feature/terminal-engine
feature/workspace-manager
```

Workflow:

```
develop
   │
   └ feature/browser-surfaces
```

Once the feature is complete, it should be merged back into **develop** through a pull request.

## bugfix branches

Bug fixes for issues found during development.

Naming format:

```
bugfix/<issue-description>
```

Examples:

```
bugfix/pane-resize-crash
bugfix/terminal-input-freeze
```

Bugfix branches are created from **develop** and merged back into **develop**.

## hotfix branches

Hotfix branches are used for urgent fixes in production.

Naming format:

```
hotfix/<issue-name>
```

Example:

```
hotfix/crash-on-startup
```

Workflow:

```
main
 └ hotfix/crash-on-startup
```

Hotfix branches are merged into both:

* `main`
* `develop`

## release branches

Release branches prepare a new version.

Naming format:

```
release/vX.X.X
```

Example:

```
release/v0.1.0
release/v1.0.0
```

Release branches allow final adjustments before a release.

Tasks performed here:

* documentation updates
* version bumps
* final bug fixes

# 🔄 Development Flow

Typical workflow:

```
develop
 ├ feature/workspace-manager
 ├ feature/browser-engine
 ├ feature/cli
```

After review:

```
feature → develop
```

When ready for release:

```
develop → release/vX.X.X → main
```

# 🛡 Branch Protection Rules

The following rules should be enabled for **main** and **develop**.

* no direct commits
* pull requests required
* CI checks must pass
* at least one code review

# 🏷 Version Tagging

Stable releases are tagged in the `main` branch.

Example:

```
v0.1.0
v0.2.0
v1.0.0
```

Semantic versioning format:

```
MAJOR.MINOR.PATCH
```

Example:

```
1.4.2
```

# 📦 Example Workflow

Example feature implementation.

```
git checkout develop
git checkout -b feature/browser-surfaces
```

Work on the feature.

```
git add .
git commit -m "Add browser surface engine"
```

Push branch.

```
git push origin feature/browser-surfaces
```

Create pull request:

```
feature/browser-surfaces → develop
```

After approval it will be merged.

# 🚀 Summary

| Branch    | Purpose                    |
| --------- | -------------------------- |
| main      | stable production releases |
| develop   | active development         |
| feature/* | new features               |
| bugfix/*  | development bug fixes      |
| hotfix/*  | urgent production fixes    |
| release/* | release preparation        |

This workflow ensures that **maxc development remains stable, organized, and scalable for open source contributions**.
