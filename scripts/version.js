#!/usr/bin/env node
/**
 * Version bump script for Crate
 * Synchronizes version across package.json, Cargo.toml, and tauri.conf.json
 *
 * Usage: node scripts/version.js <major|minor|patch>
 */

import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT = join(__dirname, '..')

const BUMP_TYPES = ['major', 'minor', 'patch']

function parseVersion(version) {
	const match = version.match(/^(\d+)\.(\d+)\.(\d+)$/)
	if (!match) {
		throw new Error(`Invalid version format: ${version}`)
	}
	return {
		major: parseInt(match[1], 10),
		minor: parseInt(match[2], 10),
		patch: parseInt(match[3], 10),
	}
}

function bumpVersion(version, type) {
	const { major, minor, patch } = parseVersion(version)

	switch (type) {
		case 'major':
			return `${major + 1}.0.0`
		case 'minor':
			return `${major}.${minor + 1}.0`
		case 'patch':
			return `${major}.${minor}.${patch + 1}`
		default:
			throw new Error(`Invalid bump type: ${type}`)
	}
}

function updateVersion(bumpType) {
	if (!BUMP_TYPES.includes(bumpType)) {
		console.error(`Invalid bump type: ${bumpType}`)
		console.error('Usage: node scripts/version.js <major|minor|patch>')
		process.exit(1)
	}

	// Read current version from package.json
	const packageJsonPath = join(ROOT, 'package.json')
	const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'))
	const oldVersion = packageJson.version
	const newVersion = bumpVersion(oldVersion, bumpType)

	// Update package.json
	packageJson.version = newVersion
	writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, '\t') + '\n')
	console.log(`Updated package.json: ${oldVersion} -> ${newVersion}`)

	// Update Cargo.toml
	const cargoTomlPath = join(ROOT, 'src-tauri', 'Cargo.toml')
	let cargoToml = readFileSync(cargoTomlPath, 'utf-8')
	cargoToml = cargoToml.replace(/^version = "[\d.]+"$/m, `version = "${newVersion}"`)
	writeFileSync(cargoTomlPath, cargoToml)
	console.log(`Updated src-tauri/Cargo.toml: ${oldVersion} -> ${newVersion}`)

	// Update tauri.conf.json
	const tauriConfPath = join(ROOT, 'src-tauri', 'tauri.conf.json')
	const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf-8'))
	tauriConf.version = newVersion
	writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, '  ') + '\n')
	console.log(`Updated src-tauri/tauri.conf.json: ${oldVersion} -> ${newVersion}`)

	console.log(`\nVersion bumped from ${oldVersion} to ${newVersion}`)
}

const bumpType = process.argv[2]
if (!bumpType) {
	console.error('Usage: node scripts/version.js <major|minor|patch>')
	process.exit(1)
}

updateVersion(bumpType)
