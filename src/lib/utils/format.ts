/**
 * Format duration in milliseconds to MM:SS or HH:MM:SS
 */
export function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
  }
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

/**
 * Format duration in milliseconds to M:SS (compact format for track lists)
 */
export function formatDurationCompact(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, '0')}`;
}

/**
 * Format BPM to display string
 */
export function formatBpm(bpm: number | null): string {
  if (bpm === null) return '-';
  return bpm.toFixed(1);
}

/**
 * Format key for display (already in Camelot or standard notation)
 */
export function formatKey(key: string | null): string {
  if (!key) return '-';
  return key;
}

/**
 * Format file size in bytes to human-readable string
 */
export function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/**
 * Format date string to localized display
 */
export function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleDateString();
}

/**
 * Format date string to relative time (e.g., "2 days ago")
 */
export function formatRelativeDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

  if (diffDays === 0) return 'Today';
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays} days ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)} weeks ago`;
  if (diffDays < 365) return `${Math.floor(diffDays / 30)} months ago`;
  return `${Math.floor(diffDays / 365)} years ago`;
}

/**
 * Format bitrate for display
 */
export function formatBitrate(bitrate: number | null): string {
  if (bitrate === null) return '-';
  return `${bitrate} kbps`;
}

/**
 * Get display name for a track (title or filename)
 */
export function getTrackDisplayName(track: { title: string | null; file_path: string }): string {
  if (track.title) return track.title;
  // Extract filename from path
  const parts = track.file_path.split(/[/\\]/);
  const filename = parts[parts.length - 1];
  // Remove extension
  return filename.replace(/\.[^.]+$/, '');
}

/**
 * Get display artist (or "Unknown Artist")
 */
export function getTrackDisplayArtist(track: { artist: string | null }): string {
  return track.artist || 'Unknown Artist';
}
