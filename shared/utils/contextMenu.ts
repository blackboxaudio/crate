import type { ContextMenuItem } from '../types'

/**
 * Assemble a context menu from ordered groups (see .claude/docs/CONTEXT_MENUS.md): empty groups
 * vanish along with their divider, so callers describe every group unconditionally and never
 * track whether a divider is still needed.
 */
export function joinMenuGroups(groups: ContextMenuItem[][]): ContextMenuItem[] {
	return groups
		.filter((group) => group.length > 0)
		.flatMap((group, i) => (i === 0 ? group : [{ id: `divider-${i}`, label: '', divider: true }, ...group]))
}
