#!/usr/bin/env node
/**
 * Syncs CHANGELOG.md from project root to docs site with Starlight frontmatter
 */

import { readFileSync, writeFileSync, mkdirSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const DOCS_ROOT = join(__dirname, '..')
const PROJECT_ROOT = join(DOCS_ROOT, '..')

const changelog = readFileSync(join(PROJECT_ROOT, 'CHANGELOG.md'), 'utf-8')

// Add Starlight frontmatter
const docsChangelog = `---
title: Changelog
description: Release history and changes for Crate
---

${changelog}
`

// Ensure directory exists
mkdirSync(join(DOCS_ROOT, 'src/content/docs'), { recursive: true })

writeFileSync(join(DOCS_ROOT, 'src/content/docs/changelog.md'), docsChangelog)

console.log('Changelog synced to docs site')
