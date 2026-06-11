# Backend Development Guidelines

> Best practices for backend development in this project.

---

## Overview

Guidelines for the **tagflow-core** Rust backend (Axum + sqlx/SQLite + Tokio, hexagonal architecture). All rules are extracted from the actual codebase — match these patterns when writing code.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | api/core/engine/infra/models layout, route wiring recipe | Filled |
| [Database Guidelines](./database-guidelines.md) | sqlx runtime queries, migrations, WAL/FK pragmas, naming | Filled |
| [Error Handling](./error-handling.md) | anyhow below API, StatusCode mapping, validation at boundary | Filled |
| [Quality Guidelines](./quality-guidelines.md) | forbidden/required patterns, inline unit tests, review checklist | Filled |
| [Logging Guidelines](./logging-guidelines.md) | tracing levels, emoji request middleware, what not to log | Filled |

---

## How to Fill These Guidelines

For each guideline file:

1. Document your project's **actual conventions** (not ideals)
2. Include **code examples** from your codebase
3. List **forbidden patterns** and why
4. Add **common mistakes** your team has made

The goal is to help AI assistants and new team members understand how YOUR project works.

---

**Language**: All documentation should be written in **English**.
