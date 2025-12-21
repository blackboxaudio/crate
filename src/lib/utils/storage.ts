const STORAGE_PREFIX = 'crate:'

export function getStoredSet(key: string): Set<string> {
	try {
		const stored = localStorage.getItem(STORAGE_PREFIX + key)
		if (stored) {
			const parsed = JSON.parse(stored)
			if (Array.isArray(parsed)) {
				return new Set(parsed)
			}
		}
	} catch {
		// Ignore parse errors, return empty set
	}
	return new Set()
}

export function setStoredSet(key: string, value: Set<string>): void {
	try {
		localStorage.setItem(STORAGE_PREFIX + key, JSON.stringify([...value]))
	} catch {
		// Ignore storage errors (e.g., quota exceeded)
	}
}

export function getStoredNumber(key: string, defaultValue: number): number {
	try {
		const stored = localStorage.getItem(STORAGE_PREFIX + key)
		if (stored !== null) {
			const parsed = Number(stored)
			if (!isNaN(parsed)) {
				return parsed
			}
		}
	} catch {
		// Ignore parse errors
	}
	return defaultValue
}

export function setStoredNumber(key: string, value: number): void {
	try {
		localStorage.setItem(STORAGE_PREFIX + key, String(value))
	} catch {
		// Ignore storage errors (e.g., quota exceeded)
	}
}

export function getStoredBoolean(key: string, defaultValue: boolean): boolean {
	try {
		const stored = localStorage.getItem(STORAGE_PREFIX + key)
		if (stored !== null) {
			return stored === 'true'
		}
	} catch {
		// Ignore parse errors
	}
	return defaultValue
}

export function setStoredBoolean(key: string, value: boolean): void {
	try {
		localStorage.setItem(STORAGE_PREFIX + key, String(value))
	} catch {
		// Ignore storage errors (e.g., quota exceeded)
	}
}
