# Frontend Development Guidelines

> Best practices for frontend development in this project.

---

## Overview

Guidelines for the **tagflow-ui** Vue 3 frontend (TypeScript strict + Vite + Pinia + Tailwind + vue-virtual-scroller). All rules are extracted from the actual codebase — match these patterns when writing code.

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | views/components/stores/api layout, naming, @ alias | Filled |
| [Component Guidelines](./component-guidelines.md) | script setup, typed props/emits, Tailwind-only styling | Filled |
| [Hook Guidelines](./hook-guidelines.md) | no composables yet — data flows API → store → view | Filled |
| [State Management](./state-management.md) | options-style Pinia stores, auth/localStorage ownership | Filled |
| [Quality Guidelines](./quality-guidelines.md) | vue-tsc gate, forbidden patterns, manual test bar | Filled |
| [Type Safety](./type-safety.md) | strict TS, snake_case DTO mirrors, no any | Filled |

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
